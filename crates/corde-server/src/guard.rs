//! Who may call what. Corde has no accounts (it serves a home network), so
//! the server bounds its exposure instead — see `docs/SECURITY-MODEL.md`:
//!
//! 1. **`Host`** (every request, DNS rebinding): with the LAN closed only
//!    `localhost` and loopback addresses; with the LAN open, also any IP
//!    literal, this machine's name and `<name>.local`. Otherwise 403.
//! 2. **`Origin`** (`/api/*` and the WebSocket upgrade — browsers do not
//!    apply CORS to WebSockets): a request that carries one is accepted only
//!    from the page's own server (same scheme, host and port as `Host`), from
//!    the desktop shell (`tauri://localhost`, `http(s)://tauri.localhost`) or
//!    from one of the two applications (`capacitor://localhost`; `http://localhost`
//!    from a phone — never pages of this PC). The page of **another program**
//!    of this PC (another loopback port) is refused like any other site, unless
//!    the server runs with `--dev-origins` (development). Otherwise 403, `GET`
//!    included. No `Origin` (curl, a direct visit) is accepted. The CORS headers
//!    reflect those origins only.
//! 3. **Routes "this PC only"** ([`route_access`]): whatever deletes data,
//!    starts heavy work or writes outside the data folder answers 403 unless
//!    the peer is the loopback and the page is this server's own (same rule as
//!    `PUT /api/settings`, [`crate::net::may_change_settings`]). Every route that
//!    changes something is closed this way unless it is listed as open.
//! 4. **Errors**: a 5xx answered to another device of the LAN loses its
//!    details (they may name a folder of the PC); they stay in the log.
//! 5. **Browser protections** on every answer ([`security_headers`]): no page of
//!    another site may frame the interface (clickjacking), nothing is ever
//!    interpreted as another type than the one announced, and the interface runs
//!    under a content security policy (no script but its own files).

use crate::net::{
    listens_on_lan, may_change_settings, origin_kind, origin_may_call, parse_ip_host,
    peer_is_local, split_host_port,
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
/// matched it, `/api/sessions/{id}`).
///
/// **Closed by default**: a route that changes something (`POST`, `PUT`,
/// `PATCH`, `DELETE`) is "this PC only" unless it is listed here as open — a
/// route added later and forgotten here is closed, not open. Reads (`GET`,
/// `HEAD`, `OPTIONS`) are open: each is bounded by its handler.
///
/// Kept in sync with the table of `docs/SECURITY-MODEL.md`.
pub fn route_access(method: &Method, route: &str) -> Access {
    if matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS) {
        return Access::Open;
    }
    // Deliberately open (the phone uses them), bounded by their handlers.
    let open_write = matches!(
        (method.as_str(), route),
        // The dashboard of the phone saves its layouts (bounded: `layouts::check`).
        ("PUT", "/api/layouts")
            | ("PUT", "/api/layouts/{id}")
            // A race dashboard of the Studio, validated (`dashes::validate`).
            | ("PUT", "/api/dashes/{id}")
            // The driver's feeling, typed on the phone (≤ 2000 characters).
            | ("POST", "/api/sessions/{id}/feedback")
            // Recording, started and stopped at the wheel; deletes nothing.
            | ("POST", "/api/recording/start")
            | ("POST", "/api/recording/stop")
            // « Courses »: keeps a copy the server already holds (one file per source of
            // the closed list), ≤ 50 validated picks, their merge with another device's
            // copy (bounded lists, ≤ 256 KiB), validated settings.
            | ("POST", "/api/races/sources/{source}/unreadable")
            | ("PUT", "/api/races/picks/{id}")
            | ("DELETE", "/api/races/picks/{id}")
            | ("POST", "/api/races/picks/sync")
            | ("PUT", "/api/races/settings")
    );
    // Everything else: deletes data (layouts, dashboards, media, sessions), writes a file
    // of up to 30 MiB (a media), starts heavy work (full analysis, scan of every results
    // file), writes outside the data folder (the game's setups), opens or closes the server
    // to the network (settings), or drives the link to the home server (address, pairing —
    // the PC then holds a device token —, sending everything again, unpairing).
    if open_write {
        Access::Open
    } else {
        Access::ThisPcOnly
    }
}

