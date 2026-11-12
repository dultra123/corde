//! Les deux petits fichiers JSON que la coquille lit dans le dossier de données
//! (`%LOCALAPPDATA%\Corde`) :
//!
//! - `settings.json` — réglages de Corde, **écrits par le serveur / l'UI web** ; la coquille
//!   n'y lit que `lan_access` (par le code du serveur, `corde_server::Settings`) et n'y écrit
//!   jamais ;
//! - `desktop.json` — mémoire et réglages propres à la coquille, que personne d'autre ne lit :
//!   « l'info-bulle de la zone de notification a déjà été montrée », « rechercher les mises à
//!   jour automatiquement » (vrai par défaut), dernière version annoncée par une notification,
//!   mise à jour en cours d'installation (voir [`PendingUpdate`]).
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
const KEY_UPDATE_AUTO_CHECK: &str = "update_auto_check";
const KEY_UPDATE_NOTIFIED: &str = "update_notified_version";
const KEY_UPDATE_PENDING: &str = "update_pending";

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
    set_desktop_key(data_dir, KEY_TRAY_HINT_SHOWN, Some(Value::Bool(true)))
}

/// Écrit (`Some`) ou retire (`None`) une clé de `desktop.json` en gardant toutes les autres.
fn set_desktop_key(data_dir: &Path, key: &str, value: Option<Value>) -> std::io::Result<()> {
    let path = data_dir.join(DESKTOP_STATE_FILE);
    let mut root = match read_json(&path) {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    match value {
        Some(value) => {
            root.insert(key.to_string(), value);
        }
        None => {
            root.remove(key);
        }
    }
    std::fs::create_dir_all(data_dir)?;
    let text = serde_json::to_string_pretty(&Value::Object(root)).map_err(std::io::Error::other)?;
    std::fs::write(path, text)
}

fn desktop_value(data_dir: &Path, key: &str) -> Option<Value> {
    match read_json(&data_dir.join(DESKTOP_STATE_FILE)) {
        Some(Value::Object(mut map)) => map.remove(key),
        _ => None,
    }
}

/// Réglage « rechercher automatiquement les mises à jour » : **vrai par défaut** (fichier
/// absent, clé absente ou d'un autre type) ; seul un `false` explicite le coupe.
pub fn update_auto_check(data_dir: &Path) -> bool {
    !matches!(
        desktop_value(data_dir, KEY_UPDATE_AUTO_CHECK),
        Some(Value::Bool(false))
    )
}

pub fn set_update_auto_check(data_dir: &Path, enabled: bool) -> std::io::Result<()> {
    set_desktop_key(data_dir, KEY_UPDATE_AUTO_CHECK, Some(Value::Bool(enabled)))
}

/// Dernière version annoncée par une notification Windows (une seule notification par
/// version, même après un redémarrage).
pub fn update_notified_version(data_dir: &Path) -> Option<String> {
    match desktop_value(data_dir, KEY_UPDATE_NOTIFIED) {
        Some(Value::String(v)) if !v.trim().is_empty() => Some(v),
        _ => None,
    }
}

pub fn set_update_notified_version(data_dir: &Path, version: &str) -> std::io::Result<()> {
    set_desktop_key(
        data_dir,
        KEY_UPDATE_NOTIFIED,
        Some(Value::String(version.to_string())),
    )
}

/// Mise à jour dont l'installeur vient d'être lancé. Écrite juste avant que Corde ne
/// s'arrête pour lui laisser la place, relue (puis effacée) au lancement suivant : elle dit
/// si l'installation a pris, et fait apparaître la fenêtre même si l'installeur a relancé
/// Corde avec `--hidden` (il reprend les arguments de l'instance qu'il remplace).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpdate {
    pub from: String,
    pub to: String,
}

pub fn set_pending_update(data_dir: &Path, from: &str, to: &str) -> std::io::Result<()> {
    let value = serde_json::json!({ "from": from, "to": to });
    set_desktop_key(data_dir, KEY_UPDATE_PENDING, Some(value))
}

pub fn clear_pending_update(data_dir: &Path) -> std::io::Result<()> {
    set_desktop_key(data_dir, KEY_UPDATE_PENDING, None)
}

/// Lit et efface la mise à jour en attente (`None` s'il n'y en a pas, ou si elle est
/// illisible — elle est alors effacée aussi).
pub fn take_pending_update(data_dir: &Path) -> Option<PendingUpdate> {
    let value = desktop_value(data_dir, KEY_UPDATE_PENDING)?;
    let _ = clear_pending_update(data_dir);
    let field = |k: &str| {
        value
            .get(k)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    Some(PendingUpdate {
        from: field("from")?,
        to: field("to")?,
    })
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

    #[test]
    fn update_auto_check_defaults_to_on() {
        let dir = temp_dir("auto");
        assert!(update_auto_check(&dir), "no file");
        for (text, expected) in [
            ("", true),
            ("{", true),
            ("[]", true),
            (r#"{"tray_hint_shown":true}"#, true),
            (r#"{"update_auto_check":"false"}"#, true),
            (r#"{"update_auto_check":0}"#, true),
            (r#"{"update_auto_check":true}"#, true),
            (r#"{"update_auto_check":false}"#, false),
            ("\u{feff}{\"update_auto_check\":false}", false),
        ] {
            std::fs::write(dir.join(DESKTOP_STATE_FILE), text).unwrap();
            assert_eq!(update_auto_check(&dir), expected, "{text:?}");
        }
        std::fs::write(dir.join(DESKTOP_STATE_FILE), r#"{"tray_hint_shown":true}"#).unwrap();
        set_update_auto_check(&dir, false).unwrap();
        assert!(!update_auto_check(&dir));
        assert!(tray_hint_shown(&dir), "other keys kept");
        set_update_auto_check(&dir, true).unwrap();
        assert!(update_auto_check(&dir));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn notified_version_and_pending_update() {
        let dir = temp_dir("pending");
        assert_eq!(update_notified_version(&dir), None);
        set_update_notified_version(&dir, "0.6.1").unwrap();
        assert_eq!(update_notified_version(&dir).as_deref(), Some("0.6.1"));

        assert_eq!(take_pending_update(&dir), None);
        set_pending_update(&dir, "0.6.0", "0.6.1").unwrap();
        assert_eq!(
            take_pending_update(&dir),
            Some(PendingUpdate {
                from: "0.6.0".into(),
                to: "0.6.1".into()
            })
        );
        assert_eq!(take_pending_update(&dir), None, "read once");
        assert_eq!(update_notified_version(&dir).as_deref(), Some("0.6.1"));

        // Illisible : effacée sans erreur, les autres clés restent.
        std::fs::write(
            dir.join(DESKTOP_STATE_FILE),
            r#"{"update_pending":{"from":"0.6.0"},"update_auto_check":false}"#,
        )
        .unwrap();
        assert_eq!(take_pending_update(&dir), None);
        let v = read_json(&dir.join(DESKTOP_STATE_FILE)).unwrap();
        assert!(v.get("update_pending").is_none());
        assert_eq!(v["update_auto_check"], false);

        set_pending_update(&dir, "0.6.0", "0.6.1").unwrap();
        clear_pending_update(&dir).unwrap();
        assert_eq!(take_pending_update(&dir), None);
        std::fs::remove_dir_all(&dir).ok();
    }
}
