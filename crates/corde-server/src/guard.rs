//! Who may call what. Corde has no accounts (it serves a home network), so
//! the server bounds its exposure instead — see `docs/SECURITY-MODEL.md`:
//!
//! 1. **`Host`** (every request, DNS rebinding): with the LAN closed only
//!    `localhost` and loopback addresses; with the LAN open, also any IP
//!    literal, this machine's name and `<name>.local`. Otherwise 403.
//! 2. **`Origin`** (`/api/*` and the WebSocket upgrade — browsers do not
//!    apply CORS to WebSockets): a request that carries one is accepted only
//!    from the page's own server (same scheme, host and port as `Host`), from
//!    a page of this PC (`http://127.0.0.1:*`, `http://localhost:*`,
//!    `http://[::1]:*`) or from the desktop shell (`tauri://localhost`,
//!    `http(s)://tauri.localhost`). Otherwise 403, `GET` included. No
//!    `Origin` (curl, a direct visit) is accepted. The CORS headers reflect
//!    those origins only.
//! 3. **Routes "this PC only"** ([`route_access`]): whatever deletes data,
//!    starts heavy work or writes outside the data folder answers 403 unless
//!    the peer is the loopback (same rule as `PUT /api/settings`,
//!    [`crate::net::may_change_settings`]).
//! 4. **Errors**: a 5xx answered to another device of the LAN loses its
//!    details (they may name a folder of the PC); they stay in the log.

use crate::net::{
    listens_on_lan, may_change_settings, origin_is_local, parse_ip_host, peer_is_local,
    split_host_port, split_origin,
};
use axum::body::Body;
use axum::extract::connect_info::MockConnectInfo;
use axum::extract::{ConnectInfo, MatchedPath, Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;
use tower_http::cors::{AllowOrigin, CorsLayer};

/// Largest request body any route accepts (layouts, feedback, settings…
/// are a few kilobytes).
pub const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Largest message a dashboard may send on the WebSocket (`SetRate`,
/// `Ping`: a few dozen bytes).
pub const WS_MAX_MESSAGE_BYTES: usize = 16 * 1024;

/// Who may call a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Any device allowed to reach the server (this PC; the LAN when open).
    Open,
    /// A program of this PC only ([`may_change_settings`]).
    ThisPcOnly,
}

/// The access rule of a route, by method and route template (as axum
/// matched it, `/api/sessions/{id}`). Unknown routes are [`Access::Open`]:
/// they are reads, the static UI or a 404.
///
/// Kept in sync with the table of `docs/SECURITY-MODEL.md`.
pub fn route_access(method: &Method, route: &str) -> Access {
    let pc_only = matches!(
        (method.as_str(), route),
        // Deletes data.
        ("DELETE", "/api/layouts")
            | ("DELETE", "/api/layouts/{id}")
            | ("DELETE", "/api/sessions/{id}")
            // Heavy work (full analysis, scan of every results file).
            | ("POST", "/api/sessions/{id}/analysis/recompute")
            | ("POST", "/api/profile/import")
            // Writes outside the data folder (the game's setups).
            | ("POST", "/api/sessions/{id}/setup-files")
            // Opens or closes the server to the network; lets it read the
            // race schedule on the web (`online_schedule`).
            | ("PUT", "/api/settings")
    );
    // Deliberately open (the phone uses them), bounded by their handlers:
    // GET /api/races/sources (fetches only the closed list of `races::sources`,
    // and only when `online_schedule` is on), POST
    // /api/races/sources/{source}/unreadable (keeps a copy the server already
    // holds, one file per source), GET /api/races/picks, PUT / DELETE
    // /api/races/picks/{id} (≤ 50 validated picks), GET / PUT
    // /api/races/settings (validated keys).
    if pc_only {
        Access::ThisPcOnly
    } else {
        Access::Open
    }
}

/// The `Host` values this server answers to.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    /// The server listens on the LAN (decided at start-up).
    lan: bool,
    /// This machine's names, lower case (`pc-jeu`, `pc-jeu.local`).
    machine_names: Vec<String>,
}

impl HostPolicy {
    /// For a server bound to `listen`, with this machine's names.
    pub fn for_listener(listen: SocketAddr) -> Self {
        Self::new(listens_on_lan(listen.ip()), &machine_names())
    }

