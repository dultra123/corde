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

/// Who may change the settings: a program of this PC only. The peer must be
/// the loopback, and when a browser sent the request (it adds `Origin` to
/// every cross-origin one, and to every `PUT`), the page must itself come
/// from this PC — the Corde UI, the desktop shell or a dev server. A web page
/// from anywhere else cannot open the server to the network behind the
/// user's back, even though the read API is CORS-permissive.
pub fn may_change_settings(peer: IpAddr, origin: Option<&str>) -> bool {
    let peer_is_local = match peer {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|m| m.is_loopback()),
    };
    peer_is_local && origin.is_none_or(origin_is_local)
}

/// `http://127.0.0.1:8471`, `http://localhost:5173`, `http://[::1]:8471`,
/// `tauri://localhost`, `https://tauri.localhost`… — any port, any scheme.
fn origin_is_local(origin: &str) -> bool {
    let Some((_, rest)) = origin.trim().split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let host = match authority.strip_prefix('[') {
        // [::1]:8471
        Some(v6) => v6.split(']').next().unwrap_or(""),
        None => authority.rsplit_once(':').map_or(authority, |(h, _)| h),
    };
    let host = host.to_ascii_lowercase();
    match host.as_str() {
        "localhost" | "tauri.localhost" => true,
        other => other.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
    }
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
            BindOrigin::Setting | BindOrigin::Default => {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        // No Origin: curl, the desktop shell, a script on this PC.
        assert!(may_change_settings(local, None));
        assert!(may_change_settings("::1".parse().unwrap(), None));
        assert!(may_change_settings(
            "::ffff:127.0.0.1".parse().unwrap(),
            None
        ));
        // The UI served by this server, a dev server, the Tauri webview.
        for origin in [
            "http://127.0.0.1:8471",
            "http://localhost:5173",
            "http://LOCALHOST:5173",
            "http://[::1]:8471",
            "tauri://localhost",
            "https://tauri.localhost",
            "http://tauri.localhost",
        ] {
            assert!(may_change_settings(local, Some(origin)), "{origin}");
        }
        // Another device, even through the UI this server gave it.
        assert!(!may_change_settings(phone, None));
        assert!(!may_change_settings(
            phone,
            Some("http://192.168.1.20:8471")
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
            assert!(!may_change_settings(local, Some(origin)), "{origin:?}");
        }
    }
}
