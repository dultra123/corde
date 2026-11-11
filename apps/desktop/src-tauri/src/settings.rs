//! Les deux petits fichiers JSON que la coquille lit dans le dossier de données
//! (`%LOCALAPPDATA%\Corde`) :
//!
//! - `settings.json` — réglages de Corde, **écrits par le serveur / l'UI web** ; la coquille
//!   n'y lit que `lan_access` (par le code du serveur, `corde_server::Settings`) et n'y écrit
//!   jamais ;
//! - `desktop.json` — mémoire propre à la coquille (« l'info-bulle de la zone de notification
//!   a déjà été montrée »), que personne d'autre ne lit.
//!
//! Tout est tolérant : fichier absent, illisible ou mal formé → valeurs par défaut.

use serde_json::Value;
use std::net::IpAddr;
use std::path::Path;

/// Réglages partagés avec le serveur (contrat : clé booléenne `lan_access`, défaut `false`).
pub const SETTINGS_FILE: &str = "settings.json";
/// État propre à la coquille.
pub const DESKTOP_STATE_FILE: &str = "desktop.json";

const KEY_TRAY_HINT_SHOWN: &str = "tray_hint_shown";

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    // Un fichier enregistré par le Bloc-notes peut commencer par un BOM UTF-8.
    serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()
}

fn read_bool(path: &Path, key: &str) -> bool {
    read_json(path)
        .and_then(|v| v.get(key).and_then(Value::as_bool))
        .unwrap_or(false)
}

/// `true` quand l'utilisateur a activé l'accès depuis le réseau local (téléphone, autre PC).
/// Lu par le code du serveur ([`corde_server::Settings`]) : une seule lecture de
/// `settings.json` pour la coquille et le serveur embarqué.
pub fn lan_access(data_dir: &Path) -> bool {
    corde_server::Settings::load(data_dir).lan_access
}

/// Adresse d'écoute du serveur embarqué : celle que `corde_server::Server::bind` choisira
/// lui-même pour ce dossier de données ([`corde_server::resolve_bind`], sans `--bind` ni
/// `--lan`). La coquille en a besoin avant lui, pour chercher un port libre sur la bonne
/// interface.
///
/// - `lan_access = false` (défaut) → `127.0.0.1` : le serveur n'est joignable que depuis ce
///   PC, donc Windows n'affiche **aucune** invite de pare-feu ;
/// - `lan_access = true` → `0.0.0.0` : toutes les interfaces (téléphone sur le Wi-Fi) ;
///   Windows demande alors une fois d'autoriser Corde sur les réseaux privés.
pub fn resolve_bind_ip(data_dir: &Path) -> IpAddr {
    let cfg = corde_server::ServerConfig {
        data_dir: Some(data_dir.to_path_buf()),
        ..corde_server::ServerConfig::default()
    };
    corde_server::resolve_bind(&cfg, &corde_server::Settings::load(data_dir))
}

/// L'utilisateur a-t-il déjà vu « Corde reste active dans la zone de notification » ?
pub fn tray_hint_shown(data_dir: &Path) -> bool {
    read_bool(&data_dir.join(DESKTOP_STATE_FILE), KEY_TRAY_HINT_SHOWN)
}

/// Mémorise que l'info-bulle a été montrée (les autres clés du fichier sont conservées).
pub fn set_tray_hint_shown(data_dir: &Path) -> std::io::Result<()> {
    let path = data_dir.join(DESKTOP_STATE_FILE);
    let mut root = match read_json(&path) {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    root.insert(KEY_TRAY_HINT_SHOWN.to_string(), Value::Bool(true));
    std::fs::create_dir_all(data_dir)?;
    let text = serde_json::to_string_pretty(&Value::Object(root)).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "corde-desktop-settings-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn lan_access_defaults_to_loopback() {
        let dir = temp_dir("default");
        assert!(!lan_access(&dir));
        assert_eq!(resolve_bind_ip(&dir), IpAddr::V4(Ipv4Addr::LOCALHOST));

        // Mal formé, mauvais type, clé absente : toujours la boucle locale.
        for bad in [
            "",
            "{",
            "[]",
            "null",
            r#"{"lan_access":"true"}"#,
            r#"{"lan_access":1}"#,
            r#"{"other":true}"#,
        ] {
            std::fs::write(dir.join(SETTINGS_FILE), bad).unwrap();
            assert!(!lan_access(&dir), "{bad:?}");
        }
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn lan_access_true_binds_every_interface() {
        let dir = temp_dir("lan");
        std::fs::write(
            dir.join(SETTINGS_FILE),
            "\u{feff}{ \"lan_access\": true, \"theme\": \"dark\" }",
        )
        .unwrap();
        assert!(lan_access(&dir));
        assert_eq!(resolve_bind_ip(&dir), IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        std::fs::write(dir.join(SETTINGS_FILE), r#"{"lan_access":false}"#).unwrap();
        assert_eq!(resolve_bind_ip(&dir), IpAddr::V4(Ipv4Addr::LOCALHOST));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn tray_hint_is_remembered_and_keeps_other_keys() {
        let dir = temp_dir("hint");
        assert!(!tray_hint_shown(&dir));
        std::fs::write(dir.join(DESKTOP_STATE_FILE), r#"{"future_key":42}"#).unwrap();
        set_tray_hint_shown(&dir).unwrap();
        assert!(tray_hint_shown(&dir));
        let v = read_json(&dir.join(DESKTOP_STATE_FILE)).unwrap();
        assert_eq!(v["future_key"], 42);
        // Ne touche pas aux réglages du serveur.
        assert!(!dir.join(SETTINGS_FILE).exists());
        std::fs::remove_dir_all(&dir).ok();
    }
}
