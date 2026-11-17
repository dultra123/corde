//! Network helpers: is the server reachable from the local network, at which
//! addresses, and the start-up banner that says so.

use crate::config::BindOrigin;
use crate::settings::Settings;
use serde::Serialize;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

/// IPv4 addresses of this machine usable from another device on the LAN
/// (no loopback, no link-local 169.254/16), as `(interface, ip)`, sorted so
/// that private ranges come first.
pub fn lan_ipv4() -> Vec<(String, Ipv4Addr)> {
    let mut out: Vec<(String, Ipv4Addr)> = match local_ip_address::list_afinet_netifas() {
        Ok(list) => list
            .into_iter()
            .filter_map(|(name, ip)| match ip {
                IpAddr::V4(v4)
                    if !v4.is_loopback() && !v4.is_link_local() && !v4.is_unspecified() =>
                {
                    Some((name, v4))
                }
                _ => None,
            })
            .collect(),
        Err(e) => {
            tracing::debug!("cannot enumerate interfaces: {e}");
            Vec::new()
        }
    };
    out.sort_by_key(|(name, ip)| (!ip.is_private(), name.clone()));
    out.dedup_by_key(|(_, ip)| *ip);
    out
}

/// `true` when a socket bound to `ip` accepts connections from other devices
/// (every interface, or one that is not the loopback).
pub fn listens_on_lan(ip: IpAddr) -> bool {
    ip.is_unspecified() || !ip.is_loopback()
}

/// The addresses another device of the LAN can type, as `(interface, url)`.
/// Empty when the server only listens on this PC.
fn lan_endpoints(bound: SocketAddr) -> Vec<(String, String)> {
    let port = bound.port();
    match bound.ip() {
        ip if !listens_on_lan(ip) => Vec::new(),
        ip if ip.is_unspecified() => lan_ipv4()
            .into_iter()
            .map(|(name, ip)| (name, format!("http://{ip}:{port}")))
            .collect(),
        IpAddr::V4(v4) => vec![("bind".to_string(), format!("http://{v4}:{port}"))],
        IpAddr::V6(v6) => vec![("bind".to_string(), format!("http://[{v6}]:{port}"))],
    }
}

/// `http://<ip>:<port>` for every address a phone of the LAN can use; empty
/// when the server only listens on this PC.
pub fn lan_urls(bound: SocketAddr) -> Vec<String> {
    lan_endpoints(bound)
        .into_iter()
        .map(|(_, url)| url)
        .collect()
}

/// `GET /api/network`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkInfo {
    /// The saved setting (`lan_access` of `settings.json`): what the next
    /// start will do, unless `--bind` / `--lan` decide otherwise.
    pub lan_access_setting: bool,
    /// What this process is doing right now.
    pub listening_on_lan: bool,
    pub port: u16,
    /// Addresses to open on the phone, private ranges first.
    pub urls: Vec<String>,
}

pub fn network_info(bound: SocketAddr, settings: &Settings) -> NetworkInfo {
    NetworkInfo {
        lan_access_setting: settings.lan_access,
        listening_on_lan: listens_on_lan(bound.ip()),
        port: bound.port(),
        urls: lan_urls(bound),
    }
}

/// `true` for a connection made from this machine (loopback, IPv4-mapped
/// loopback included).
pub fn peer_is_local(peer: IpAddr) -> bool {
    match peer {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    }
}

/// What the `Origin` header of a request says about the page that sent it. A browser adds it
/// to every cross-origin request and to every `POST` / `PUT` / `DELETE`; a program (curl, the
/// desktop shell's own code, a script) usually sends none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OriginKind {
    /// No `Origin`: a program, or a plain visit.
    None,
    /// A page this very server gave: `http://<Host>` (same host and port as the request).
    ThisServer,
    /// The desktop shell's own pages: `tauri://localhost`, `http(s)://tauri.localhost`.
    Shell,
    /// The iPhone application ([`IOS_APP_ORIGIN`]).
    IosApp,
    /// The Android application ([`ANDROID_APP_ORIGIN`]).
    AndroidApp,
    /// A page served by **another** program of the machine that runs the browser, on another
    /// loopback port (`http://127.0.0.1:<port>`, `http://localhost:<port>`, `http://[::1]:<port>`):
    /// a development server, a capture script — or any other software's local web page.
    OtherLocal,
    /// Any other page: refused.
    Foreign,
}

