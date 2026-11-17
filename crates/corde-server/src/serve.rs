//! The accept loop: every connection is bounded in number and in patience.
//!
//! Anything that can reach the port can open connections, before any rule of the guard is
//! looked at — on the home server, before any pairing. Left alone, a few hundred idle
//! connections (or requests whose headers never end) take every file descriptor of the
//! process and nobody else gets in. So:
//!
//! * **patience**: a connection that does not send a full request head within
//!   [`HEADER_READ_TIMEOUT`] is closed — the head of its first request, and of each one after
//!   (an idle keep-alive connection ends the same way; browsers open a new one);
//! * **number**: at most [`MAX_CONNECTIONS`] at once, and [`MAX_CONNECTIONS_PER_ADDRESS`]
//!   from one address of the network (this PC itself is not counted per address: its
//!   programs are many — the window, a browser, every OBS source). A connection over the
//!   limit is closed at once. A WebSocket counts for as long as it stays open.
//!
//! HTTP/1.1 only, as before (what browsers speak without TLS). Stopping: no new connection,
//! the requests in flight end, then at most [`DRAIN_TIMEOUT`] of patience.

use axum::extract::Request;
use axum::Router;
use hyper::body::Incoming;
use hyper_util::rt::{TokioIo, TokioTimer};
use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, OwnedSemaphorePermit, Semaphore};
use tower::{Service, ServiceExt};

/// A request head (and the wait for the next one) may take this long.
pub const HEADER_READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Connections open at once, every address together.
pub const MAX_CONNECTIONS: usize = 512;
/// Connections open at once from one address that is not this machine. Generous on purpose:
/// behind a container's port forwarding, every device of the house may show the same address
/// (the bridge's), and a browser alone opens half a dozen connections plus its WebSockets.
pub const MAX_CONNECTIONS_PER_ADDRESS: usize = 128;
/// How long the requests in flight get to end when the server stops.
pub const DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// The limits of one server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub header_read_timeout: Duration,
    pub max_connections: usize,
    pub max_per_address: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            header_read_timeout: HEADER_READ_TIMEOUT,
            max_connections: MAX_CONNECTIONS,
            max_per_address: MAX_CONNECTIONS_PER_ADDRESS,
        }
    }
}

/// Connections open per address (addresses of the network only).
#[derive(Debug, Default)]
struct PerAddress(Mutex<HashMap<IpAddr, usize>>);

impl PerAddress {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<IpAddr, usize>> {
        match self.0.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    /// One more connection from `ip`, unless it already holds `max`.
    fn enter(self: &Arc<Self>, ip: IpAddr, max: usize) -> Option<AddressSlot> {
        let mut map = self.lock();
        let n = map.entry(ip).or_insert(0);
        if *n >= max {
            return None;
        }
        *n += 1;
        Some(AddressSlot {
            owner: Arc::clone(self),
            ip,
        })
    }
}

/// One connection of an address, given back when dropped.
#[derive(Debug)]
struct AddressSlot {
    owner: Arc<PerAddress>,
    ip: IpAddr,
}

impl Drop for AddressSlot {
    fn drop(&mut self) {
        let mut map = self.owner.lock();
        if let Some(n) = map.get_mut(&self.ip) {
            *n = n.saturating_sub(1);
            if *n == 0 {
                map.remove(&self.ip);
            }
        }
    }
}

/// IPv4-mapped IPv6 counted as the IPv4 address.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

/// A connection that holds its place in the counts for as long as it lives — through a
/// WebSocket upgrade too: the socket is then owned by the WebSocket, with its place.
struct Counted {
    io: TcpStream,
    _total: OwnedSemaphorePermit,
    _address: Option<AddressSlot>,
}

impl AsyncRead for Counted {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_read(cx, buf)
    }
}

impl AsyncWrite for Counted {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.io).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.io).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }
}

