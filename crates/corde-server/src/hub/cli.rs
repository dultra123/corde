//! `corde-server pair | devices | revoke`: commands run **on the server itself** that talk
//! to the running service through its local control routes (`/api/hub/local/*`, loopback
//! only, key of `<data>/hub/control.json` — readable by the service account and root only).

use super::auth::{DeviceInfo, IssuedCode, CONTROL_FILE};
use anyhow::{anyhow, Context};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Data folder of the installed service (`install.sh`).
pub const SERVICE_DATA_DIR: &str = "/var/lib/corde";

#[derive(Debug, Deserialize)]
struct Control {
    port: u16,
    key: String,
}

/// `--data-dir`, else the installed service's folder when it holds a control file, else
/// the per-user data folder.
pub fn resolve_data_dir(given: Option<PathBuf>) -> PathBuf {
    if let Some(d) = given {
        return d;
    }
    let service = Path::new(SERVICE_DATA_DIR);
    if service.join("hub").exists() {
        return service.to_path_buf();
    }
    corde_store::default_data_dir()
}

fn control(data_dir: &Path) -> anyhow::Result<Control> {
    let path = data_dir.join("hub").join(CONTROL_FILE);
    let bytes = std::fs::read(&path).map_err(|e| match e.kind() {
        std::io::ErrorKind::PermissionDenied => anyhow!(
            "accès refusé à {} : relance la commande avec sudo",
            path.display()
        ),
        std::io::ErrorKind::NotFound => anyhow!(
            "{} introuvable : le service Corde serveur est-il lancé ? (sudo systemctl status corde)",
            path.display()
        ),
        _ => anyhow!("{} illisible : {e}", path.display()),
    })?;
    serde_json::from_slice(&bytes).with_context(|| format!("{} illisible", path.display()))
}

fn agent() -> ureq::Agent {
    ureq::config::Config::builder()
        .http_status_as_error(false)
        .proxy(None)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .new_agent()
}

fn call(
    data_dir: &Path,
    method: &str,
    path: &str,
    body: Option<serde_json::Value>,
) -> anyhow::Result<(u16, String)> {
    let c = control(data_dir)?;
    let url = format!("http://127.0.0.1:{}{path}", c.port);
    let auth = format!("Bearer {}", c.key);
    let agent = agent();
    let resp = match (method, body) {
        ("GET", _) => agent.get(&url).header("Authorization", &auth).call(),
        ("DELETE", _) => agent.delete(&url).header("Authorization", &auth).call(),
        (_, Some(b)) => agent
            .post(&url)
            .header("Authorization", &auth)
            .header("Content-Type", "application/json")
            .send(b.to_string().as_bytes()),
        (_, None) => agent.post(&url).header("Authorization", &auth).send_empty(),
    };
    let mut resp = resp.map_err(|e| {
        anyhow!(
            "le service ne répond pas sur le port {} ({e}) : sudo systemctl status corde",
            c.port
        )
    })?;
    let status = resp.status().as_u16();
    let text = resp.body_mut().read_to_string().unwrap_or_default();
    Ok((status, text))
}

fn error_of(text: &str) -> String {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.as_str()).map(str::to_string))
        .unwrap_or_else(|| text.chars().take(200).collect())
}

/// Ask the service for a pairing code (printed on the terminal only).
pub fn pair(data_dir: &Path, admin: bool) -> anyhow::Result<IssuedCode> {
    let (status, text) = call(
        data_dir,
        "POST",
        "/api/hub/local/pairing-code",
        Some(serde_json::json!({ "admin": admin })),
    )?;
    if status != 200 {
        return Err(anyhow!("refusé ({status}) : {}", error_of(&text)));
    }
    Ok(serde_json::from_str(&text)?)
}

pub fn devices(data_dir: &Path) -> anyhow::Result<Vec<DeviceInfoOut>> {
    let (status, text) = call(data_dir, "GET", "/api/hub/local/devices", None)?;
    if status != 200 {
        return Err(anyhow!("refusé ({status}) : {}", error_of(&text)));
    }
    Ok(serde_json::from_str(&text)?)
}

pub fn revoke(data_dir: &Path, id: &str) -> anyhow::Result<bool> {
    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(anyhow!("identifiant d'appareil invalide : {id:?}"));
    }
    let (status, text) = call(
        data_dir,
        "DELETE",
        &format!("/api/hub/local/devices/{id}"),
        None,
    )?;
    match status {
        204 => Ok(true),
        404 => Ok(false),
        _ => Err(anyhow!("refusé ({status}) : {}", error_of(&text))),
    }
}

/// [`DeviceInfo`] as read back from JSON.
#[derive(Debug, Clone, Deserialize)]
pub struct DeviceInfoOut {
    pub id: String,
    pub name: String,
    pub admin: bool,
    pub created_ms: i64,
    pub last_used_ms: Option<i64>,
}

impl From<DeviceInfo> for DeviceInfoOut {
    fn from(d: DeviceInfo) -> Self {
        Self {
            id: d.id,
            name: d.name,
            admin: d.admin,
            created_ms: d.created_ms,
            last_used_ms: d.last_used_ms,
        }
    }
}

/// `1234 5678`.
pub fn format_code(code: &str) -> String {
    if code.len() == 8 {
        format!("{} {}", &code[..4], &code[4..])
    } else {
        code.to_string()
    }
}

/// A Unix ms date for the terminal (UTC).
pub fn format_date(ms: Option<i64>) -> String {
    match ms.and_then(chrono::DateTime::from_timestamp_millis) {
        Some(d) => d.format("%Y-%m-%d %H:%M UTC").to_string(),
        None => "—".to_string(),
    }
}
