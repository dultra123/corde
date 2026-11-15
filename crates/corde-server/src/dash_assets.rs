//! Pictures and videos of the race dashboards (ADR 0016), stored on the PC next to the
//! definitions: `<data-dir>/dash-assets/<id>`, where the id is the SHA-256 of the content and the
//! extension of its type (`<64 hex>.png`). A definition cites a media by this id, never by a
//! path or an address.
//!
//! * **What is accepted**: PNG, JPEG, WebP, GIF, MP4 (ISO base media, MOV included) and WebM,
//!   recognised by the **first bytes of the file** — never by its name nor by the type a client
//!   announces. No SVG (it can hold a script), no sound, nothing else.
//! * **Limits**: a picture ≤ [`MAX_IMAGE_BYTES`], a video ≤ [`MAX_VIDEO_BYTES`], at most
//!   [`MAX_ASSETS`] files and [`MAX_TOTAL_BYTES`] in all.
//! * **Clean-up** ([`AssetStore::prune`]): a file that no definition cites any more is removed
//!   (when a definition is saved or deleted, and at start-up), once it is older than
//!   [`PRUNE_GRACE`] — the time for the Studio to save the definition a picture was sent for.
//!
//! The interface strips the metadata of a picture before sending it (re-encoding) and empties
//! the metadata boxes of a video; the server never decodes a media, it only stores and serves it.

use ring::digest::{digest, SHA256};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

/// Largest picture.
pub const MAX_IMAGE_BYTES: u64 = 4 * 1024 * 1024;
/// Largest video.
pub const MAX_VIDEO_BYTES: u64 = 30 * 1024 * 1024;
/// Most media files kept.
pub const MAX_ASSETS: usize = 200;
/// Most bytes of media kept.
pub const MAX_TOTAL_BYTES: u64 = 300 * 1024 * 1024;
/// A media no definition cites is kept this long (it was just sent, its definition is not
/// saved yet, or a draft of the Studio uses it).
pub const PRUNE_GRACE: Duration = Duration::from_secs(24 * 3600);

/// The kinds of media accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    Png,
    Jpeg,
    Webp,
    Gif,
    Mp4,
    Webm,
}

impl AssetKind {
    pub const ALL: [AssetKind; 6] = [
        AssetKind::Png,
        AssetKind::Jpeg,
        AssetKind::Webp,
        AssetKind::Gif,
        AssetKind::Mp4,
        AssetKind::Webm,
    ];

    /// The extension of the id (and of the file).
    pub fn ext(self) -> &'static str {
        match self {
            AssetKind::Png => "png",
            AssetKind::Jpeg => "jpg",
            AssetKind::Webp => "webp",
            AssetKind::Gif => "gif",
            AssetKind::Mp4 => "mp4",
            AssetKind::Webm => "webm",
        }
    }

    /// The `Content-Type` it is served with.
    pub fn mime(self) -> &'static str {
        match self {
            AssetKind::Png => "image/png",
            AssetKind::Jpeg => "image/jpeg",
            AssetKind::Webp => "image/webp",
            AssetKind::Gif => "image/gif",
            AssetKind::Mp4 => "video/mp4",
            AssetKind::Webm => "video/webm",
        }
    }

    pub fn is_video(self) -> bool {
        matches!(self, AssetKind::Mp4 | AssetKind::Webm)
    }

    /// The largest file of this kind.
    pub fn max_bytes(self) -> u64 {
        if self.is_video() {
            MAX_VIDEO_BYTES
        } else {
            MAX_IMAGE_BYTES
        }
    }

    pub fn from_ext(ext: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.ext() == ext)
    }
}

/// The kind of a file from its first bytes (its signature), `None` for anything else.
pub fn sniff(head: &[u8]) -> Option<AssetKind> {
    if head.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(AssetKind::Png);
    }
    if head.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some(AssetKind::Jpeg);
    }
    if head.starts_with(b"GIF87a") || head.starts_with(b"GIF89a") {
        return Some(AssetKind::Gif);
    }
    if head.len() >= 12 && &head[0..4] == b"RIFF" && &head[8..12] == b"WEBP" {
        return Some(AssetKind::Webp);
    }
    // ISO base media (MP4, MOV): a first box `ftyp` of a sane size.
    if head.len() >= 12 && &head[4..8] == b"ftyp" {
        let size = u32::from_be_bytes([head[0], head[1], head[2], head[3]]);
        if (8..=4096).contains(&size) {
            return Some(AssetKind::Mp4);
        }
    }
    // EBML with the document type `webm` in its header.
    if head.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        let n = head.len().min(64);
        if head[..n].windows(4).any(|w| w == b"webm") {
            return Some(AssetKind::Webm);
        }
    }
    None
}

