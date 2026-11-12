//! The PC → home server synchronisation protocol (ADR 0010), shared by the sender
//! ([`crate::hub_link`], on the PC) and the receiver ([`crate::hub::receive`], on the server).
//!
//! ```text
//! POST /api/sync/hello                       Hello → Manifest (what the server holds)
//! POST /api/sync/results                     zstd(ResultBundle JSON) → {fingerprint}
//! PUT  /api/sync/files/{id}/{name}?offset=&total=   one chunk (≤ CHUNK_MAX) of a Parquet file
//!                                            → {received} · 409 {expected_offset} (resume there)
//! POST /api/sync/files/{id}/{name}/commit    {sha256, size} → file moved into sessions/{id}/
//!                                            · 422 checksum mismatch (the partial file is dropped)
//! PUT  /api/sync/sessions/{id}               zstd(SessionBundle JSON) → {fingerprint}
//!                                            · 409 {missing:[names]} when a sample file is absent
//! GET  /api/sync/sessions/{id}/feedback      FeedbackSet of the server
//! POST /api/sync/sessions/{id}/feedback      FeedbackSet of the PC → MergeReport
//! ```
//!
//! Every request carries `Authorization: Bearer <device token>` and the two version headers
//! below. **Idempotent**: a bundle replaces, a file is identified by its SHA-256, feedback is
//! merged by `uid`. The fingerprint of a bundle is the SHA-256 of its uncompressed JSON: the
//! PC sends again only what differs from the manifest.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Version of this protocol. Bumped on an incompatible change.
pub const SYNC_VERSION: u32 = 1;
/// Oldest protocol version a server of this build still accepts.
pub const MIN_SYNC_VERSION: u32 = 1;

/// Header: the sender's [`SYNC_VERSION`].
pub const HEADER_SYNC_VERSION: &str = "corde-sync-version";
/// Header: the sender's database schema version (`corde_store::db::SCHEMA_VERSION`).
pub const HEADER_SCHEMA_VERSION: &str = "corde-schema-version";

/// Largest chunk of a sample file per request.
pub const CHUNK_MAX: usize = 1024 * 1024;
/// Chunk size the PC sends by default (kind to a slow link: one request ≈ a few seconds at
/// 1 Mbit/s).
pub const CHUNK_DEFAULT: usize = 512 * 1024;
/// Largest compressed bundle per request.
pub const BUNDLE_MAX: usize = 16 * 1024 * 1024;
/// Largest bundle once decompressed.
pub const BUNDLE_MAX_UNCOMPRESSED: u64 = 256 * 1024 * 1024;
/// Largest sample file accepted.
pub const FILE_MAX: u64 = 2 * 1024 * 1024 * 1024;
/// zstd level of the bundles (fast; JSON compresses ≈ 10×).
pub const ZSTD_LEVEL: i32 = 3;

/// `POST /api/sync/hello`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Hello {
    /// Version of Corde on the PC.
    pub version: String,
    pub sync_version: u32,
    pub schema_version: i64,
    /// Addresses at which the PC serves the live view on the local network (empty when it
    /// listens on itself only): the server offers a link to them on its « Direct » tab.
    #[serde(default)]
    pub urls: Vec<String>,
}

/// What the server holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Manifest {
    pub server_version: String,
    pub sync_version: u32,
    pub min_sync_version: u32,
    pub schema_version: i64,
    /// By session id.
    pub sessions: BTreeMap<String, SessionEntry>,
    /// Result file name → fingerprint.
    pub results: BTreeMap<String, String>,
    /// Sessions deleted on the server: not to be sent again.
    pub tombstones: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SessionEntry {
    /// Fingerprint of the bundle received (`None`: only files so far).
    pub fingerprint: Option<String>,
    /// Sample file name → SHA-256.
    pub files: BTreeMap<String, String>,
    pub feedback: BTreeSet<String>,
    pub iterations: BTreeSet<String>,
}

/// `POST …/commit`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileCommit {
    pub sha256: String,
    pub size: u64,
}

/// Why the two ends cannot talk, worded for the driver.
pub fn incompatibility(
    sender_sync: u32,
    sender_schema: i64,
    receiver_version: &str,
) -> Option<(&'static str, String)> {
    let schema = corde_store::db::SCHEMA_VERSION;
    if sender_sync > SYNC_VERSION || sender_schema > schema {
        return Some((
            "server_outdated",
            format!(
                "Le serveur Corde (version {receiver_version}) est plus ancien que Corde sur ton PC : mets à jour le serveur (docs/SERVEUR.md, « Mettre à jour »)."
            ),
        ));
    }
    if sender_sync < MIN_SYNC_VERSION {
        return Some((
            "pc_outdated",
            format!(
                "Corde sur ton PC est trop ancien pour ce serveur (version {receiver_version}) : mets à jour Corde sur ton PC."
            ),
        ));
    }
    None
}

/// zstd of a JSON value.
pub fn compress_json<T: Serialize>(value: &T) -> std::io::Result<(Vec<u8>, String)> {
    let json = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    let fingerprint = crate::crypto::sha256_hex(&json);
    let packed = zstd::encode_all(&json[..], ZSTD_LEVEL)?;
    Ok((packed, fingerprint))
}

/// The JSON of a bundle and its fingerprint (what the PC compares with the manifest).
pub fn fingerprint<T: Serialize>(value: &T) -> std::io::Result<String> {
    let json = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    Ok(crate::crypto::sha256_hex(&json))
}

/// Decompress (at most [`BUNDLE_MAX_UNCOMPRESSED`] bytes) → (value, fingerprint).
pub fn decompress_json<T: for<'de> Deserialize<'de>>(
    packed: &[u8],
) -> std::io::Result<(T, String)> {
    use std::io::Read;
    let decoder = zstd::stream::read::Decoder::new(packed)?;
    let mut json = Vec::new();
    decoder
        .take(BUNDLE_MAX_UNCOMPRESSED + 1)
        .read_to_end(&mut json)?;
    if json.len() as u64 > BUNDLE_MAX_UNCOMPRESSED {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "bundle too large once decompressed",
        ));
    }
    let fingerprint = crate::crypto::sha256_hex(&json);
    let value = serde_json::from_slice(&json)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok((value, fingerprint))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_bundles() {
        let schema = corde_store::db::SCHEMA_VERSION;
        assert!(incompatibility(SYNC_VERSION, schema, "0.7.0").is_none());
        assert!(incompatibility(SYNC_VERSION, schema - 1, "0.7.0").is_none());
        let (code, msg) = incompatibility(SYNC_VERSION, schema + 1, "0.7.0").unwrap();
        assert_eq!(code, "server_outdated");
        assert!(msg.contains("mets à jour le serveur"), "{msg}");
        let (code, _) = incompatibility(SYNC_VERSION + 1, schema, "0.7.0").unwrap();
        assert_eq!(code, "server_outdated");
        let (code, _) = incompatibility(0, schema, "0.7.0").unwrap();
        assert_eq!(code, "pc_outdated");

        let m = Manifest {
            server_version: "x".into(),
            ..Default::default()
        };
        let (packed, fp) = compress_json(&m).unwrap();
        let (back, fp2): (Manifest, String) = decompress_json(&packed).unwrap();
        assert_eq!(back, m);
        assert_eq!(fp, fp2);
        assert_eq!(fp, fingerprint(&m).unwrap());
        assert!(decompress_json::<Manifest>(b"not zstd").is_err());
    }
}