/// The kind of page behind `origin`, for a request sent to `host` (its `Host` header).
pub fn origin_kind(origin: Option<&str>, host: Option<&str>) -> OriginKind {
    let Some(origin) = origin else {
        return OriginKind::None;
    };
    if origin_is_shell(origin) {
        OriginKind::Shell
    } else if host.is_some_and(|h| same_origin(origin, h)) {
        OriginKind::ThisServer
    } else if origin_is_ios_app(origin) {
        OriginKind::IosApp
    } else if origin_is_android_app(origin) {
        OriginKind::AndroidApp
    } else if origin_is_loopback(origin) {
        OriginKind::OtherLocal
    } else {
        OriginKind::Foreign
    }
}

/// May a page of this kind call the API at all (`/api/*`, `/ws`)?
///
/// * the server's own pages, the desktop shell, the two applications, a program: yes;
/// * the Android application's origin is `http://localhost`. From a phone that is the
///   application; from this very machine (`peer_local`) it is a web server of this PC on
///   port 80 — another program's pages — and is treated as such;
/// * **another program's local pages are refused**: every piece of software that serves a web
///   page on the loopback (and whatever content it shows there) would otherwise hold all the
///   rights of this PC on Corde. `dev_origins` (`--dev-origins`, development and capture
///   scripts only) accepts them again;
/// * anything else: no.
pub fn origin_may_call(kind: OriginKind, peer_local: bool, dev_origins: bool) -> bool {
    match kind {
        OriginKind::None | OriginKind::ThisServer | OriginKind::Shell | OriginKind::IosApp => true,
        OriginKind::AndroidApp => !peer_local || dev_origins,
        OriginKind::OtherLocal => dev_origins,
        OriginKind::Foreign => false,
    }
}

/// Who may change the settings (and call the other "this PC only" routes, see
/// [`crate::guard::route_access`]): a program of this PC only. The peer must be the loopback,
/// and when a browser sent the request, the page must be this server's own (`http://<Host>`:
/// the Corde UI opened on this PC) or the desktop shell's. A phone of the LAN, even through
/// the UI this server gave it, is refused; so is the page of another program of this PC
/// (unless `dev_origins`), and so are the applications, wherever they seem to connect from.
pub fn may_change_settings(
    peer: IpAddr,
    origin: Option<&str>,
    host: Option<&str>,
    dev_origins: bool,
) -> bool {
    peer_is_local(peer)
        && match origin_kind(origin, host) {
            OriginKind::None | OriginKind::ThisServer | OriginKind::Shell => true,
            OriginKind::OtherLocal | OriginKind::AndroidApp => dev_origins,
            OriginKind::IosApp | OriginKind::Foreign => false,
        }
}