/// `<64 lowercase hex>.<ext>`: the id of a media (also its file name, never a device name of
/// Windows nor a path).
pub fn valid_asset_id(id: &str) -> bool {
    parse_asset_id(id).is_some()
}

/// The kind an id names.
pub fn parse_asset_id(id: &str) -> Option<AssetKind> {
    let (hash, ext) = id.split_once('.')?;
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    AssetKind::from_ext(ext)
}

/// The SHA-256 of the content, lowercase hex.
pub fn content_hash(bytes: &[u8]) -> String {
    let d = digest(&SHA256, bytes);
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

/// What the store knows of a media.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct AssetInfo {
    pub id: String,
    #[serde(skip)]
    pub kind: AssetKind,
    pub mime: &'static str,
    pub size: u64,
}

/// Why a media was not stored.
#[derive(Debug)]
pub enum AssetError {
    /// Not a PNG, JPEG, WebP, GIF, MP4 or WebM (by its first bytes).
    Unsupported,
    /// Larger than the limit of its kind.
    TooLarge(AssetKind),
    /// Already [`MAX_ASSETS`] files or [`MAX_TOTAL_BYTES`].
    Full,
    Io(io::Error),
}

impl std::fmt::Display for AssetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetError::Unsupported => write!(
                f,
                "unsupported file: pictures PNG, JPEG, WebP, GIF and videos MP4, WebM only"
            ),
            AssetError::TooLarge(k) => write!(
                f,
                "file too large ({} at most {} MiB)",
                if k.is_video() { "a video" } else { "a picture" },
                k.max_bytes() / (1024 * 1024)
            ),
            AssetError::Full => write!(
                f,
                "too many media (at most {MAX_ASSETS} files and {} MiB)",
                MAX_TOTAL_BYTES / (1024 * 1024)
            ),
            AssetError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl From<io::Error> for AssetError {
    fn from(e: io::Error) -> Self {
        AssetError::Io(e)
    }
}

pub struct AssetStore {
    dir: PathBuf,
    index: Mutex<BTreeMap<String, AssetInfo>>,
}

impl std::fmt::Debug for AssetStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetStore")
            .field("dir", &self.dir)
            .finish()
    }
}

/// The first bytes of a file (enough for [`sniff`]).
fn read_head(path: &Path) -> io::Result<Vec<u8>> {
    let mut f = fs::File::open(path)?;
    let mut head = vec![0u8; 64];
    let mut n = 0;
    while n < head.len() {
        let r = f.read(&mut head[n..])?;
        if r == 0 {
            break;
        }
        n += r;
    }
    head.truncate(n);
    Ok(head)
}