    pub fn new(lan: bool, names: &[String]) -> Self {
        let mut machine_names = Vec::new();
        for n in names {
            let n = n.trim().trim_end_matches('.').to_ascii_lowercase();
            if n.is_empty() || n == "localhost" || n.parse::<IpAddr>().is_ok() {
                continue;
            }
            let short = n.strip_suffix(".local").unwrap_or(&n).to_string();
            for v in [short.clone(), format!("{short}.local")] {
                if !machine_names.contains(&v) {
                    machine_names.push(v);
                }
            }
        }
        Self { lan, machine_names }
    }

    pub fn lan(&self) -> bool {
        self.lan
    }

    /// `true` when `host` (a `Host` header: `name[:port]`, `[v6][:port]`)
    /// names this server.
    pub fn allows(&self, host: &str) -> bool {
        let Some((name, _port)) = split_host_port(host) else {
            return false;
        };
        if name == "localhost" {
            return true;
        }
        if let Some(ip) = parse_ip_host(&name) {
            // With the LAN closed only the loopback is reachable anyway; a
            // DNS name is what a rebinding attack needs, never an IP literal.
            return ip.is_loopback() || self.lan;
        }
        self.lan && self.machine_names.contains(&name)
    }
}

/// This machine's network names, as the OS reports them (`COMPUTERNAME` on
/// Windows, the kernel host name on Linux). Empty when unknown.
pub fn machine_names() -> Vec<String> {
    let mut names = Vec::new();
    for var in ["COMPUTERNAME", "HOSTNAME"] {
        if let Ok(v) = std::env::var(var) {
            names.push(v);
        }
    }
    #[cfg(unix)]
    for file in ["/proc/sys/kernel/hostname", "/etc/hostname"] {
        if let Ok(v) = std::fs::read_to_string(file) {
            names.push(v.trim().to_string());
        }
    }
    names.retain(|n| !n.trim().is_empty());
    names
}

/// `true` when a browser page of `origin` may call this server, reached as
/// `host` (the request's `Host` header).
pub fn origin_allowed(origin: &str, host: Option<&str>) -> bool {
    origin_is_local(origin) || host.is_some_and(|h| same_origin(origin, h))
}

/// The page comes from this very server: `http://<Host>`.
fn same_origin(origin: &str, host: &str) -> bool {
    let Some((scheme, authority)) = split_origin(origin) else {
        return false;
    };
    if scheme != "http" {
        return false;
    }
    match (split_host_port(&authority), split_host_port(host)) {
        (Some((oh, op)), Some((hh, hp))) => oh == hh && op.unwrap_or(80) == hp.unwrap_or(80),
        _ => false,
    }
}

/// CORS headers for the allowed origins only (preflights included).
pub fn cors_layer() -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, parts| {
            let host = parts
                .headers
                .get(header::HOST)
                .and_then(|h| h.to_str().ok());
            origin.to_str().is_ok_and(|o| origin_allowed(o, host))
        }))
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT])
        .max_age(Duration::from_secs(600))
}

fn forbidden(msg: &'static str) -> Response {
    (StatusCode::FORBIDDEN, Json(json!({ "error": msg }))).into_response()
}

/// `/api/*` and `/ws`: where `Origin` is checked.
fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/ws" || path.starts_with("/ws/")
}

/// Shown to another device instead of a 5xx's details.
pub const HIDDEN_ERROR: &str = "erreur interne du serveur (détails dans le journal du PC)";