/// The page comes from this very server: `http://<Host>`, same host and same port.
pub fn same_origin(origin: &str, host: &str) -> bool {
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

/// The desktop shell's own pages (`apps/desktop/dist`): `tauri://localhost` on macOS and
/// Linux, `http(s)://tauri.localhost` on Windows. No web page can carry these origins.
pub fn origin_is_shell(origin: &str) -> bool {
    let Some((scheme, authority)) = split_origin(origin) else {
        return false;
    };
    matches!(
        (scheme.as_str(), authority.as_str()),
        ("tauri", "localhost") | ("http" | "https", "tauri.localhost")
    )
}

/// A page served on the loopback of the machine that runs the browser:
/// `http://127.0.0.1:<port>`, `http://localhost:<port>`, `http://[::1]:<port>`, any port.
pub fn origin_is_loopback(origin: &str) -> bool {
    let Some((scheme, authority)) = split_origin(origin) else {
        return false;
    };
    let Some((host, _)) = split_host_port(&authority) else {
        return false;
    };
    scheme == "http"
        && (host == "localhost" || parse_ip_host(&host).is_some_and(|ip| ip.is_loopback()))
}

/// Origin of the pages of the Corde iPhone application (`apps/mobile`, Capacitor): the
/// webview serves the bundled interface from `capacitor://localhost` (Capacitor's default
/// `server.iosScheme` and `server.hostname`, left unchanged in `capacitor.config.json`).
///
/// Accepted by the `Origin` check and by CORS ([`origin_may_call`]), **never** as a page of
/// this PC: the application runs on a phone, so the "this PC only" routes stay closed to it
/// ([`may_change_settings`]). Accepting it adds nothing a native program could not already
/// do — a program is not a browser, it can send any `Origin`, or none, which is accepted —
/// while a web page opened in a browser can never carry it.
pub const IOS_APP_ORIGIN: &str = "capacitor://localhost";
/// The former name of [`IOS_APP_ORIGIN`].
pub const MOBILE_APP_ORIGIN: &str = IOS_APP_ORIGIN;

/// Origin of the pages of the Corde Android application: `http://localhost`, without a port
/// (Capacitor's `server.androidScheme` set to `http` in `capacitor.config.json`, ADR 0013).
pub const ANDROID_APP_ORIGIN: &str = "http://localhost";

/// `true` for the origin of the Corde iPhone application ([`IOS_APP_ORIGIN`]).
pub fn origin_is_ios_app(origin: &str) -> bool {
    match split_origin(origin) {
        Some((scheme, authority)) => scheme == "capacitor" && authority == "localhost",
        None => false,
    }
}

/// `true` for the origin of the Corde Android application ([`ANDROID_APP_ORIGIN`]): exactly
/// `http://localhost`, no port.
pub fn origin_is_android_app(origin: &str) -> bool {
    match split_origin(origin) {
        Some((scheme, authority)) => scheme == "http" && authority == "localhost",
        None => false,
    }
}

/// `true` for the origin of one of the two Corde applications.
pub fn origin_is_mobile_app(origin: &str) -> bool {
    origin_is_ios_app(origin) || origin_is_android_app(origin)
}

/// `("http", "pc-jeu:8471")` from `http://PC-Jeu:8471`, lower case. `None`
/// when it is not `scheme://authority` (with nothing after but an optional
/// `/`), or the authority carries credentials.
pub(crate) fn split_origin(origin: &str) -> Option<(String, String)> {
    let origin = origin.trim().to_ascii_lowercase();
    let (scheme, rest) = origin.split_once("://")?;
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    if scheme.is_empty()
        || authority.is_empty()
        || authority.contains(['/', '?', '#', '@', ' '])
        || !scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    {
        return None;
    }
    Some((scheme.to_string(), authority.to_string()))
}

/// `("192.168.1.20", Some(8471))`, `("::1", None)`, `("localhost", None)`
/// from an authority (`Host` header or origin), host in lower case. `None`
/// when malformed (bad port, unclosed bracket, empty host).
pub(crate) fn split_host_port(authority: &str) -> Option<(String, Option<u16>)> {
    let a = authority.trim().to_ascii_lowercase();
    let (host, port) = if let Some(v6) = a.strip_prefix('[') {
        // [::1]:8471 — only an IPv6 literal goes between brackets.
        let (host, rest) = v6.split_once(']')?;
        host.parse::<std::net::Ipv6Addr>().ok()?;
        let port = match rest {
            "" => None,
            p => Some(p.strip_prefix(':')?),
        };
        (host.to_string(), port)
    } else {
        let (h, p) = match a.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p)),
            None => (a.clone(), None),
        };
        // A bare IPv6 literal (no brackets) is not a valid authority.
        if h.contains(':') {
            return None;
        }
        (h, p)
    };
    if host.is_empty() {
        return None;
    }
    let port = match port {
        None => None,
        Some(p) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => Some(p.parse().ok()?),
        Some(_) => return None,
    };
    Some((host, port))
}

/// An IP literal host (`192.168.1.20`, or `::1` taken out of its brackets).
pub(crate) fn parse_ip_host(host: &str) -> Option<IpAddr> {
    host.parse::<IpAddr>().ok()
}

