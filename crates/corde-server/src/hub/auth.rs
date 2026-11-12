//! Paired devices of the home server (`--role hub`, ADR 0010, docs/SECURITY-MODEL.md §7).
//!
//! * **Pairing code**: 8 random digits, single use, valid 10 minutes, issued only on the
//!   server itself (`corde-server pair`, through the local control route). Three wrong
//!   attempts invalidate it. Attempts are rate limited per address and in total.
//! * **Device token**: 256 random bits, given once to the device (cookie `HttpOnly`,
//!   `SameSite=Strict` for a browser, `Authorization: Bearer` for the PC). The server keeps
//!   only its SHA-256 (`<data>/hub/devices.json`, mode 0600) and compares in constant time.
//! * **Revocation** is immediate: the device is removed and every WebSocket it holds is
//!   closed ([`HubAuth::subscribe_revocations`]).
//!
//! Neither a code nor a token is ever written to the log or put in an error message.

use crate::crypto::{ct_eq, random_below, random_hex, sha256_hex};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

/// How long a pairing code stays valid.
pub const PAIRING_CODE_TTL: Duration = Duration::from_secs(10 * 60);
/// Wrong attempts before a code is thrown away.
pub const PAIRING_MAX_FAILURES: u8 = 3;
/// Window of the rate limit on pairing attempts.
pub const ATTEMPT_WINDOW: Duration = Duration::from_secs(10 * 60);
/// Pairing attempts per address and window.
pub const ATTEMPTS_PER_ADDRESS: usize = 5;
/// Pairing attempts per window, every address together.
pub const ATTEMPTS_TOTAL: usize = 30;
/// Name of the browser cookie that carries the token.
pub const COOKIE_NAME: &str = "corde_device";
/// Longest device name, in characters.
pub const DEVICE_NAME_MAX: usize = 60;
/// Most devices that may be paired at once.
pub const MAX_DEVICES: usize = 50;
/// `last_used_ms` is written to disk at most this often.
const LAST_USED_RESOLUTION_MS: i64 = 60_000;

/// File of the paired devices, under `<data>/hub/`.
pub const DEVICES_FILE: &str = "devices.json";
/// File the local commands read (port + control key), under `<data>/hub/`.
pub const CONTROL_FILE: &str = "control.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// A browser (phone, tablet…): token in a cookie.
    #[default]
    Browser,
    /// Corde on the PC: token in `Authorization: Bearer`.
    App,
}

/// One paired device, as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct DeviceRecord {
    id: String,
    name: String,
    /// SHA-256 (hex) of the token. The token itself is never stored.
    token_sha256: String,
    created_ms: i64,
    last_used_ms: Option<i64>,
    admin: bool,
    #[serde(default)]
    kind: DeviceKind,
}

/// One paired device, as the API shows it (no hash).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub kind: DeviceKind,
    pub admin: bool,
    pub created_ms: i64,
    pub last_used_ms: Option<i64>,
}

impl From<&DeviceRecord> for DeviceInfo {
    fn from(r: &DeviceRecord) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            kind: r.kind,
            admin: r.admin,
            created_ms: r.created_ms,
            last_used_ms: r.last_used_ms,
        }
    }
}

/// The device behind a request (inserted by the hub middleware).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceCtx {
    pub id: String,
    pub name: String,
    pub admin: bool,
}

struct PairingCode {
    code: String,
    expires: Instant,
    failures: u8,
    /// The device paired with this code is an administrator.
    admin: bool,
}

/// Why a pairing attempt failed. The messages never repeat the code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairError {
    RateLimited { retry_after_s: u64 },
    NoCode,
    Expired,
    Wrong { attempts_left: u8 },
    TooManyFailures,
    TooManyDevices,
    Io(String),
}