/// The middleware: `Host`, `Origin`, "this PC only" routes, hidden 5xx
/// details for the LAN. Runs after routing (`MatchedPath` known).
pub async fn guard(State(policy): State<Arc<HostPolicy>>, req: Request, next: Next) -> Response {
    let headers = req.headers();
    let host = headers.get(header::HOST).map(|h| h.to_str().unwrap_or(""));
    if let Some(h) = host {
        if !policy.allows(h) {
            tracing::debug!("refused: unexpected Host {h:?}");
            return forbidden(
                "hôte non autorisé : ouvre Corde par l'adresse IP du PC ou par 127.0.0.1",
            );
        }
    }
    let origin = headers
        .get(header::ORIGIN)
        .map(|o| o.to_str().unwrap_or(""));
    if is_api_path(req.uri().path()) {
        if let Some(o) = origin {
            if !origin_allowed(o, host) {
                tracing::debug!("refused: origin {o:?} for {}", req.uri().path());
                return forbidden("origine non autorisée");
            }
        }
    }
    // No ConnectInfo (a router driven without a socket): not this PC.
    let peer = req
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .or_else(|| {
            req.extensions()
                .get::<MockConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip())
        });
    if let Some(route) = req.extensions().get::<MatchedPath>() {
        if route_access(req.method(), route.as_str()) == Access::ThisPcOnly
            && !peer.is_some_and(|p| may_change_settings(p, origin))
        {
            return forbidden(
                "cette action n'est possible que depuis le PC qui fait tourner Corde",
            );
        }
    }
    let response = next.run(req).await;
    match peer {
        Some(p) if !peer_is_local(p) && response.status().is_server_error() => {
            hide_details(response, p).await
        }
        _ => response,
    }
}