/// Who may call a route of the home server (`--role hub`, ADR 0010). Every route not listed
/// needs a paired device ([`HubAccess::Device`]): unknown routes too (closed by default).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubAccess {
    /// Anyone who reaches the server: the pairing state and the typed code only.
    Public,
    /// The commands run on the server itself (`corde-server pair | devices | revoke`):
    /// loopback, no `Origin`, the control key of `<data>/hub/control.json`.
    LocalControl,
    /// Any paired device.
    Device,
    /// A paired device marked administrator (the first one paired).
    Admin,
    /// Never on the server, with the reason shown.
    Refused(&'static str),
}

/// The access rule of a route on the home server. Kept in sync with the table of
/// `docs/SECURITY-MODEL.md` §7.
pub fn hub_route_access(method: &Method, route: &str) -> HubAccess {
    use HubAccess::*;
    match (method.as_str(), route) {
        ("GET", "/api/hub/pairing") | ("POST", "/api/hub/pair") => Public,
        (_, r) if r.starts_with("/api/hub/local/") => LocalControl,
        // Deletes data (a deleted session is not sent again by the PC: tombstone), heavy
        // work, settings (the online schedule makes the server read two web pages).
        ("DELETE", "/api/layouts")
        | ("DELETE", "/api/layouts/{id}")
        | ("DELETE", "/api/dashes")
        | ("DELETE", "/api/dashes/{id}")
        | ("DELETE", "/api/dash-assets/{id}")
        | ("POST", "/api/dash-assets")
        | ("DELETE", "/api/sessions/{id}")
        | ("POST", "/api/sessions/{id}/analysis/recompute")
        | ("PUT", "/api/settings") => Admin,
        // The game's folders are on the PC, not on the server.
        ("POST", "/api/sessions/{id}/setup-files") => Refused(
            "le serveur Corde n'écrit pas dans les dossiers du jeu : crée les fichiers de réglage depuis le PC",
        ),
        ("POST", "/api/profile/import") => Refused(
            "le serveur Corde ne lit pas les fichiers du jeu : les résultats arrivent du PC à chaque synchronisation",
        ),
        ("POST", "/api/recording/start") | ("POST", "/api/recording/stop") => Refused(
            "le serveur Corde n'enregistre pas : l'enregistrement se fait sur le PC qui fait tourner le jeu",
        ),
        _ => Device,
    }
}

/// The `Host` values this server answers to.
#[derive(Debug, Clone)]
pub struct HostPolicy {
    /// The server listens on the LAN (decided at start-up).
    lan: bool,
    /// This machine's names, lower case (`pc-jeu`, `pc-jeu.local`), plus `--allow-host`.
    machine_names: Vec<String>,
    /// Home server: the "this PC only" rule gives way to the paired devices
    /// ([`hub_route_access`], enforced by `hub::hub_guard`).
    hub: bool,
    /// `--dev-origins`: the pages of other programs of this PC (other loopback ports) are
    /// accepted, as pages of this PC. Development and capture scripts only.
    dev_origins: bool,
}

impl HostPolicy {
    /// For a server bound to `listen`, with this machine's names.
    pub fn for_listener(listen: SocketAddr) -> Self {
        Self::new(listens_on_lan(listen.ip()), &machine_names())
    }

    /// For a server bound to `listen`, with this machine's names and `extra` ones
    /// (`--allow-host`), as a home server when `hub`.
    pub fn for_server(listen: SocketAddr, extra: &[String], hub: bool) -> Self {
        let mut names = machine_names();
        names.extend(extra.iter().cloned());
        let mut p = Self::new(listens_on_lan(listen.ip()), &names);
        p.hub = hub;
        p
    }

    pub fn hub(&self) -> bool {
        self.hub
    }

    /// The same policy, accepting the pages of other programs of this PC (`--dev-origins`).
    pub fn with_dev_origins(mut self, dev_origins: bool) -> Self {
        self.dev_origins = dev_origins;
        self
    }