/// Lines of the start-up banner.
pub fn banner_lines(
    bound: SocketAddr,
    origin: BindOrigin,
    source: &str,
    ui: Option<&std::path::Path>,
    data_dir: &std::path::Path,
) -> Vec<String> {
    let port = bound.port();
    let mut lines = vec![
        format!(
            "corde-server {} — source : {source}",
            crate::state::SERVER_VERSION
        ),
        format!("  Local  : http://127.0.0.1:{port}   (WebSocket ws://127.0.0.1:{port}/ws)"),
    ];
    if listens_on_lan(bound.ip()) {
        let why = match origin {
            BindOrigin::Explicit => format!("--bind {}", bound.ip()),
            BindOrigin::LanFlag => "--lan".to_string(),
            BindOrigin::Setting | BindOrigin::Default | BindOrigin::Hub => {
                "réglage « accès depuis mon téléphone »".to_string()
            }
        };
        lines.push(format!(
            "  Réseau : OUVERT au réseau local ({why}) — Windows peut demander d'autoriser Corde sur les réseaux privés."
        ));
        let endpoints = lan_endpoints(bound);
        if endpoints.is_empty() {
            lines.push(
                "  LAN    : aucune adresse IPv4 trouvée (le PC est-il connecté au réseau ?)"
                    .to_string(),
            );
        }
        for (name, url) in endpoints {
            lines.push(format!(
                "  LAN    : {url}   ({name}) ← téléphone / autre PC"
            ));
        }
    } else {
        let why = match origin {
            BindOrigin::Explicit => format!(" (--bind {})", bound.ip()),
            _ => String::new(),
        };
        lines.push(format!(
            "  Réseau : FERMÉ{why} — seul ce PC peut ouvrir Corde (pas d'invite du pare-feu Windows)."
        ));
        lines.push(
            "           Pour ton téléphone : bouton « Activer l'accès depuis mon téléphone » de l'écran"
                .to_string(),
        );
        lines.push("           d'accueil puis redémarre Corde, ou relance avec --lan.".to_string());
    }
    lines.push(match ui {
        Some(dir) => format!("  UI     : {}", dir.display()),
        None => "  UI     : aucune (API seule — --ui-dir <dossier>)".to_string(),
    });
    lines.push(format!("  Data   : {}", data_dir.display()));
    lines
}

/// `true` inside a Docker / Podman container (their marker files): the addresses this
/// process sees are then internal to the container, not the host's.
pub fn in_container() -> bool {
    ["/.dockerenv", "/run/.containerenv"]
        .iter()
        .any(|p| std::path::Path::new(p).exists())
}

/// The command that prints a pairing code, as its owner types it where the server runs: with
/// `sudo` on a machine set up by the installer (the data belong to the `corde` account),
/// without it in a container (its terminal is root already, and has no `sudo`).
pub fn pair_command() -> &'static str {
    pair_command_for(in_container())
}

pub(crate) fn pair_command_for(container: bool) -> &'static str {
    if container {
        "corde-server pair"
    } else {
        "sudo corde-server pair"
    }
}

/// Where and how to get a pairing code, for a message shown to the person pairing a device:
/// « sudo corde-server pair » sur le serveur, or « corde-server pair » dans le terminal du
/// conteneur.
pub fn pair_hint() -> String {
    pair_hint_for(in_container())
}

pub(crate) fn pair_hint_for(container: bool) -> String {
    let command = pair_command_for(container);
    if container {
        format!("« {command} » dans le terminal du conteneur")
    } else {
        format!("« {command} » sur le serveur")
    }
}