/// Serve `app` on `listener` until `shutdown` resolves, within `limits`. Every request
/// carries the address it came from (`ConnectInfo<SocketAddr>`).
pub async fn serve(
    listener: TcpListener,
    app: Router,
    limits: Limits,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> io::Result<()> {
    let mut make_service = app.into_make_service_with_connect_info::<SocketAddr>();
    let total = Arc::new(Semaphore::new(limits.max_connections.max(1)));
    let per_address = Arc::new(PerAddress::default());
    // `true` once the server stops: every connection ends its request in flight and closes.
    let (closing_tx, closing_rx) = watch::channel(false);
    // Dropped by each connection when it ends: `done_tx.closed()` is « all of them ended ».
    let (done_tx, done_rx) = watch::channel(());
    let mut shutdown = std::pin::pin!(shutdown);
    let mut refused_logged = std::time::Instant::now() - Duration::from_secs(3600);

    loop {
        let (socket, peer) = tokio::select! {
            biased;
            _ = &mut shutdown => break,
            accepted = listener.accept() => match accepted {
                Ok(pair) => pair,
                Err(e) => {
                    // Out of file descriptors, a connection reset before it was accepted…:
                    // nothing to stop for. A short pause keeps a persistent error from
                    // spinning.
                    tracing::debug!("accept failed: {e}");
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    continue;
                }
            },
        };
        let ip = canonical(peer.ip());
        let slots = Arc::clone(&total).try_acquire_owned().ok().and_then(|t| {
            if ip.is_loopback() {
                Some((t, None))
            } else {
                per_address
                    .enter(ip, limits.max_per_address.max(1))
                    .map(|a| (t, Some(a)))
            }
        });
        let Some((total_slot, address_slot)) = slots else {
            // Closed at once (dropping the socket). Said once in a while, not per connection.
            if refused_logged.elapsed() >= Duration::from_secs(60) {
                refused_logged = std::time::Instant::now();
                tracing::warn!("too many connections at once: new ones from {ip} are closed");
            }
            continue;
        };
        let _ = socket.set_nodelay(true);
        let tower_service = match make_service.call(peer).await {
            Ok(s) => s,
            Err(never) => match never {},
        };
        let io = TokioIo::new(Counted {
            io: socket,
            _total: total_slot,
            _address: address_slot,
        });
        let mut closing = closing_rx.clone();
        let done = done_rx.clone();
        tokio::spawn(async move {
            let hyper_service = hyper::service::service_fn(move |request: Request<Incoming>| {
                tower_service
                    .clone()
                    .oneshot(request.map(axum::body::Body::new))
            });
            let mut builder = hyper::server::conn::http1::Builder::new();
            builder
                .timer(TokioTimer::new())
                .header_read_timeout(limits.header_read_timeout);
            let conn = builder.serve_connection(io, hyper_service).with_upgrades();
            let mut conn = std::pin::pin!(conn);
            let mut asked = false;
            loop {
                tokio::select! {
                    result = conn.as_mut() => {
                        if let Err(e) = result {
                            tracing::trace!("connection from {peer} ended: {e}");
                        }
                        break;
                    }
                    _ = closing.changed(), if !asked => {
                        asked = true;
                        conn.as_mut().graceful_shutdown();
                    }
                }
            }
            drop(done);
        });
    }

    // No new connection; the ones in flight end their request.
    drop(listener);
    drop(done_rx);
    let _ = closing_tx.send(true);
    if tokio::time::timeout(DRAIN_TIMEOUT, done_tx.closed())
        .await
        .is_err()
    {
        tracing::warn!(
            "some connections were still busy after {} s: closed",
            DRAIN_TIMEOUT.as_secs()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::ConnectInfo;
    use axum::routing::get;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn start(
        limits: Limits,
    ) -> (
        SocketAddr,
        tokio::sync::oneshot::Sender<()>,
        tokio::task::JoinHandle<io::Result<()>>,
    ) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/who",
            get(|ConnectInfo(peer): ConnectInfo<SocketAddr>| async move { peer.ip().to_string() }),
        );
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        let task = tokio::spawn(serve(listener, app, limits, async move {
            let _ = stopped.await;
        }));
        (addr, stop, task)
    }

    async fn get_who(addr: SocketAddr) -> String {
        let mut s = TcpStream::connect(addr).await.unwrap();
        s.write_all(b"GET /who HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
            .await
            .unwrap();
        let mut out = String::new();
        s.read_to_string(&mut out).await.unwrap();
        out
    }

    /// `true` when the server closed the connection within `within`.
    async fn closed_within(s: &mut TcpStream, within: Duration) -> bool {
        let mut buf = [0u8; 64];
        loop {
            match tokio::time::timeout(within, s.read(&mut buf)).await {
                Ok(Ok(0)) | Ok(Err(_)) => return true,
                Ok(Ok(_)) => continue,
                Err(_) => return false,
            }
        }
    }

    #[tokio::test]
    async fn requests_are_served_with_the_address_they_came_from() {
        let (addr, stop, task) = start(Limits::default()).await;
        let answer = get_who(addr).await;
        assert!(answer.starts_with("HTTP/1.1 200"), "{answer}");
        assert!(answer.ends_with("127.0.0.1"), "{answer}");
        let _ = stop.send(());
        task.await.unwrap().unwrap();
        // Stopped: the port no longer answers.
        assert!(TcpStream::connect(addr).await.is_err());
    }

    /// A connection that says nothing, or never ends its headers, does not stay for ever.
    #[tokio::test]
    async fn a_connection_that_never_sends_its_request_is_closed() {
        let limits = Limits {
            header_read_timeout: Duration::from_millis(300),
            ..Limits::default()
        };
        let (addr, stop, task) = start(limits).await;
        // Silent.
        let mut silent = TcpStream::connect(addr).await.unwrap();
        // Half a request, then nothing.
        let mut slow = TcpStream::connect(addr).await.unwrap();
        slow.write_all(b"GET /who HTTP/1.1\r\nHost: x\r\nX-Slow: ")
            .await
            .unwrap();
        assert!(closed_within(&mut silent, Duration::from_secs(5)).await);
        assert!(closed_within(&mut slow, Duration::from_secs(5)).await);
        // A keep-alive connection is closed the same way once idle.
        let mut idle = TcpStream::connect(addr).await.unwrap();
        idle.write_all(b"GET /who HTTP/1.1\r\nHost: x\r\n\r\n")
            .await
            .unwrap();
        assert!(closed_within(&mut idle, Duration::from_secs(5)).await);
        // The server still answers.
        assert!(get_who(addr).await.starts_with("HTTP/1.1 200"));
        let _ = stop.send(());
        task.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn connections_over_the_limit_are_closed_at_once() {
        let limits = Limits {
            max_connections: 3,
            ..Limits::default()
        };
        let (addr, stop, task) = start(limits).await;
        let mut held = Vec::new();
        for _ in 0..3 {
            let mut s = TcpStream::connect(addr).await.unwrap();
            // Something on the wire, so that each one is really accepted and counted.
            s.write_all(b"GET /who HTTP/1.1\r\n").await.unwrap();
            held.push(s);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
        let mut extra = TcpStream::connect(addr).await.unwrap();
        assert!(
            closed_within(&mut extra, Duration::from_secs(2)).await,
            "the fourth is closed"
        );
        // A place given back is taken again.
        drop(held.pop());
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert!(get_who(addr).await.starts_with("HTTP/1.1 200"));
        drop(held);
        let _ = stop.send(());
        task.await.unwrap().unwrap();
    }

    #[test]
    fn addresses_of_the_network_are_counted_one_by_one() {
        let per = Arc::new(PerAddress::default());
        let phone: IpAddr = "192.168.1.42".parse().unwrap();
        let other: IpAddr = "192.168.1.43".parse().unwrap();
        let a = per.enter(phone, 2).unwrap();
        let _b = per.enter(phone, 2).unwrap();
        assert!(per.enter(phone, 2).is_none(), "the third is refused");
        assert!(per.enter(other, 2).is_some(), "another address is not");
        drop(a);
        assert!(per.enter(phone, 2).is_some(), "a place given back");
        // The same phone seen through an IPv6 socket is the same address.
        assert_eq!(canonical("::ffff:192.168.1.42".parse().unwrap()), phone);
    }
}