impl PairError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RateLimited { .. } => "rate_limited",
            Self::NoCode => "no_code",
            Self::Expired => "expired",
            Self::Wrong { .. } => "wrong_code",
            Self::TooManyFailures => "code_invalidated",
            Self::TooManyDevices => "too_many_devices",
            Self::Io(_) => "io",
        }
    }

    pub fn message_fr(&self) -> String {
        match self {
            Self::RateLimited { retry_after_s } => format!(
                "Trop d'essais depuis cet appareil : réessaie dans {} min.",
                retry_after_s.div_ceil(60).max(1)
            ),
            Self::NoCode => "Aucun code d'appairage en cours : sur le serveur, lance « sudo corde-server pair » pour en obtenir un.".into(),
            Self::Expired => "Ce code a expiré (10 minutes) : demandes-en un nouveau sur le serveur (« sudo corde-server pair »).".into(),
            Self::Wrong { attempts_left } => format!(
                "Code incorrect. Encore {attempts_left} essai(s) avant que ce code ne soit annulé."
            ),
            Self::TooManyFailures => "Trois codes incorrects : ce code est annulé. Demandes-en un nouveau sur le serveur (« sudo corde-server pair »).".into(),
            Self::TooManyDevices => format!("Déjà {MAX_DEVICES} appareils appairés : révoques-en un d'abord."),
            Self::Io(_) => "Le serveur n'a pas pu enregistrer l'appareil (détails dans son journal).".into(),
        }
    }
}

/// A pairing code just issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssuedCode {
    /// 8 digits.
    pub code: String,
    pub expires_in_s: u64,
    pub admin: bool,
}

/// Pairing attempts seen in the current window: per address, and all together.
type Attempts = (HashMap<IpAddr, VecDeque<Instant>>, VecDeque<Instant>);

/// Paired devices, pairing code, rate limits.
pub struct HubAuth {
    dir: PathBuf,
    devices: Mutex<Vec<DeviceRecord>>,
    dirty: AtomicBool,
    pairing: Mutex<Option<PairingCode>>,
    attempts: Mutex<Attempts>,
    revoked: broadcast::Sender<String>,
    control_key: String,
}

impl std::fmt::Debug for HubAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HubAuth").field("dir", &self.dir).finish()
    }
}

fn now_ms() -> i64 {
    corde_store::db::now_ms()
}

/// IPv4-mapped IPv6 counted as the IPv4 address.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(ip),
        v4 => v4,
    }
}

/// Trimmed, control characters removed, at most [`DEVICE_NAME_MAX`] characters; empty →
/// `fallback`.
pub fn clean_name(name: &str, fallback: &str) -> String {
    let cleaned: String = name
        .chars()
        .filter(|c| !c.is_control())
        .collect::<String>()
        .trim()
        .chars()
        .take(DEVICE_NAME_MAX)
        .collect();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned
    }
}

/// Write `data` to `path` through a temporary file (atomic), readable by the owner only.
pub fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        use std::io::Write;
        f.write_all(data)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// Create `dir` readable by the owner only.