    pub fn dev_origins(&self) -> bool {
        self.dev_origins
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
        Self {
            lan,
            machine_names,
            hub: false,
            dev_origins: false,
        }
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
/// `host` (the request's `Host` header), from `peer_local` or not
/// ([`crate::net::origin_may_call`]).
pub fn origin_allowed(
    origin: &str,
    host: Option<&str>,
    peer_local: bool,
    dev_origins: bool,
) -> bool {
    origin_may_call(origin_kind(Some(origin), host), peer_local, dev_origins)
}

/// The address a request came from, when the router is driven by a socket.
fn peer_of(extensions: &axum::http::Extensions) -> Option<IpAddr> {
    extensions
        .get::<ConnectInfo<SocketAddr>>()
        .map(|c| c.0.ip())
        .or_else(|| {
            extensions
                .get::<MockConnectInfo<SocketAddr>>()
                .map(|c| c.0.ip())
        })
}

/// CORS headers for the allowed origins only (preflights included).
pub fn cors_layer(dev_origins: bool) -> CorsLayer {
    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            move |origin: &HeaderValue, parts| {
                let host = parts
                    .headers
                    .get(header::HOST)
                    .and_then(|h| h.to_str().ok());
                let peer_local = peer_of(&parts.extensions).is_some_and(peer_is_local);
                origin
                    .to_str()
                    .is_ok_and(|o| origin_allowed(o, host, peer_local, dev_origins))
            },
        ))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
        ])
        // `Authorization`: the iPhone application sends its device token to the home server
        // in `Authorization: Bearer` (a cookie of the server is not sent from its origin).
        .allow_headers([header::CONTENT_TYPE, header::ACCEPT, header::AUTHORIZATION])
        .max_age(Duration::from_secs(600))
}

fn forbidden(msg: &'static str) -> Response {
    (StatusCode::FORBIDDEN, Json(json!({ "error": msg }))).into_response()
}

/// `/api/*` and `/ws`: where `Origin` is checked.
fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/") || path == "/ws" || path.starts_with("/ws/")
}

/// Content security policy of the interface (every answer that is not the API): scripts and
/// frames from this server only — no inline script, no `eval`, no plug-in —, nothing may be
/// framed by another site (`frame-ancestors`), forms and `<base>` stay here.
///
/// What stays open, and why: `style-src 'unsafe-inline'` (styles set by the interface and its
/// libraries; a style cannot run code); pictures and videos from this server, from memory
/// (`data:`, `blob:`: a media read with the device token) and from `http(s):` (the pictures
/// the race schedule cites; a dashboard's media when the page is pointed at another Corde);
/// connections to `http(s):` and `ws(s):` (the « server address » setting lets a page talk to
/// another Corde, and the desktop window talks to its shell).
pub const UI_CSP: &str =
    "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; \
img-src 'self' data: blob: https: http:; media-src 'self' data: blob: https: http:; \
font-src 'self' data:; connect-src 'self' http: https: ws: wss:; frame-src 'self'; \
worker-src 'self' blob:; object-src 'none'; base-uri 'self'; form-action 'self'; \
frame-ancestors 'self'";

/// Content security policy of the API's answers (JSON): if a browser ever showed one as a
/// page, nothing in it could load, run or be framed.
pub const API_CSP: &str = "default-src 'none'; frame-ancestors 'none'";

/// Browser features no page of Corde uses: refused, whatever the page.
pub const PERMISSIONS_POLICY: &str = "camera=(), microphone=(), geolocation=(), payment=(), usb=()";