/// A 5xx for another device: same status, generic message; the details go
/// to the log of the PC.
async fn hide_details(response: Response, peer: IpAddr) -> Response {
    let (mut parts, body) = response.into_parts();
    let details = axum::body::to_bytes(body, 64 * 1024)
        .await
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default();
    tracing::warn!(
        "error {} for {peer} (details hidden from the network): {details}",
        parts.status.as_u16()
    );
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    let body = json!({ "error": HIDDEN_ERROR }).to_string();
    Response::from_parts(parts, Body::from(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::{delete, get, post, put};
    use axum::Router;
    use tower::ServiceExt;

    fn names() -> Vec<String> {
        vec!["PC-Jeu".to_string()]
    }

    #[test]
    fn host_header_with_the_lan_closed() {
        let p = HostPolicy::new(false, &names());
        for ok in [
            "127.0.0.1",
            "127.0.0.1:8471",
            "localhost",
            "LOCALHOST:5173",
            "[::1]",
            "[::1]:8471",
            "127.0.0.2:8471",
        ] {
            assert!(p.allows(ok), "{ok}");
        }
        for bad in [
            "192.168.1.20:8471",
            "pc-jeu:8471",
            "pc-jeu.local",
            "evil.example",
            "evil.example:8471",
            "127.0.0.1.evil.example",
            "localhost.evil.example",
            "",
            ":8471",
            "127.0.0.1:port",
            "127.0.0.1:99999",
            "[::1",
            "::1",
            "[evil.example]:8471",
        ] {
            assert!(!p.allows(bad), "{bad:?}");
        }
    }

    #[test]
    fn host_header_with_the_lan_open() {
        let p = HostPolicy::new(true, &names());
        for ok in [
            "127.0.0.1:8471",
            "localhost:8471",
            "[::1]:8471",
            "192.168.1.20:8471",
            "10.0.0.5",
            "[fd00::20]:8471",
            "pc-jeu:8471",
            "PC-JEU",
            "pc-jeu.local:8471",
        ] {
            assert!(p.allows(ok), "{ok}");
        }
        for bad in [
            "evil.example",
            "evil.example:8471",
            "pc-jeu.evil.example",
            "pc-jeu.local.evil.example",
            "192.168.1.20.nip.io:8471",
            "",
        ] {
            assert!(!p.allows(bad), "{bad:?}");
        }
        // A `.local` name given by the OS is accepted with and without suffix.
        let p = HostPolicy::new(true, &["mac-mini.local".to_string()]);
        assert!(p.allows("mac-mini:8471") && p.allows("mac-mini.local"));
        // No name known: IP literals only.
        let p = HostPolicy::new(true, &[]);
        assert!(p.allows("192.168.1.20:8471") && !p.allows("pc-jeu"));
    }

    #[test]
    fn origins() {
        let host = Some("127.0.0.1:8471");
        for ok in [
            // The UI served by this server (desktop shell, browser of the PC, OBS).
            "http://127.0.0.1:8471",
            // Dev server, capture scripts (?server=127.0.0.1:8471).
            "http://127.0.0.1:5173",
            "http://localhost:5173",
            "http://[::1]:4173",
            // Desktop shell's own pages.
            "tauri://localhost",
            "http://tauri.localhost",
            "https://tauri.localhost",
        ] {
            assert!(origin_allowed(ok, host), "{ok}");
        }
        // A phone: the page and the API share the PC's address.
        assert!(origin_allowed(
            "http://192.168.1.20:8471",
            Some("192.168.1.20:8471")
        ));
        assert!(origin_allowed(
            "http://pc-jeu.local:8471",
            Some("PC-JEU.local:8471")
        ));
        assert!(origin_allowed("http://pc-jeu", Some("pc-jeu:80")));
        for (bad, host) in [
            ("https://evil.example", host),
            ("http://192.168.1.20:8471", host),
            ("http://192.168.1.20:9999", Some("192.168.1.20:8471")),
            ("https://192.168.1.20:8471", Some("192.168.1.20:8471")),
            ("https://localhost:8471", host),
            ("http://127.0.0.1:8471@evil.example", host),
            ("http://127.0.0.1.evil.example", host),
            ("null", host),
            ("", host),
            ("file://", host),
            ("tauri://evil.example", host),
            ("http://192.168.1.20:8471", None),
        ] {
            assert!(!origin_allowed(bad, host), "{bad:?} on {host:?}");
        }
    }

    #[test]
    fn route_classes() {
        use Access::*;
        let cases = [
            (Method::GET, "/api/sessions/{id}", Open),
            (Method::DELETE, "/api/sessions/{id}", ThisPcOnly),
            (Method::DELETE, "/api/layouts", ThisPcOnly),
            (Method::DELETE, "/api/layouts/{id}", ThisPcOnly),
            (Method::PUT, "/api/layouts", Open),
            (Method::PUT, "/api/layouts/{id}", Open),
            (Method::POST, "/api/sessions/{id}/feedback", Open),
            (Method::POST, "/api/recording/start", Open),
            (Method::POST, "/api/recording/stop", Open),
            (
                Method::POST,
                "/api/sessions/{id}/analysis/recompute",
                ThisPcOnly,
            ),
            (Method::GET, "/api/sessions/{id}/analysis", Open),
            (Method::POST, "/api/profile/import", ThisPcOnly),
            (Method::POST, "/api/sessions/{id}/setup-files", ThisPcOnly),
            (Method::PUT, "/api/settings", ThisPcOnly),
            (Method::GET, "/api/settings", Open),
            (Method::OPTIONS, "/api/settings", Open),
            (Method::GET, "/ws", Open),
            (Method::GET, "/api/races/sources", Open),
            (Method::POST, "/api/races/sources/{source}/unreadable", Open),
            (Method::GET, "/api/races/picks", Open),
            (Method::PUT, "/api/races/picks/{id}", Open),
            (Method::DELETE, "/api/races/picks/{id}", Open),
            (Method::GET, "/api/races/settings", Open),
            (Method::PUT, "/api/races/settings", Open),
        ];
        for (m, r, want) in cases {
            assert_eq!(route_access(&m, r), want, "{m} {r}");
        }
    }

    // --- the middleware on a small router ------------------------------------

    fn app(lan: bool, peer: &str) -> Router {
        let policy = Arc::new(HostPolicy::new(lan, &names()));
        let peer: SocketAddr = peer.parse().unwrap();
        Router::new()
            .route("/api/health", get(|| async { "ok" }))
            .route(
                "/api/sessions/{id}",
                delete(|| async { StatusCode::NO_CONTENT }),
            )
            .route("/api/layouts", put(|| async { "stored" }))
            .route(
                "/api/boom",
                post(|| async {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "cannot open C:\\Users\\someone\\x" })),
                    )
                }),
            )
            .route("/ws", get(|| async { "ws" }))
            .fallback(|| async { "index" })
            .layer(cors_layer())
            .layer(axum::middleware::from_fn_with_state(policy, guard))
            .layer(MockConnectInfo(peer))
    }

    async fn send(app: Router, method: Method, path: &str, h: &[(&str, &str)]) -> Response {
        let mut req = Request::builder().method(method).uri(path);
        for (k, v) in h {
            req = req.header(*k, *v);
        }
        app.oneshot(req.body(Body::empty()).unwrap()).await.unwrap()
    }

    async fn body_text(r: Response) -> String {
        let b = axum::body::to_bytes(r.into_body(), 1 << 20).await.unwrap();
        String::from_utf8_lossy(&b).into_owned()
    }

    #[tokio::test]
    async fn middleware_checks_host_origin_and_pc_only_routes() {
        let local = "127.0.0.1:50000";
        let phone = "192.168.1.42:50000";
        let h = |host| ("host", host);

        // Host.
        let r = send(
            app(false, local),
            Method::GET,
            "/api/health",
            &[h("evil.example:8471")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = send(
            app(false, local),
            Method::GET,
            "/",
            &[h("evil.example:8471")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "static UI too");
        let r = send(
            app(false, local),
            Method::GET,
            "/api/health",
            &[h("192.168.1.20:8471")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "LAN closed");
        let r = send(
            app(true, phone),
            Method::GET,
            "/api/health",
            &[h("192.168.1.20:8471")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK);
        let r = send(app(false, local), Method::GET, "/api/health", &[]).await;
        assert_eq!(r.status(), StatusCode::OK, "no Host header (not a browser)");

        // Origin on the API: refused, GET included; CORS echo for allowed ones.
        let host = h("127.0.0.1:8471");
        let r = send(
            app(false, local),
            Method::GET,
            "/api/health",
            &[host, ("origin", "https://evil.example")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = send(
            app(false, local),
            Method::GET,
            "/ws",
            &[host, ("origin", "https://evil.example")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "WebSocket upgrade");
        let r = send(
            app(false, local),
            Method::GET,
            "/",
            &[host, ("origin", "https://evil.example")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK, "static files are not the API");
        let r = send(
            app(false, local),
            Method::GET,
            "/api/health",
            &[host, ("origin", "http://localhost:5173")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "http://localhost:5173"
        );
        let r = send(app(false, local), Method::GET, "/api/health", &[host]).await;
        assert!(r.headers().get("access-control-allow-origin").is_none());

        // Preflight.
        let pre = [
            host,
            ("origin", "http://127.0.0.1:5173"),
            ("access-control-request-method", "PUT"),
            ("access-control-request-headers", "content-type"),
        ];
        let r = send(app(false, local), Method::OPTIONS, "/api/layouts", &pre).await;
        assert!(r.status().is_success(), "{}", r.status());
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "http://127.0.0.1:5173"
        );
        let mut bad = pre;
        bad[1] = ("origin", "https://evil.example");
        let r = send(app(false, local), Method::OPTIONS, "/api/layouts", &bad).await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);

        // PC-only routes.
        let lan_host = h("192.168.1.20:8471");
        let same = ("origin", "http://192.168.1.20:8471");
        let r = send(
            app(true, phone),
            Method::DELETE,
            "/api/sessions/x",
            &[lan_host, same],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        assert!(body_text(r).await.contains("depuis le PC"));
        let r = send(
            app(true, phone),
            Method::PUT,
            "/api/layouts",
            &[lan_host, same],
        )
        .await;
        assert_eq!(
            r.status(),
            StatusCode::OK,
            "dashboards stay editable from the phone"
        );
        let r = send(
            app(true, local),
            Method::DELETE,
            "/api/sessions/x",
            &[host, ("origin", "http://127.0.0.1:8471")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::NO_CONTENT);
        let r = send(app(true, local), Method::DELETE, "/api/sessions/x", &[]).await;
        assert_eq!(r.status(), StatusCode::NO_CONTENT, "curl on the PC");
    }

    #[tokio::test]
    async fn server_errors_lose_their_details_on_the_lan_only() {
        let r = send(app(true, "192.168.1.42:1"), Method::POST, "/api/boom", &[]).await;
        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let text = body_text(r).await;
        assert!(!text.contains("Users"), "{text}");
        assert!(text.contains("journal du PC"), "{text}");

        let r = send(app(true, "127.0.0.1:1"), Method::POST, "/api/boom", &[]).await;
        assert!(
            body_text(r).await.contains("Users"),
            "the PC keeps the details"
        );
    }
}