impl AssetStore {
    /// Open (creating it) `<data_dir>/dash-assets` and index every valid media in it: a name
    /// that is an id, a signature that matches its extension, a size within the limits.
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let dir = data_dir.join("dash-assets");
        fs::create_dir_all(&dir)?;
        let mut index = BTreeMap::new();
        for entry in fs::read_dir(&dir)? {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            // A temporary file left by a write that did not finish.
            if name.ends_with(".tmp") {
                let _ = fs::remove_file(&path);
                continue;
            }
            let Some(kind) = parse_asset_id(name) else {
                continue;
            };
            let ok = fs::metadata(&path).and_then(|m| {
                let head = read_head(&path)?;
                Ok((m.len(), m.is_file() && sniff(&head) == Some(kind)))
            });
            match ok {
                Ok((size, true)) if size <= kind.max_bytes() => {
                    index.insert(
                        name.to_string(),
                        AssetInfo {
                            id: name.to_string(),
                            kind,
                            mime: kind.mime(),
                            size,
                        },
                    );
                }
                Ok(_) => {
                    tracing::warn!("ignoring media {}: not what its name says", path.display())
                }
                Err(e) => tracing::warn!("ignoring unreadable media {}: {e}", path.display()),
            }
        }
        tracing::info!("{} dashboard media in {}", index.len(), dir.display());
        Ok(Self {
            dir,
            index: Mutex::new(index),
        })
    }

    /// [`AssetStore::open`], or an empty store when the folder cannot be read.
    pub fn open_or_empty(data_dir: &Path) -> Self {
        Self::open(data_dir).unwrap_or_else(|e| {
            tracing::error!("dashboard media folder unreadable: {e}");
            Self {
                dir: data_dir.join("dash-assets"),
                index: Mutex::new(BTreeMap::new()),
            }
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn list(&self) -> Vec<AssetInfo> {
        self.lock().values().cloned().collect()
    }

    pub fn get(&self, id: &str) -> Option<AssetInfo> {
        self.lock().get(id).cloned()
    }

    /// Count and bytes stored.
    pub fn usage(&self) -> (usize, u64) {
        let idx = self.lock();
        (idx.len(), idx.values().map(|a| a.size).sum())
    }

    /// Stores a media (its kind read from its first bytes); the same content gives the same id,
    /// stored once.
    pub fn put(&self, bytes: &[u8]) -> Result<AssetInfo, AssetError> {
        let kind = sniff(bytes).ok_or(AssetError::Unsupported)?;
        let size = bytes.len() as u64;
        if size > kind.max_bytes() {
            return Err(AssetError::TooLarge(kind));
        }
        let id = format!("{}.{}", content_hash(bytes), kind.ext());
        let mut idx = self.lock();
        if let Some(a) = idx.get(&id) {
            // Sent again: still in use, a fresh date keeps it from the clean-up a little longer.
            let _ = fs::File::options()
                .write(true)
                .open(self.path_of(&id))
                .and_then(|f| f.set_modified(SystemTime::now()));
            return Ok(a.clone());
        }
        let total: u64 = idx.values().map(|a| a.size).sum();
        if idx.len() >= MAX_ASSETS || total + size > MAX_TOTAL_BYTES {
            return Err(AssetError::Full);
        }
        let tmp = self.dir.join(format!("{id}.tmp"));
        fs::write(&tmp, bytes)?;
        fs::rename(&tmp, self.path_of(&id))?;
        let info = AssetInfo {
            id: id.clone(),
            kind,
            mime: kind.mime(),
            size,
        };
        idx.insert(id, info.clone());
        Ok(info)
    }

    /// Reads `len` bytes from `start` of a media (a whole picture, a part of a video).
    pub fn read_range(&self, id: &str, start: u64, len: u64) -> io::Result<Vec<u8>> {
        if !valid_asset_id(id) {
            return Err(io::Error::new(io::ErrorKind::NotFound, "no such media"));
        }
        let mut f = fs::File::open(self.path_of(id))?;
        f.seek(SeekFrom::Start(start))?;
        let mut out = Vec::with_capacity(len.min(MAX_VIDEO_BYTES) as usize);
        f.take(len).read_to_end(&mut out)?;
        Ok(out)
    }

    /// Returns `true` when the media existed.
    pub fn delete(&self, id: &str) -> io::Result<bool> {
        if !valid_asset_id(id) {
            return Ok(false);
        }
        let existed = self.lock().remove(id).is_some();
        match fs::remove_file(self.path_of(id)) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(existed),
            Err(e) => Err(e),
        }
    }

    /// Removes the media no definition cites (`referenced`) and older than `grace`; returns how
    /// many were removed.
    pub fn prune(&self, referenced: &HashSet<String>, grace: Duration) -> usize {
        let now = SystemTime::now();
        let mut idx = self.lock();
        let unused: Vec<String> = idx
            .keys()
            .filter(|id| !referenced.contains(*id))
            .filter(|id| {
                let age = fs::metadata(self.path_of(id))
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|m| now.duration_since(m).ok());
                // A date in the future or unreadable: the file is kept.
                age.is_some_and(|a| a >= grace)
            })
            .cloned()
            .collect();
        let mut n = 0;
        for id in unused {
            match fs::remove_file(self.path_of(&id)) {
                Ok(()) => {
                    idx.remove(&id);
                    n += 1;
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    idx.remove(&id);
                }
                Err(e) => tracing::warn!("media {id} not removed: {e}"),
            }
        }
        if n > 0 {
            tracing::info!("{n} dashboard media no longer used, removed");
        }
        n
    }

    fn path_of(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, AssetInfo>> {
        match self.index.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        }
    }
}