/// Protections a browser applies when asked to, on **every** answer (refusals included):
///
/// * `X-Frame-Options` + `frame-ancestors`: no page of another site can show the interface
///   in a frame and get the driver to click in it (clickjacking: « enable network access »,
///   « delete this session »…);
/// * `X-Content-Type-Options: nosniff`: an answer is never interpreted as another type than
///   the one announced (JSON run as a script, a media as a page);
/// * `Content-Security-Policy`: [`UI_CSP`] for the interface, [`API_CSP`] for the API (a
///   handler's own policy is kept: a media is served sandboxed);
/// * `Cross-Origin-Resource-Policy: same-origin`: another site cannot embed a file or an
///   answer of this server (`<script src>`, `<img>`) — except a dashboard's media, which the
///   applications show from their own origin;
/// * `Referrer-Policy: no-referrer`: a link to another site never tells where it was clicked;
/// * `Cache-Control: no-store` on the API's answers that say nothing: sessions and names are
///   not kept in a browser's disk cache.
pub async fn security_headers(req: Request, next: Next) -> Response {
    let path = req.uri().path();
    let api = is_api_path(path);
    let media = path.starts_with("/api/dash-assets/");
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::X_FRAME_OPTIONS,
        HeaderValue::from_static("SAMEORIGIN"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        "permissions-policy",
        HeaderValue::from_static(PERMISSIONS_POLICY),
    );
    if !media {
        h.insert(
            "cross-origin-resource-policy",
            HeaderValue::from_static("same-origin"),
        );
    }
    if !h.contains_key(header::CONTENT_SECURITY_POLICY) {
        h.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(if api { API_CSP } else { UI_CSP }),
        );
    }
    if api && !h.contains_key(header::CACHE_CONTROL) {
        h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    res
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
    // No ConnectInfo (a router driven without a socket): not this PC.
    let peer = peer_of(req.extensions());
    let dev = policy.dev_origins;
    // A matched route is the API even if its path were ever to lie outside `/api`.
    let is_api = is_api_path(req.uri().path()) || req.extensions().get::<MatchedPath>().is_some();
    if is_api {
        if let Some(o) = origin {
            if !origin_allowed(o, host, peer.is_some_and(peer_is_local), dev) {
                tracing::debug!("refused: origin {o:?} for {}", req.uri().path());
                return forbidden("origine non autorisée");
            }
        }
    }
    if let Some(route) = req.extensions().get::<MatchedPath>() {
        if !policy.hub
            && route_access(req.method(), route.as_str()) == Access::ThisPcOnly
            && !peer.is_some_and(|p| may_change_settings(p, origin, host, dev))
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
        // From this PC or from another device, nothing changes for these.
        for peer_local in [true, false] {
            for ok in [
                // The UI served by this server (desktop shell, browser of the PC, OBS).
                "http://127.0.0.1:8471",
                // Desktop shell's own pages.
                "tauri://localhost",
                "http://tauri.localhost",
                "https://tauri.localhost",
            ] {
                assert!(origin_allowed(ok, host, peer_local, false), "{ok}");
            }
        }
        // A phone: the page and the API share the PC's address.
        assert!(origin_allowed(
            "http://192.168.1.20:8471",
            Some("192.168.1.20:8471"),
            false,
            false
        ));
        assert!(origin_allowed(
            "http://pc-jeu.local:8471",
            Some("PC-JEU.local:8471"),
            false,
            false
        ));
        assert!(origin_allowed(
            "http://pc-jeu",
            Some("pc-jeu:80"),
            false,
            false
        ));
        // The pages of other programs of this PC: development only.
        for other in [
            "http://127.0.0.1:5173",
            "http://localhost:5173",
            "http://[::1]:4173",
            "http://127.0.0.1:8888",
        ] {
            assert!(!origin_allowed(other, host, true, false), "{other}");
            assert!(origin_allowed(other, host, true, true), "{other} (dev)");
        }
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
            // Only the exact origin of the iPhone application.
            ("capacitor://evil.example", host),
            ("capacitor://localhost:8080", host),
            ("capacitor://localhost@evil.example", host),
            ("ionic://localhost", host),
            ("http://capacitor.localhost", host),
        ] {
            for dev in [false, true] {
                assert!(!origin_allowed(bad, host, true, dev), "{bad:?} on {host:?}");
                assert!(
                    !origin_allowed(bad, host, false, dev),
                    "{bad:?} on {host:?}"
                );
            }
        }
    }

    #[test]
    fn the_iphone_application_is_allowed_but_never_counts_as_this_pc() {
        for host in [
            Some("192.168.1.20:8471"),
            Some("10.8.0.1:8471"),
            Some("pc-jeu.local:8471"),
            None,
        ] {
            for origin in ["capacitor://localhost", "Capacitor://LOCALHOST/"] {
                assert!(origin_allowed(origin, host, false, false), "{host:?}");
            }
        }
        // Even from the loopback (which a phone never is), its pages cannot change the settings.
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        for dev in [false, true] {
            assert!(!may_change_settings(
                lo,
                Some("capacitor://localhost"),
                Some("127.0.0.1:8471"),
                dev
            ));
        }
    }

    /// The Android application serves its pages from `http://localhost` (Capacitor's
    /// `server.androidScheme` set to `http` in apps/mobile/capacitor.config.json, ADR 0013):
    /// accepted from a phone, with no right of "this PC". From this very machine the same
    /// origin is a web server of this PC on port 80 — another program's pages: refused.
    #[test]
    fn the_android_application_is_allowed_but_never_counts_as_this_pc() {
        for host in [
            Some("192.168.1.20:8471"),
            Some("10.8.0.1:8471"),
            Some("pc-jeu.local:8471"),
            None,
        ] {
            assert!(
                origin_allowed("http://localhost", host, false, false),
                "{host:?}"
            );
        }
        let phone: IpAddr = "192.168.1.42".parse().unwrap();
        let vpn: IpAddr = "10.8.0.2".parse().unwrap();
        for dev in [false, true] {
            for peer in [phone, vpn] {
                assert!(!may_change_settings(
                    peer,
                    Some("http://localhost"),
                    Some("192.168.1.20:8471"),
                    dev
                ));
            }
        }
        // On this PC, `http://localhost` is whatever serves port 80 here.
        let lo: IpAddr = "127.0.0.1".parse().unwrap();
        let host = Some("127.0.0.1:8471");
        assert!(!origin_allowed("http://localhost", host, true, false));
        assert!(!may_change_settings(
            lo,
            Some("http://localhost"),
            host,
            false
        ));
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
            // Race dashboards: exactly the rule of the layouts (ADR 0015).
            (Method::GET, "/api/dashes", Open),
            (Method::GET, "/api/dashes/{id}", Open),
            (Method::PUT, "/api/dashes/{id}", Open),
            (Method::DELETE, "/api/dashes", ThisPcOnly),
            (Method::DELETE, "/api/dashes/{id}", ThisPcOnly),
            (Method::GET, "/api/dash-assets", Open),
            (Method::GET, "/api/dash-assets/{id}", Open),
            (Method::POST, "/api/dash-assets", ThisPcOnly),
            (Method::DELETE, "/api/dash-assets/{id}", ThisPcOnly),
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
            (Method::POST, "/api/races/picks/sync", Open),
            (Method::GET, "/api/races/settings", Open),
            (Method::PUT, "/api/races/settings", Open),
        ];
        for (m, r, want) in cases {
            assert_eq!(route_access(&m, r), want, "{m} {r}");
        }
        for (m, r) in [
            (Method::PUT, "/api/hub-link"),
            (Method::DELETE, "/api/hub-link"),
            (Method::POST, "/api/hub-link/pair"),
            (Method::POST, "/api/hub-link/sync"),
            (Method::POST, "/api/hub-link/resend"),
        ] {
            assert_eq!(route_access(&m, r), ThisPcOnly, "{m} {r}");
        }
        assert_eq!(route_access(&Method::GET, "/api/hub-link"), Open);
        // Closed by default: a route that changes something and is not listed as open — one
        // added later and forgotten in the table — is reserved to this PC.
        for (m, r) in [
            (Method::POST, "/api/some-new-route"),
            (Method::PUT, "/api/some-new-route/{id}"),
            (Method::PATCH, "/api/layouts/{id}"),
            (Method::DELETE, "/api/races/picks"),
            (Method::POST, "/api/layouts"),
            (Method::from_bytes(b"PURGE").unwrap(), "/api/sessions"),
        ] {
            assert_eq!(route_access(&m, r), ThisPcOnly, "{m} {r}");
        }
        for (m, r) in [
            (Method::GET, "/api/some-new-route"),
            (Method::HEAD, "/api/sessions"),
        ] {
            assert_eq!(route_access(&m, r), Open, "{m} {r}");
        }
    }

    #[test]
    fn hub_route_classes() {
        use HubAccess::*;
        let cases = [
            (Method::GET, "/api/hub/pairing", Public),
            (Method::POST, "/api/hub/pair", Public),
            (Method::POST, "/api/hub/local/pairing-code", LocalControl),
            (Method::GET, "/api/hub/local/devices", LocalControl),
            (Method::DELETE, "/api/hub/local/devices/{id}", LocalControl),
            (Method::GET, "/api/health", Device),
            (Method::GET, "/api/sessions", Device),
            (Method::GET, "/ws", Device),
            (Method::GET, "/api/hub/devices", Device),
            (Method::PATCH, "/api/hub/devices/{id}", Device),
            (Method::DELETE, "/api/hub/devices/{id}", Device),
            (Method::POST, "/api/sync/hello", Device),
            (Method::PUT, "/api/sync/sessions/{id}", Device),
            (Method::POST, "/api/sessions/{id}/feedback", Device),
            (Method::PUT, "/api/races/picks/{id}", Device),
            (Method::POST, "/api/races/picks/sync", Device),
            (Method::DELETE, "/api/sessions/{id}", Admin),
            (Method::POST, "/api/sessions/{id}/analysis/recompute", Admin),
            (Method::PUT, "/api/settings", Admin),
            (Method::DELETE, "/api/layouts", Admin),
            (Method::GET, "/api/dashes", Device),
            (Method::PUT, "/api/dashes/{id}", Device),
            (Method::DELETE, "/api/dashes", Admin),
            (Method::DELETE, "/api/dashes/{id}", Admin),
            (Method::GET, "/api/dash-assets/{id}", Device),
            (Method::POST, "/api/dash-assets", Admin),
            (Method::DELETE, "/api/dash-assets/{id}", Admin),
            (Method::GET, "/api/does-not-exist", Device),
        ];
        for (m, r, want) in cases {
            assert_eq!(hub_route_access(&m, r), want, "{m} {r}");
        }
        for (m, r) in [
            (Method::POST, "/api/sessions/{id}/setup-files"),
            (Method::POST, "/api/profile/import"),
            (Method::POST, "/api/recording/start"),
            (Method::POST, "/api/recording/stop"),
        ] {
            assert!(matches!(hub_route_access(&m, r), Refused(_)), "{m} {r}");
        }
    }

    // --- the middleware on a small router ------------------------------------

    fn app(lan: bool, peer: &str) -> Router {
        app_with(lan, peer, false)
    }

    /// The same router started for development (`--dev-origins`).
    fn dev_app(lan: bool, peer: &str) -> Router {
        app_with(lan, peer, true)
    }

    fn app_with(lan: bool, peer: &str, dev: bool) -> Router {
        let policy = Arc::new(HostPolicy::new(lan, &names()).with_dev_origins(dev));
        let peer: SocketAddr = peer.parse().unwrap();
        Router::new()
            .route("/api/health", get(|| async { "ok" }))
            .route(
                "/api/sessions/{id}",
                delete(|| async { StatusCode::NO_CONTENT }),
            )
            .route("/api/layouts", put(|| async { "stored" }))
            .route("/api/some-new-route", post(|| async { "changed" }))
            .route(
                "/api/boom",
                get(|| async {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": "cannot open C:\\Users\\someone\\x" })),
                    )
                }),
            )
            .route("/ws", get(|| async { "ws" }))
            .fallback(|| async { "index" })
            .layer(cors_layer(dev))
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
        // The page of another program of this PC (another loopback port): a site like any
        // other, reading included — unless the server was started for development.
        let other = ("origin", "http://localhost:5173");
        let r = send(
            app(false, local),
            Method::GET,
            "/api/health",
            &[host, other],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        assert!(r.headers().get("access-control-allow-origin").is_none());
        let r = send(app(false, local), Method::GET, "/ws", &[host, other]).await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "WebSocket upgrade");
        let r = send(
            dev_app(false, local),
            Method::GET,
            "/api/health",
            &[host, other],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "http://localhost:5173"
        );
        // The interface this server gave: its own origin, echoed.
        let own = ("origin", "http://127.0.0.1:8471");
        let r = send(app(false, local), Method::GET, "/api/health", &[host, own]).await;
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "http://127.0.0.1:8471"
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
        assert_eq!(r.status(), StatusCode::FORBIDDEN, "another program's page");
        let r = send(dev_app(false, local), Method::OPTIONS, "/api/layouts", &pre).await;
        assert!(r.status().is_success(), "{}", r.status());
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "http://127.0.0.1:5173"
        );
        let mut bad = pre;
        bad[1] = ("origin", "https://evil.example");
        for a in [app(false, local), dev_app(false, local)] {
            let r = send(a, Method::OPTIONS, "/api/layouts", &bad).await;
            assert_eq!(r.status(), StatusCode::FORBIDDEN);
        }

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
        // Another program's page of this PC: no right of this PC either.
        let r = send(
            dev_app(true, local),
            Method::DELETE,
            "/api/sessions/x",
            &[host, ("origin", "http://127.0.0.1:5173")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::NO_CONTENT, "development only");
        let r = send(
            app(true, local),
            Method::DELETE,
            "/api/sessions/x",
            &[host, ("origin", "http://127.0.0.1:5173")],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        // A route that changes something and that nobody listed: this PC only.
        let r = send(
            app(true, phone),
            Method::POST,
            "/api/some-new-route",
            &[lan_host, same],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        let r = send(app(true, local), Method::POST, "/api/some-new-route", &[]).await;
        assert_eq!(r.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn the_iphone_application_reads_the_api_but_not_the_pc_only_routes() {
        let phone = "192.168.1.42:50000";
        let lan_host = ("host", "192.168.1.20:8471");
        let app_origin = ("origin", "capacitor://localhost");

        let r = send(
            app(true, phone),
            Method::GET,
            "/api/health",
            &[lan_host, app_origin],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "capacitor://localhost"
        );
        let r = send(
            app(true, phone),
            Method::GET,
            "/ws",
            &[lan_host, app_origin],
        )
        .await;
        assert_eq!(r.status(), StatusCode::OK, "WebSocket upgrade");
        // The preflight of a request that carries the device token.
        let pre = [
            lan_host,
            app_origin,
            ("access-control-request-method", "PUT"),
            (
                "access-control-request-headers",
                "authorization,content-type",
            ),
        ];
        let r = send(app(true, phone), Method::OPTIONS, "/api/layouts", &pre).await;
        assert!(r.status().is_success(), "{}", r.status());
        assert_eq!(
            r.headers().get("access-control-allow-origin").unwrap(),
            "capacitor://localhost"
        );
        let allowed = r
            .headers()
            .get("access-control-allow-headers")
            .unwrap()
            .to_str()
            .unwrap()
            .to_ascii_lowercase();
        assert!(allowed.contains("authorization"), "{allowed}");
        // The routes of the PC stay closed to it.
        let r = send(
            app(true, phone),
            Method::DELETE,
            "/api/sessions/x",
            &[lan_host, app_origin],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
        // With the LAN closed the phone cannot reach the server at all (Host refused).
        let r = send(
            app(false, phone),
            Method::GET,
            "/api/health",
            &[lan_host, app_origin],
        )
        .await;
        assert_eq!(r.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn server_errors_lose_their_details_on_the_lan_only() {
        let r = send(app(true, "192.168.1.42:1"), Method::GET, "/api/boom", &[]).await;
        assert_eq!(r.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let text = body_text(r).await;
        assert!(!text.contains("Users"), "{text}");
        assert!(text.contains("journal du PC"), "{text}");

        let r = send(app(true, "127.0.0.1:1"), Method::GET, "/api/boom", &[]).await;
        assert!(
            body_text(r).await.contains("Users"),
            "the PC keeps the details"
        );
    }
}