pub fn create_private_dir(dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

impl HubAuth {
    /// Open `<data>/hub/` (created, private). An unreadable devices file is set aside and the
    /// server starts with no device paired (closed, never open).
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("hub");
        create_private_dir(&dir)?;
        let path = dir.join(DEVICES_FILE);
        let devices = match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Vec<DeviceRecord>>(&bytes) {
                Ok(d) => d,
                Err(e) => {
                    let aside = dir.join(format!("{DEVICES_FILE}.unreadable-{}", now_ms()));
                    tracing::error!(
                        "paired devices file unreadable ({e}): set aside as {}, no device paired",
                        aside.display()
                    );
                    let _ = std::fs::rename(&path, &aside);
                    Vec::new()
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e),
        };
        let (revoked, _) = broadcast::channel(64);
        Ok(Self {
            dir,
            devices: Mutex::new(devices),
            dirty: AtomicBool::new(false),
            pairing: Mutex::new(None),
            attempts: Mutex::new((HashMap::new(), VecDeque::new())),
            revoked,
            control_key: random_hex(32),
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn lock_devices(&self) -> std::sync::MutexGuard<'_, Vec<DeviceRecord>> {
        match self.devices.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn save(&self, devices: &[DeviceRecord]) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(devices).map_err(io::Error::other)?;
        write_private(&self.dir.join(DEVICES_FILE), &json)?;
        self.dirty.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// Write `<data>/hub/control.json` (port + a key made for this run), mode 0600: what
    /// `corde-server pair | devices | revoke` read to talk to the running service.
    pub fn write_control_file(&self, port: u16) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(&serde_json::json!({
            "port": port,
            "key": self.control_key,
        }))
        .map_err(io::Error::other)?;
        write_private(&self.dir.join(CONTROL_FILE), &json)
    }

    /// `true` when `key` is this run's control key (constant time).
    pub fn control_key_matches(&self, key: &str) -> bool {
        ct_eq(key.as_bytes(), self.control_key.as_bytes())
    }

    /// A fresh pairing code; replaces the previous one. Made administrator when asked, or
    /// when no administrator is paired.
    pub fn issue_code(&self, admin: bool) -> IssuedCode {
        let code = format!("{:08}", random_below(100_000_000));
        let admin = admin || !self.lock_devices().iter().any(|d| d.admin);
        if let Ok(mut p) = self.pairing.lock() {
            *p = Some(PairingCode {
                code: code.clone(),
                expires: Instant::now() + PAIRING_CODE_TTL,
                failures: 0,
                admin,
            });
        }
        tracing::info!(
            "pairing code issued (valid {} min{})",
            PAIRING_CODE_TTL.as_secs() / 60,
            if admin { ", administrator" } else { "" }
        );
        IssuedCode {
            code,
            expires_in_s: PAIRING_CODE_TTL.as_secs(),
            admin,
        }
    }

    /// Counts one attempt from `ip`; `Err(retry_after_s)` when over the limit.
    fn record_attempt(&self, ip: IpAddr, now: Instant) -> Result<(), u64> {
        let ip = canonical(ip);
        let mut guard = match self.attempts.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        let (per_ip, total) = &mut *guard;
        let expire = |q: &mut VecDeque<Instant>| {
            while q
                .front()
                .is_some_and(|t| now.duration_since(*t) >= ATTEMPT_WINDOW)
            {
                q.pop_front();
            }
        };
        expire(total);
        per_ip.retain(|_, q| {
            expire(q);
            !q.is_empty()
        });
        let q = per_ip.entry(ip).or_default();
        let retry = |q: &VecDeque<Instant>| {
            q.front()
                .map(|t| {
                    ATTEMPT_WINDOW
                        .saturating_sub(now.duration_since(*t))
                        .as_secs()
                })
                .unwrap_or(0)
                .max(1)
        };
        if q.len() >= ATTEMPTS_PER_ADDRESS {
            return Err(retry(q));
        }
        if total.len() >= ATTEMPTS_TOTAL {
            return Err(retry(total));
        }
        q.push_back(now);
        total.push_back(now);
        Ok(())
    }

    /// Try a code typed on a device. Success → the new device and its token (to hand over
    /// once). Digits only are compared (spaces, dashes ignored).
    pub fn pair(
        &self,
        ip: IpAddr,
        typed: &str,
        name: &str,
        kind: DeviceKind,
    ) -> Result<(DeviceInfo, String), PairError> {
        self.pair_at(ip, typed, name, kind, Instant::now())
    }

    fn pair_at(
        &self,
        ip: IpAddr,
        typed: &str,
        name: &str,
        kind: DeviceKind,
        now: Instant,
    ) -> Result<(DeviceInfo, String), PairError> {
        if let Err(retry_after_s) = self.record_attempt(ip, now) {
            tracing::warn!("pairing attempt from {ip} refused: too many attempts");
            return Err(PairError::RateLimited { retry_after_s });
        }
        let typed: String = typed.chars().filter(|c| c.is_ascii_digit()).collect();
        let admin = {
            let mut guard = match self.pairing.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            let Some(code) = guard.as_mut() else {
                tracing::info!("pairing attempt from {ip}: no code issued");
                return Err(PairError::NoCode);
            };
            if now >= code.expires {
                *guard = None;
                tracing::info!("pairing attempt from {ip}: code expired");
                return Err(PairError::Expired);
            }
            if !ct_eq(typed.as_bytes(), code.code.as_bytes()) {
                code.failures += 1;
                if code.failures >= PAIRING_MAX_FAILURES {
                    *guard = None;
                    tracing::warn!("pairing attempt from {ip}: wrong code, code invalidated");
                    return Err(PairError::TooManyFailures);
                }
                tracing::warn!("pairing attempt from {ip}: wrong code");
                return Err(PairError::Wrong {
                    attempts_left: PAIRING_MAX_FAILURES - code.failures,
                });
            }
            let admin = code.admin;
            *guard = None;
            admin
        };
        let mut devices = self.lock_devices();
        if devices.len() >= MAX_DEVICES {
            return Err(PairError::TooManyDevices);
        }
        let token = random_hex(32);
        let fallback = match kind {
            DeviceKind::App => "PC",
            DeviceKind::Browser => "Appareil",
        };
        let record = DeviceRecord {
            id: random_hex(8),
            name: clean_name(name, fallback),
            token_sha256: sha256_hex(token.as_bytes()),
            created_ms: now_ms(),
            last_used_ms: Some(now_ms()),
            admin: admin || !devices.iter().any(|d| d.admin),
            kind,
        };
        devices.push(record.clone());
        if let Err(e) = self.save(&devices) {
            devices.pop();
            tracing::error!("cannot save the paired devices: {e}");
            return Err(PairError::Io(e.to_string()));
        }
        tracing::info!(
            "device paired from {ip}: {} ({}){}",
            record.id,
            record.name,
            if record.admin { ", administrator" } else { "" }
        );
        Ok((DeviceInfo::from(&record), token))
    }

    /// The device holding `token`, if any. Every stored hash is compared (constant time,
    /// no early exit).
    pub fn authenticate(&self, token: &str) -> Option<DeviceCtx> {
        let token = token.trim();
        if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let hash = sha256_hex(token.to_ascii_lowercase().as_bytes());
        let mut devices = self.lock_devices();
        let mut found: Option<usize> = None;
        for (i, d) in devices.iter().enumerate() {
            if ct_eq(hash.as_bytes(), d.token_sha256.as_bytes()) {
                found = Some(i);
            }
        }
        let d = &mut devices[found?];
        let now = now_ms();
        if d.last_used_ms
            .is_none_or(|t| now - t >= LAST_USED_RESOLUTION_MS)
        {
            d.last_used_ms = Some(now);
            self.dirty.store(true, Ordering::Relaxed);
        }
        Some(DeviceCtx {
            id: d.id.clone(),
            name: d.name.clone(),
            admin: d.admin,
        })
    }

    /// Write the `last_used_ms` changes, if any (called every minute).
    pub fn flush(&self) {
        if self.dirty.load(Ordering::Relaxed) {
            let devices = self.lock_devices().clone();
            if let Err(e) = self.save(&devices) {
                tracing::warn!("cannot save the paired devices: {e}");
            }
        }
    }

    pub fn list(&self) -> Vec<DeviceInfo> {
        self.lock_devices().iter().map(DeviceInfo::from).collect()
    }

    pub fn get(&self, id: &str) -> Option<DeviceInfo> {
        self.lock_devices()
            .iter()
            .find(|d| d.id == id)
            .map(DeviceInfo::from)
    }

    pub fn has_admin(&self) -> bool {
        self.lock_devices().iter().any(|d| d.admin)
    }

    /// Change a device's name and / or administrator flag. `Ok(None)`: unknown id.
    pub fn update(
        &self,
        id: &str,
        name: Option<&str>,
        admin: Option<bool>,
    ) -> io::Result<Option<DeviceInfo>> {
        let mut devices = self.lock_devices();
        let Some(i) = devices.iter().position(|d| d.id == id) else {
            return Ok(None);
        };
        let before = devices[i].clone();
        if let Some(n) = name {
            devices[i].name = clean_name(n, &before.name);
        }
        if let Some(a) = admin {
            devices[i].admin = a;
        }
        if let Err(e) = self.save(&devices) {
            devices[i] = before;
            return Err(e);
        }
        Ok(Some(DeviceInfo::from(&devices[i])))
    }

    /// Remove a device: its token stops working at once and its WebSockets are closed.
    pub fn revoke(&self, id: &str) -> io::Result<bool> {
        let mut devices = self.lock_devices();
        let Some(i) = devices.iter().position(|d| d.id == id) else {
            return Ok(false);
        };
        let removed = devices.remove(i);
        if let Err(e) = self.save(&devices) {
            devices.insert(i, removed);
            return Err(e);
        }
        drop(devices);
        let _ = self.revoked.send(removed.id.clone());
        tracing::info!("device revoked: {} ({})", removed.id, removed.name);
        Ok(true)
    }

    /// Ids of the devices revoked from now on.
    pub fn subscribe_revocations(&self) -> broadcast::Receiver<String> {
        self.revoked.subscribe()
    }
}

/// The token of a request: `Authorization: Bearer …`, else the `corde_device` cookie.
pub fn token_of(headers: &axum::http::HeaderMap) -> Option<String> {
    use axum::http::header;
    if let Some(v) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(t) = v
            .strip_prefix("Bearer ")
            .or_else(|| v.strip_prefix("bearer "))
        {
            return Some(t.trim().to_string());
        }
    }
    for v in headers.get_all(header::COOKIE) {
        let Ok(v) = v.to_str() else { continue };
        for part in v.split(';') {
            if let Some((k, val)) = part.trim().split_once('=') {
                if k.trim() == COOKIE_NAME {
                    return Some(val.trim().to_string());
                }
            }
        }
    }
    None
}

/// `Set-Cookie` value that hands a browser its token. Not `Secure`: the server speaks plain
/// HTTP on the private network (the VPN encrypts the link from outside; see
/// docs/SECURITY-MODEL.md §7).
pub fn cookie_set(token: &str) -> String {
    format!("{COOKIE_NAME}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=34560000")
}

/// `Set-Cookie` value that removes it.
pub fn cookie_clear() -> String {
    format!("{COOKIE_NAME}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "corde-hub-auth-{tag}-{}-{}",
            std::process::id(),
            now_ms()
        ))
    }

    const PHONE: IpAddr = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 42));

    #[test]
    fn pairing_gives_a_token_stored_as_a_hash_and_the_first_device_is_admin() {
        let d = dir("pair");
        let auth = HubAuth::open(&d).unwrap();
        assert!(
            auth.pair(PHONE, "12345678", "x", DeviceKind::Browser)
                .unwrap_err()
                == PairError::NoCode
        );
        let code = auth.issue_code(false);
        assert_eq!(code.code.len(), 8);
        assert!(code.admin, "no administrator yet");
        let typed = format!("{} {}", &code.code[..4], &code.code[4..]);
        let (dev, token) = auth
            .pair(PHONE, &typed, "  Mon téléphone\n ", DeviceKind::Browser)
            .unwrap();
        assert_eq!(dev.name, "Mon téléphone");
        assert!(dev.admin);
        assert_eq!(token.len(), 64);
        // Single use.
        assert_eq!(
            auth.pair(PHONE, &code.code, "x", DeviceKind::Browser)
                .unwrap_err(),
            PairError::NoCode
        );
        // Stored as a hash only, file private.
        let text = std::fs::read_to_string(d.join("hub").join(DEVICES_FILE)).unwrap();
        assert!(!text.contains(&token));
        assert!(text.contains(&sha256_hex(token.as_bytes())));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(d.join("hub").join(DEVICES_FILE))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o077, 0, "{mode:o}");
        }
        assert_eq!(auth.authenticate(&token).unwrap().id, dev.id);
        assert_eq!(
            auth.authenticate(&token.to_ascii_uppercase()).unwrap().id,
            dev.id
        );
        assert!(auth.authenticate(&"0".repeat(64)).is_none());
        assert!(auth.authenticate("short").is_none());
        // The second device is not an administrator, unless the code says so.
        let c2 = auth.issue_code(false);
        assert!(!c2.admin);
        let (pc, pc_token) = auth.pair(PHONE, &c2.code, "", DeviceKind::App).unwrap();
        assert_eq!(pc.name, "PC");
        assert!(!pc.admin);
        let c3 = auth.issue_code(true);
        assert!(c3.admin);

        // Survives a restart.
        drop(auth);
        let auth = HubAuth::open(&d).unwrap();
        assert_eq!(auth.list().len(), 2);
        assert!(auth.authenticate(&pc_token).is_some());
        // Revocation.
        let mut rx = auth.subscribe_revocations();
        assert!(auth.revoke(&pc.id).unwrap());
        assert!(!auth.revoke(&pc.id).unwrap());
        assert_eq!(rx.try_recv().unwrap(), pc.id);
        assert!(auth.authenticate(&pc_token).is_none());
        // Rename, admin flag.
        let up = auth
            .update(&dev.id, Some("Tablette"), Some(false))
            .unwrap()
            .unwrap();
        assert_eq!(up.name, "Tablette");
        assert!(!up.admin && !auth.has_admin());
        assert!(auth.update("nope", Some("x"), None).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn three_wrong_codes_invalidate_it_and_an_old_code_expires() {
        let d = dir("wrong");
        let auth = HubAuth::open(&d).unwrap();
        let code = auth.issue_code(false);
        let wrong = if code.code == "00000000" {
            "11111111"
        } else {
            "00000000"
        };
        let t0 = Instant::now();
        let ip = |n| IpAddr::V4(std::net::Ipv4Addr::new(10, 0, 0, n));
        assert_eq!(
            auth.pair_at(ip(1), wrong, "", DeviceKind::Browser, t0)
                .unwrap_err(),
            PairError::Wrong { attempts_left: 2 }
        );
        assert_eq!(
            auth.pair_at(ip(2), "garbage", "", DeviceKind::Browser, t0)
                .unwrap_err(),
            PairError::Wrong { attempts_left: 1 }
        );
        assert_eq!(
            auth.pair_at(ip(3), wrong, "", DeviceKind::Browser, t0)
                .unwrap_err(),
            PairError::TooManyFailures
        );
        // Even the right code no longer works.
        assert_eq!(
            auth.pair_at(ip(4), &code.code, "", DeviceKind::Browser, t0)
                .unwrap_err(),
            PairError::NoCode
        );
        // Expiry.
        let code = auth.issue_code(false);
        let late = Instant::now() + PAIRING_CODE_TTL + Duration::from_secs(1);
        assert_eq!(
            auth.pair_at(ip(5), &code.code, "", DeviceKind::Browser, late)
                .unwrap_err(),
            PairError::Expired
        );
        assert!(auth.list().is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn attempts_are_rate_limited_per_address_and_in_total() {
        let d = dir("rate");
        let auth = HubAuth::open(&d).unwrap();
        let t0 = Instant::now();
        for _ in 0..ATTEMPTS_PER_ADDRESS {
            assert_eq!(
                auth.pair_at(PHONE, "1", "", DeviceKind::Browser, t0)
                    .unwrap_err(),
                PairError::NoCode
            );
        }
        // The right code from that address is refused too, before being looked at.
        let code = auth.issue_code(false);
        match auth
            .pair_at(PHONE, &code.code, "", DeviceKind::Browser, t0)
            .unwrap_err()
        {
            PairError::RateLimited { retry_after_s } => {
                assert!(retry_after_s > 0 && retry_after_s <= 600)
            }
            other => panic!("{other:?}"),
        }
        // IPv4-mapped IPv6 is the same address.
        let mapped: IpAddr = "::ffff:192.168.1.42".parse().unwrap();
        assert!(matches!(
            auth.pair_at(mapped, &code.code, "", DeviceKind::Browser, t0),
            Err(PairError::RateLimited { .. })
        ));
        // Another address may still pair; after the window, this one is heard again.
        let other = IpAddr::V4(std::net::Ipv4Addr::new(192, 168, 1, 43));
        assert!(auth
            .pair_at(other, &code.code, "", DeviceKind::Browser, t0)
            .is_ok());
        let later = t0 + ATTEMPT_WINDOW + Duration::from_secs(1);
        assert_eq!(
            auth.pair_at(PHONE, "1", "", DeviceKind::Browser, later)
                .unwrap_err(),
            PairError::NoCode
        );
        // Total limit.
        let auth = HubAuth::open(&dir("rate2")).unwrap();
        let mut refused = 0;
        for i in 0..(ATTEMPTS_TOTAL + 5) {
            let ip = IpAddr::V4(std::net::Ipv4Addr::new(
                10,
                1,
                (i / 200) as u8,
                (i % 200) as u8,
            ));
            if matches!(
                auth.pair_at(ip, "1", "", DeviceKind::Browser, t0),
                Err(PairError::RateLimited { .. })
            ) {
                refused += 1;
            }
        }
        assert_eq!(refused, 5);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn tokens_come_from_the_bearer_header_or_the_cookie() {
        use axum::http::{HeaderMap, HeaderValue};
        let mut h = HeaderMap::new();
        assert_eq!(token_of(&h), None);
        h.insert(
            "cookie",
            HeaderValue::from_static("theme=dark; corde_device=abc ; x=1"),
        );
        assert_eq!(token_of(&h).as_deref(), Some("abc"));
        h.insert("authorization", HeaderValue::from_static("Bearer def"));
        assert_eq!(token_of(&h).as_deref(), Some("def"));
        let c = cookie_set("t");
        assert!(c.contains("HttpOnly") && c.contains("SameSite=Strict") && c.contains("Path=/"));
        assert!(!c.contains("Secure"));
        assert!(cookie_clear().contains("Max-Age=0"));
        assert_eq!(clean_name("\u{7}  ", "Appareil"), "Appareil");
        assert_eq!(
            clean_name(&"é".repeat(100), "x").chars().count(),
            DEVICE_NAME_MAX
        );
    }

    #[test]
    fn an_unreadable_devices_file_means_no_device() {
        let d = dir("bad");
        create_private_dir(&d.join("hub")).unwrap();
        std::fs::write(d.join("hub").join(DEVICES_FILE), "not json").unwrap();
        let auth = HubAuth::open(&d).unwrap();
        assert!(auth.list().is_empty());
        assert!(!d.join("hub").join(DEVICES_FILE).exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