/// A `Range` that cannot be satisfied (416).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BadRange;

/// A `Range: bytes=…` header for a file of `size` bytes: `Ok(None)` without one (or one that is
/// not a single byte range: the whole file), `Ok(Some((start, end)))` inclusive, `Err(BadRange)`
/// when it cannot be satisfied (416).
pub fn parse_range(header: Option<&str>, size: u64) -> Result<Option<(u64, u64)>, BadRange> {
    let Some(h) = header else { return Ok(None) };
    let Some(spec) = h.trim().strip_prefix("bytes=") else {
        return Ok(None);
    };
    // Several ranges: answered with the whole file (allowed by RFC 9110).
    if spec.contains(',') {
        return Ok(None);
    }
    let (a, b) = spec.split_once('-').ok_or(BadRange)?;
    let (a, b) = (a.trim(), b.trim());
    if size == 0 {
        return Err(BadRange);
    }
    if a.is_empty() {
        // The last `b` bytes.
        let n: u64 = b.parse().map_err(|_| BadRange)?;
        if n == 0 {
            return Err(BadRange);
        }
        let n = n.min(size);
        return Ok(Some((size - n, size - 1)));
    }
    let start: u64 = a.parse().map_err(|_| BadRange)?;
    if start >= size {
        return Err(BadRange);
    }
    let end = if b.is_empty() {
        size - 1
    } else {
        let e: u64 = b.parse().map_err(|_| BadRange)?;
        if e < start {
            return Err(BadRange);
        }
        e.min(size - 1)
    };
    Ok(Some((start, end)))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "corde-assets-{}-{}-{:?}",
            name,
            std::process::id(),
            SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let _ = fs::remove_dir_all(&d);
        d
    }

    /// A tiny PNG (1 × 1, made here), plus `salt` bytes after its end to vary its hash.
    pub(crate) fn png(salt: u8) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
        v.extend_from_slice(&[0, 0, 0, 13]);
        v.extend_from_slice(b"IHDR");
        v.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0]);
        v.extend_from_slice(&[0x1f, 0x15, 0xc4, 0x89]);
        v.push(salt);
        v
    }

    pub(crate) fn mp4(len: usize) -> Vec<u8> {
        let mut v = vec![0, 0, 0, 20];
        v.extend_from_slice(b"ftypisom\0\0\x02\0isom");
        v.resize(len.max(v.len()), 0);
        v
    }

    #[test]
    fn signatures() {
        assert_eq!(sniff(&png(0)), Some(AssetKind::Png));
        assert_eq!(
            sniff(&[0xff, 0xd8, 0xff, 0xe0, 0, 0]),
            Some(AssetKind::Jpeg)
        );
        assert_eq!(sniff(b"GIF89a\x01\x00"), Some(AssetKind::Gif));
        assert_eq!(sniff(b"RIFF\x10\0\0\0WEBPVP8 "), Some(AssetKind::Webp));
        assert_eq!(sniff(&mp4(64)), Some(AssetKind::Mp4));
        let mut qt = vec![0, 0, 0, 20];
        qt.extend_from_slice(b"ftypqt  \0\0\0\0qt  ");
        assert_eq!(sniff(&qt), Some(AssetKind::Mp4), "MOV");
        let mut webm = vec![0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x86, 0x81, 0x01];
        webm.extend_from_slice(b"\x42\x82\x84webm");
        assert_eq!(sniff(&webm), Some(AssetKind::Webm));
        for bad in [
            &b"<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>"[..],
            b"<?xml version='1.0'?><svg/>",
            b"<html><body>x</body></html>",
            b"ID3\x04\0\0\0\0\0\0",        // mp3
            b"OggS\0\x02\0\0\0\0\0\0\0\0", // ogg
            b"RIFF\x10\0\0\0WAVEfmt ",     // wav
            b"%PDF-1.7\n",
            b"MZ\x90\0",   // exe
            b"PK\x03\x04", // zip
            &[
                0x1a, 0x45, 0xdf, 0xa3, 0x42, 0x82, 0x88, b'm', b'a', b't', b'r', b'o', b's', b'k',
                b'a',
            ][..],
            b"",
            b"\x89PN",
        ] {
            assert_eq!(sniff(bad), None, "{:?}", String::from_utf8_lossy(bad));
        }
        // A `ftyp` box of an absurd size is not a video.
        let mut huge = vec![0xff, 0xff, 0xff, 0xff];
        huge.extend_from_slice(b"ftypisom\0\0\0\0");
        assert_eq!(sniff(&huge), None);
    }

    #[test]
    fn ids() {
        let h = "a".repeat(64);
        for ok in ["png", "jpg", "webp", "gif", "mp4", "webm"] {
            assert!(valid_asset_id(&format!("{h}.{ok}")), "{ok}");
        }
        for bad in [
            format!("{h}.svg"),
            format!("{h}.PNG"),
            format!("{h}.jpeg"),
            h.clone(),
            format!("{}.png", "a".repeat(63)),
            format!("{}.png", "A".repeat(64)),
            format!("{}.png", "g".repeat(64)),
            format!("../{}.png", "a".repeat(61)),
            "con.png".to_string(),
            "nul".to_string(),
            format!("{h}.png.exe"),
            String::new(),
        ] {
            assert!(!valid_asset_id(&bad), "{bad}");
        }
    }

    #[test]
    fn put_names_by_content_and_keeps_one_copy() {
        let dir = tmp_dir("put");
        let store = AssetStore::open(&dir).unwrap();
        let a = store.put(&png(1)).unwrap();
        assert_eq!(a.id, format!("{}.png", content_hash(&png(1))));
        assert_eq!(a.mime, "image/png");
        assert_eq!(a.size, png(1).len() as u64);
        assert!(dir.join("dash-assets").join(&a.id).is_file());
        let again = store.put(&png(1)).unwrap();
        assert_eq!(again, a);
        assert_eq!(store.list().len(), 1);
        let v = store.put(&mp4(1000)).unwrap();
        assert!(v.id.ends_with(".mp4"));
        assert_eq!(v.mime, "video/mp4");
        // Read back by ranges.
        assert_eq!(store.read_range(&v.id, 4, 4).unwrap(), b"ftyp");
        assert_eq!(store.read_range(&a.id, 0, 10_000).unwrap(), png(1));
        // Opened again: the same index.
        let reopened = AssetStore::open(&dir).unwrap();
        assert_eq!(reopened.list().len(), 2);
        assert_eq!(reopened.get(&a.id).unwrap().size, a.size);
        assert!(reopened.delete(&a.id).unwrap());
        assert!(!reopened.delete(&a.id).unwrap());
        assert!(!reopened.delete("../x.png").unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn put_refuses_unknown_types_and_large_files() {
        let dir = tmp_dir("refuse");
        let store = AssetStore::open(&dir).unwrap();
        assert!(matches!(
            store.put(b"<svg onload='alert(1)'/>"),
            Err(AssetError::Unsupported)
        ));
        assert!(matches!(store.put(b""), Err(AssetError::Unsupported)));
        let mut big = png(0);
        big.resize(MAX_IMAGE_BYTES as usize + 1, 0);
        assert!(matches!(
            store.put(&big),
            Err(AssetError::TooLarge(AssetKind::Png))
        ));
        big.truncate(MAX_IMAGE_BYTES as usize);
        assert!(store.put(&big).is_ok(), "exactly at the limit");
        assert!(matches!(
            store.put(&mp4(MAX_VIDEO_BYTES as usize + 1)),
            Err(AssetError::TooLarge(AssetKind::Mp4))
        ));
        assert!(fs::read_dir(dir.join("dash-assets")).unwrap().count() == 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn count_and_total_limits() {
        let dir = tmp_dir("full");
        let store = AssetStore::open(&dir).unwrap();
        for i in 0..MAX_ASSETS {
            let mut p = png(0);
            p.extend_from_slice(&(i as u32).to_be_bytes());
            store.put(&p).unwrap();
        }
        assert!(matches!(store.put(&png(250)), Err(AssetError::Full)));
        // The same content again is not a new file.
        let mut first = png(0);
        first.extend_from_slice(&0u32.to_be_bytes());
        assert!(store.put(&first).is_ok());
        let _ = fs::remove_dir_all(&dir);

        let dir = tmp_dir("bytes");
        let store = AssetStore::open(&dir).unwrap();
        // 10 videos of 30 MiB = the 300 MiB.
        for i in 0..10u8 {
            let mut v = mp4(MAX_VIDEO_BYTES as usize);
            v[100] = i;
            store.put(&v).unwrap();
        }
        assert_eq!(store.usage(), (10, MAX_TOTAL_BYTES));
        assert!(matches!(store.put(&png(9)), Err(AssetError::Full)));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_skips_files_that_are_not_what_their_name_says() {
        let dir = tmp_dir("open");
        let assets = dir.join("dash-assets");
        fs::create_dir_all(&assets).unwrap();
        let h = "b".repeat(64);
        fs::write(assets.join(format!("{h}.png")), b"<svg/>").unwrap();
        fs::write(assets.join(format!("{h}.gif")), png(0)).unwrap();
        fs::write(assets.join("notes.txt"), b"x").unwrap();
        fs::write(assets.join(format!("{h}.jpg.tmp")), b"x").unwrap();
        fs::write(assets.join(format!("{h}.webp")), b"RIFF\0\0\0\0WEBPVP8 ").unwrap();
        let store = AssetStore::open(&dir).unwrap();
        let ids: Vec<String> = store.list().into_iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![format!("{h}.webp")]);
        assert!(!assets.join(format!("{h}.jpg.tmp")).exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn prune_removes_old_unreferenced_files_only() {
        let dir = tmp_dir("prune");
        let store = AssetStore::open(&dir).unwrap();
        let used = store.put(&png(1)).unwrap();
        let old = store.put(&png(2)).unwrap();
        let fresh = store.put(&png(3)).unwrap();
        let past = SystemTime::now() - PRUNE_GRACE - Duration::from_secs(60);
        for id in [&used.id, &old.id] {
            fs::File::options()
                .write(true)
                .open(store.dir().join(id))
                .unwrap()
                .set_modified(past)
                .unwrap();
        }
        let referenced: HashSet<String> = [used.id.clone()].into_iter().collect();
        assert_eq!(store.prune(&referenced, PRUNE_GRACE), 1);
        let ids: HashSet<String> = store.list().into_iter().map(|a| a.id).collect();
        assert!(ids.contains(&used.id));
        assert!(ids.contains(&fresh.id), "sent a moment ago: kept");
        assert!(!ids.contains(&old.id));
        assert!(!store.dir().join(&old.id).exists());
        // Without a grace period, the fresh one goes too.
        assert_eq!(store.prune(&referenced, Duration::ZERO), 1);
        assert_eq!(store.list().len(), 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn ranges() {
        assert_eq!(parse_range(None, 100), Ok(None));
        assert_eq!(parse_range(Some("bytes=0-1"), 100), Ok(Some((0, 1))));
        assert_eq!(parse_range(Some("bytes=0-"), 100), Ok(Some((0, 99))));
        assert_eq!(parse_range(Some("bytes=50-1000"), 100), Ok(Some((50, 99))));
        assert_eq!(parse_range(Some("bytes=-10"), 100), Ok(Some((90, 99))));
        assert_eq!(parse_range(Some("bytes=-1000"), 100), Ok(Some((0, 99))));
        assert_eq!(parse_range(Some("bytes=0-1,5-6"), 100), Ok(None));
        assert_eq!(parse_range(Some("items=0-1"), 100), Ok(None));
        for bad in [
            "bytes=100-",
            "bytes=5-2",
            "bytes=x-",
            "bytes=-0",
            "bytes=-",
            "bytes=1",
        ] {
            assert_eq!(parse_range(Some(bad), 100), Err(BadRange), "{bad}");
        }
    }
}