/// Lines of the start-up banner of the home server (`--role hub`).
pub fn hub_banner_lines(
    bound: SocketAddr,
    ui: Option<&std::path::Path>,
    data_dir: &std::path::Path,
) -> Vec<String> {
    let mut lines = vec![format!(
        "corde-server {} — serveur Corde (rôle hub)",
        crate::state::SERVER_VERSION
    )];
    let endpoints = lan_endpoints(bound);
    if endpoints.is_empty() {
        lines.push(format!(
            "  Adresse : http://127.0.0.1:{}   (aucune adresse réseau trouvée)",
            bound.port()
        ));
    }
    for (name, url) in endpoints {
        lines.push(format!("  Adresse : {url}   ({name})"));
    }
    if in_container() {
        lines.push(
            "            (dans un conteneur : adresse interne, utilise celle de la machine hôte, ex. le NAS)"
                .to_string(),
        );
    }
    lines.push(format!(
        "  Accès   : appareils appairés seulement. Code d'appairage : {}",
        pair_command()
    ));
    lines.push(
        "            Ne redirige jamais ce port depuis ta box : l'accès extérieur passe par un VPN."
            .to_string(),
    );
    lines.push(match ui {
        Some(dir) => format!("  UI      : {}", dir.display()),
        None => "  UI      : aucune (API seule — --ui-dir <dossier>)".to_string(),
    });
    lines.push(format!("  Data    : {}", data_dir.display()));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pairing_command_fits_where_the_server_runs() {
        // A machine set up by the installer: the data belong to the `corde` account.
        assert_eq!(pair_command_for(false), "sudo corde-server pair");
        assert_eq!(
            pair_hint_for(false),
            "« sudo corde-server pair » sur le serveur"
        );
        // A container: its terminal is root and has no `sudo`.
        assert_eq!(pair_command_for(true), "corde-server pair");
        assert_eq!(
            pair_hint_for(true),
            "« corde-server pair » dans le terminal du conteneur"
        );
    }

    #[test]
    fn the_hub_banner_names_the_pairing_command() {
        let lines = hub_banner_lines(
            "0.0.0.0:8471".parse().unwrap(),
            None,
            std::path::Path::new("/var/lib/corde"),
        );
        assert!(lines[0].contains("hub"));
        assert!(lines.iter().any(|l| l.contains("corde-server pair")));
        assert!(lines.iter().any(|l| l.contains("VPN")));
    }

    #[test]
    fn banner_says_whether_the_lan_is_open_and_how_to_open_it() {
        let dir = std::path::Path::new("/tmp/x");
        let open = banner_lines(
            "0.0.0.0:8471".parse().unwrap(),
            BindOrigin::LanFlag,
            "sim",
            None,
            dir,
        );
        assert!(open[1].contains("http://127.0.0.1:8471"));
        assert!(open
            .iter()
            .any(|l| l.contains("OUVERT") && l.contains("--lan")));
        assert!(open.iter().any(|l| l.contains("LAN")));

        let closed = banner_lines(
            "127.0.0.1:1".parse().unwrap(),
            BindOrigin::Default,
            "sim",
            None,
            dir,
        );
        assert!(closed.iter().any(|l| l.contains("FERMÉ")));
        assert!(closed.iter().any(|l| l.contains("--lan")));
        assert!(closed.iter().any(|l| l.contains("Activer l'accès")));
        assert!(!closed.iter().any(|l| l.contains("LAN    :")));

        let pinned = banner_lines(
            "192.168.1.20:8471".parse().unwrap(),
            BindOrigin::Explicit,
            "sim",
            None,
            dir,
        );
        assert!(pinned
            .iter()
            .any(|l| l.contains("OUVERT") && l.contains("--bind 192.168.1.20")));
        assert!(pinned
            .iter()
            .any(|l| l.contains("http://192.168.1.20:8471")));
    }

    #[test]
    fn network_info_lists_urls_only_when_listening_on_the_lan() {
        let on = Settings {
            lan_access: true,
            ..Default::default()
        };
        // Setting saved, but this process still listens on the loopback.
        let info = network_info("127.0.0.1:8471".parse().unwrap(), &on);
        assert!(info.lan_access_setting && !info.listening_on_lan);
        assert_eq!(info.port, 8471);
        assert!(info.urls.is_empty());

        let info = network_info("192.168.1.20:9000".parse().unwrap(), &Settings::default());
        assert!(!info.lan_access_setting && info.listening_on_lan);
        assert_eq!(info.urls, ["http://192.168.1.20:9000"]);

        let info = network_info("[::1]:8471".parse().unwrap(), &on);
        assert!(!info.listening_on_lan && info.urls.is_empty());
        let info = network_info("[fd00::20]:8471".parse().unwrap(), &on);
        assert_eq!(info.urls, ["http://[fd00::20]:8471"]);

        // Every interface: whatever this machine has, well formed.
        let info = network_info("0.0.0.0:8471".parse().unwrap(), &on);
        assert!(info.listening_on_lan);
        for url in &info.urls {
            assert!(
                url.starts_with("http://") && url.ends_with(":8471"),
                "{url}"
            );
            assert!(
                !url.contains("127.0.0.1") && !url.contains("0.0.0.0"),
                "{url}"
            );
        }
    }

    #[test]
    fn only_this_pc_may_change_the_settings() {
        let local: IpAddr = "127.0.0.1".parse().unwrap();
        let phone: IpAddr = "192.168.1.42".parse().unwrap();
        let host = Some("127.0.0.1:8471");
        // No Origin: curl, the desktop shell, a script on this PC.
        assert!(may_change_settings(local, None, host, false));
        assert!(may_change_settings(
            "::1".parse().unwrap(),
            None,
            host,
            false
        ));
        assert!(may_change_settings(
            "::ffff:127.0.0.1".parse().unwrap(),
            None,
            host,
            false
        ));
        // The UI served by this server (same host, same port), the Tauri webview.
        for (origin, host) in [
            ("http://127.0.0.1:8471", "127.0.0.1:8471"),
            ("http://localhost:8471", "localhost:8471"),
            ("http://LOCALHOST:8471", "localhost:8471"),
            ("http://[::1]:8471", "[::1]:8471"),
            ("tauri://localhost", "127.0.0.1:8471"),
            ("https://tauri.localhost", "127.0.0.1:8471"),
            ("http://tauri.localhost", "127.0.0.1:8471"),
        ] {
            assert!(
                may_change_settings(local, Some(origin), Some(host), false),
                "{origin}"
            );
        }
        // Another device, even through the UI this server gave it.
        assert!(!may_change_settings(phone, None, host, false));
        assert!(!may_change_settings(
            phone,
            Some("http://192.168.1.20:8471"),
            Some("192.168.1.20:8471"),
            false
        ));
        // A web page opened on this PC (plain cross-site, or DNS rebinding).
        for origin in [
            "https://evil.example",
            "http://evil.example:8471",
            "http://127.0.0.1.evil.example",
            "http://localhost.evil.example:8471",
            "http://127.0.0.1:8471@evil.example",
            "http://192.168.1.20:8471",
            "null",
            "",
        ] {
            for dev in [false, true] {
                assert!(
                    !may_change_settings(local, Some(origin), host, dev),
                    "{origin:?}"
                );
            }
        }
    }

    /// The page of another program of this PC — any software that serves a web page on the
    /// loopback, and whatever content it shows there — is not Corde's: no right at all,
    /// unless the server was started for development.
    #[test]
    fn another_program_of_this_pc_is_not_this_server() {
        let local: IpAddr = "127.0.0.1".parse().unwrap();
        let host = Some("127.0.0.1:8471");
        for origin in [
            "http://127.0.0.1:8888",
            "http://localhost:5173",
            "http://[::1]:4173",
            "http://127.0.0.1",
            // Port 80 of this PC: also the origin of the Android application.
            "http://localhost",
            // Another name of the loopback than the one the request was sent to.
            "http://localhost:8471",
        ] {
            let kind = origin_kind(Some(origin), host);
            assert!(
                matches!(kind, OriginKind::OtherLocal | OriginKind::AndroidApp),
                "{origin}: {kind:?}"
            );
            assert!(!origin_may_call(kind, true, false), "{origin}");
            assert!(
                !may_change_settings(local, Some(origin), host, false),
                "{origin}"
            );
            // Development (`--dev-origins`): as before.
            assert!(origin_may_call(kind, true, true), "{origin}");
            assert!(
                may_change_settings(local, Some(origin), host, true),
                "{origin}"
            );
        }
        // The same server reached by the same name is itself.
        assert_eq!(
            origin_kind(Some("http://localhost:8471"), Some("localhost:8471")),
            OriginKind::ThisServer
        );
    }

    #[test]
    fn kinds_of_origin() {
        use OriginKind as K;
        let host = Some("192.168.1.20:8471");
        for (origin, kind) in [
            (None, K::None),
            (Some("http://192.168.1.20:8471"), K::ThisServer),
            (Some("http://192.168.1.20:8471/"), K::ThisServer),
            (Some("tauri://localhost"), K::Shell),
            (Some("https://tauri.localhost"), K::Shell),
            (Some("capacitor://localhost"), K::IosApp),
            (Some("Capacitor://LOCALHOST/"), K::IosApp),
            (Some("http://localhost"), K::AndroidApp),
            (Some("http://localhost:80"), K::OtherLocal),
            (Some("http://127.0.0.1:5173"), K::OtherLocal),
            (Some("https://192.168.1.20:8471"), K::Foreign),
            (Some("http://192.168.1.20:9999"), K::Foreign),
            (Some("https://localhost"), K::Foreign),
            (Some("capacitor://localhost:8080"), K::Foreign),
            (Some("tauri://evil.example"), K::Foreign),
            (Some("https://evil.example"), K::Foreign),
            (Some("null"), K::Foreign),
            (Some(""), K::Foreign),
        ] {
            assert_eq!(origin_kind(origin, host), kind, "{origin:?}");
        }
        // Who may call the API.
        for peer_local in [false, true] {
            assert!(origin_may_call(K::None, peer_local, false));
            assert!(origin_may_call(K::ThisServer, peer_local, false));
            assert!(origin_may_call(K::Shell, peer_local, false));
            assert!(origin_may_call(K::IosApp, peer_local, false));
            assert!(!origin_may_call(K::Foreign, peer_local, false));
            assert!(!origin_may_call(K::Foreign, peer_local, true));
            assert!(!origin_may_call(K::OtherLocal, peer_local, false));
        }
        // The Android application: from a phone, never from this very machine.
        assert!(origin_may_call(K::AndroidApp, false, false));
        assert!(!origin_may_call(K::AndroidApp, true, false));
    }
}
