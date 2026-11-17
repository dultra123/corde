//! Copies of what the game maps.
//!
//! * Windows: the named mapping of the game, opened **read-only** ([`WindowsMap`]).
//! * Everywhere: one file holding an image of the mapping ([`FileMap`]) — what the tests
//!   and `iracing-probe --file` use, written by [`Image`]. The game itself only runs on
//!   Windows.
//!
//! The game takes no lock. It writes its rows in turn and says in the header which one
//! is the newest; a row is copied, then the header is read again: if the game came back
//! to that row meanwhile, the copy is taken again.
//!
//! Every offset and length of the header is checked against the size of the mapping
//! before anything is read.

use crate::layout::{
    Header, VarBuf, VarType, HEADER_SIZE, MAX_ROW, MAX_SESSION_TEXT, MAX_VARS, SDK_VERSION,
    STATUS_CONNECTED, VAR_HEADER_SIZE,
};
use crate::vars::VarTable;
use crate::yaml;
use corde_core::SourceError;
use std::path::{Path, PathBuf};

/// How many times a row is copied again when the game rewrote it meanwhile.
const COPY_TRIES: usize = 4;

/// Bytes of the mapping, copied on demand.
pub trait RawMap: Send {
    /// `len` bytes at `at`. [`SourceError::Invalid`] when that is outside the mapping,
    /// [`SourceError::NotAvailable`] when the mapping is gone.
    fn copy(&mut self, at: usize, len: usize) -> Result<Vec<u8>, SourceError>;
    /// One line for the logs: where the bytes come from.
    fn describe(&self) -> String;
}

fn non_negative(v: i32, what: &str) -> Result<usize, SourceError> {
    usize::try_from(v).map_err(|_| SourceError::Invalid(format!("{what} négatif ({v})")))
}

/// The header, as it is now.
pub fn header(map: &mut dyn RawMap) -> Result<Header, SourceError> {
    let bytes = map.copy(0, HEADER_SIZE)?;
    let bytes: &[u8; HEADER_SIZE] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| SourceError::Invalid("en-tête tronqué".into()))?;
    Ok(Header::parse(bytes))
}

/// Refuses an interface this module does not know.
pub fn check_version(h: &Header) -> Result<(), SourceError> {
    if h.ver == SDK_VERSION {
        Ok(())
    } else {
        Err(SourceError::NotAvailable(format!(
            "interface {} du jeu, {SDK_VERSION} attendue",
            h.ver
        )))
    }
}

/// The session text the header points at.
pub fn session_text(map: &mut dyn RawMap, h: &Header) -> Result<String, SourceError> {
    let len = non_negative(h.session_info_len, "taille du texte de session")?;
    if len > MAX_SESSION_TEXT {
        return Err(SourceError::Invalid(format!(
            "texte de session de {len} octets"
        )));
    }
    let at = non_negative(h.session_info_offset, "position du texte de session")?;
    Ok(yaml::decode(&map.copy(at, len)?))
}

/// The table of the variables the header points at.
pub fn var_table(map: &mut dyn RawMap, h: &Header) -> Result<VarTable, SourceError> {
    let count = non_negative(h.num_vars, "nombre de variables")?;
    let row_len = non_negative(h.buf_len, "taille d'une ligne")?;
    if count > MAX_VARS || row_len > MAX_ROW {
        return Err(SourceError::Invalid(format!(
            "{count} variables, lignes de {row_len} octets"
        )));
    }
    let at = non_negative(h.var_header_offset, "position de la table")?;
    Ok(VarTable::parse(
        &map.copy(at, count * VAR_HEADER_SIZE)?,
        row_len,
    ))
}

/// The newest row, with the header it was found through.
#[derive(Debug, Clone)]
pub struct Tick {
    pub header: Header,
    pub tick: i32,
    pub row: Vec<u8>,
}

/// Copies the newest row.
pub fn latest_row(map: &mut dyn RawMap) -> Result<Tick, SourceError> {
    let mut last = None;
    for _ in 0..COPY_TRIES {
        let h = header(map)?;
        check_version(&h)?;
        let (index, buf) = h
            .latest()
            .ok_or_else(|| SourceError::Invalid("aucune ligne de valeurs".into()))?;
        let len = non_negative(h.buf_len, "taille d'une ligne")?;
        if len > MAX_ROW {
            return Err(SourceError::Invalid(format!("lignes de {len} octets")));
        }
        let at = non_negative(buf.buf_offset, "position d'une ligne")?;
        let row = map.copy(at, len)?;
        let unchanged = header(map)?.var_buf[index].tick_count == buf.tick_count;
        last = Some(Tick {
            header: h,
            tick: buf.tick_count,
            row,
        });
        if unchanged {
            break;
        }
    }
    last.ok_or_else(|| SourceError::Invalid("aucune ligne de valeurs".into()))
}

/// A file holding an image of the mapping, opened at each read (it is replaced whole).
pub struct FileMap {
    path: PathBuf,
}

impl FileMap {
    pub fn open(path: &Path) -> Result<Self, SourceError> {
        if !path.is_file() {
            return Err(SourceError::NotAvailable(format!(
                "{} : pas de fichier",
                path.display()
            )));
        }
        Ok(Self {
            path: path.to_path_buf(),
        })
    }
}

impl RawMap for FileMap {
    fn copy(&mut self, at: usize, len: usize) -> Result<Vec<u8>, SourceError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = std::fs::File::open(&self.path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SourceError::NotAvailable(format!("{} a disparu", self.path.display()))
            } else {
                SourceError::Io(e)
            }
        })?;
        let size = file.metadata()?.len();
        let end = (at as u64).checked_add(len as u64);
        if end.is_none_or(|end| end > size) {
            return Err(SourceError::Invalid(format!(
                "{len} octets en {at} : le fichier en fait {size}"
            )));
        }
        file.seek(SeekFrom::Start(at as u64))?;
        let mut out = vec![0u8; len];
        file.read_exact(&mut out)?;
        Ok(out)
    }

    fn describe(&self) -> String {
        format!("fichier {}", self.path.display())
    }
}

/// The reader of this platform: the game's mapping on Windows; elsewhere, or when `file`
/// is given, an image in a file.
pub fn open_default_map(file: Option<&Path>) -> Result<Box<dyn RawMap>, SourceError> {
    if let Some(file) = file {
        return Ok(Box::new(FileMap::open(file)?));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(WindowsMap::open()?))
    }
    #[cfg(not(windows))]
    {
        Err(SourceError::NotAvailable(
            "iRacing ne tourne que sous Windows".into(),
        ))
    }
}

/// A whole mapping, made here: what the tests and the fake game write, and the
/// diagnostic copy of `iracing-probe --dump`.
#[derive(Debug, Clone)]
pub struct Image {
    pub status: i32,
    pub tick_rate: i32,
    pub session_info_update: i32,
    pub session_text: String,
    pub table: VarTable,
    /// Rows with their tick counts (4 at most are written).
    pub rows: Vec<(i32, Vec<u8>)>,
}

impl Image {
    /// What `map` holds now: header, session text, table and the newest row.
    pub fn of(map: &mut dyn RawMap) -> Result<Self, SourceError> {
        let tick = latest_row(map)?;
        let h = tick.header;
        Ok(Self {
            status: h.status,
            tick_rate: h.tick_rate,
            session_info_update: h.session_info_update,
            session_text: session_text(map, &h)?,
            table: var_table(map, &h)?,
            rows: vec![(tick.tick, tick.row)],
        })
    }

    /// The same without anything that names or identifies somebody: the session text
    /// keeps only what describes the circuit, the session and the cars
    /// ([`yaml::anonymise`]), and the text variables of the rows are blanked.
    pub fn for_diagnostic(mut self) -> Self {
        self.session_text = yaml::anonymise(&self.session_text);
        for v in self.table.vars() {
            if v.kind != VarType::Char {
                continue;
            }
            for (_, row) in &mut self.rows {
                if let Some(text) = row.get_mut(v.offset..v.offset + v.len()) {
                    text.fill(0);
                }
            }
        }
        self
    }

    /// The bytes of the mapping: header, session text, table, rows.
    pub fn to_bytes(&self) -> Vec<u8> {
        // The text as the game writes it: one byte per character unless it says UTF-8.
        let utf8 = self
            .session_text
            .lines()
            .take(4)
            .any(|l| l.trim() == "Encoding: UTF8");
        let mut text: Vec<u8> = if utf8 {
            self.session_text.as_bytes().to_vec()
        } else {
            self.session_text
                .chars()
                .map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
                .collect()
        };
        text.push(0);
        let table = self.table.to_bytes();
        let row_len = self.table.row_len();
        let align = |v: usize| v.div_ceil(16) * 16;
        let text_at = HEADER_SIZE;
        let table_at = align(text_at + text.len());
        let rows_at = align(table_at + table.len());
        let rows = &self.rows[..self.rows.len().min(crate::layout::MAX_BUFS)];
        let mut var_buf = [VarBuf::default(); crate::layout::MAX_BUFS];
        for (i, (tick, _)) in rows.iter().enumerate() {
            var_buf[i] = VarBuf {
                tick_count: *tick,
                buf_offset: (rows_at + i * row_len) as i32,
            };
        }
        let header = Header {
            ver: SDK_VERSION,
            status: self.status,
            tick_rate: self.tick_rate,
            session_info_update: self.session_info_update,
            session_info_len: text.len() as i32,
            session_info_offset: text_at as i32,
            num_vars: self.table.len() as i32,
            var_header_offset: table_at as i32,
            num_buf: rows.len() as i32,
            buf_len: row_len as i32,
            var_buf,
        };
        let mut out = vec![0u8; rows_at + rows.len() * row_len];
        out[..HEADER_SIZE].copy_from_slice(&header.to_bytes());
        out[text_at..text_at + text.len()].copy_from_slice(&text);
        out[table_at..table_at + table.len()].copy_from_slice(&table);
        for (i, (_, row)) in rows.iter().enumerate() {
            let at = rows_at + i * row_len;
            let n = row.len().min(row_len);
            out[at..at + n].copy_from_slice(&row[..n]);
        }
        out
    }

    /// Written as the file a [`FileMap`] reads (whole, then put in place).
    pub fn write(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("tmp");
        std::fs::write(&tmp, self.to_bytes())?;
        std::fs::rename(&tmp, path)
    }

    /// In a session (the bit the game sets while one is running).
    pub fn connected(&self) -> bool {
        self.status & STATUS_CONNECTED != 0
    }
}

/// An image held in memory (the tests).
pub struct MemMap(pub Vec<u8>);

impl RawMap for MemMap {
    fn copy(&mut self, at: usize, len: usize) -> Result<Vec<u8>, SourceError> {
        at.checked_add(len)
            .and_then(|end| self.0.get(at..end))
            .map(<[u8]>::to_vec)
            .ok_or_else(|| {
                SourceError::Invalid(format!(
                    "{len} octets en {at} : l'image en fait {}",
                    self.0.len()
                ))
            })
    }

    fn describe(&self) -> String {
        "image en mémoire".into()
    }
}

#[cfg(windows)]
pub use windows_impl::WindowsMap;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows_sys::core::w;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FALSE, HANDLE};
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
        MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    };

    /// The mapping of the running game, read-only.
    pub struct WindowsMap {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        size: usize,
    }

    // SAFETY: the view is only read, and only through `&mut self`.
    unsafe impl Send for WindowsMap {}

    impl WindowsMap {
        pub fn open() -> Result<Self, SourceError> {
            // SAFETY: plain Win32 call with a valid NUL-terminated wide name.
            let handle =
                unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, w!("Local\\IRSDKMemMapFileName")) };
            if handle.is_null() {
                // SAFETY: no precondition.
                let err = unsafe { GetLastError() };
                let hint = match err {
                    2 => "le jeu n'est pas lancé",
                    5 => "accès refusé : le jeu tourne probablement en administrateur ; lance Corde en administrateur aussi",
                    _ => "le jeu est-il lancé ?",
                };
                return Err(SourceError::NotAvailable(format!(
                    "OpenFileMappingW(\"{}\") : erreur {err} ({hint})",
                    crate::layout::MAP_NAME
                )));
            }
            // SAFETY: `handle` is a valid mapping handle; size 0 maps the whole section.
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
            if view.Value.is_null() {
                // SAFETY: no precondition; the handle is valid and owned by us.
                let err = unsafe { GetLastError() };
                unsafe { CloseHandle(handle) };
                return Err(SourceError::Other(format!(
                    "MapViewOfFile(\"{}\") : erreur {err}",
                    crate::layout::MAP_NAME
                )));
            }
            // How large the view really is, before trusting any offset.
            let mut info = MEMORY_BASIC_INFORMATION::default();
            // SAFETY: `info` is a properly sized out-parameter; `view` is a valid address.
            let got = unsafe {
                VirtualQuery(
                    view.Value,
                    &mut info,
                    std::mem::size_of::<MEMORY_BASIC_INFORMATION>(),
                )
            };
            let size = if got == 0 { 0 } else { info.RegionSize };
            if size < HEADER_SIZE {
                // SAFETY: both were obtained above and are owned by us.
                unsafe {
                    UnmapViewOfFile(view);
                    CloseHandle(handle);
                }
                return Err(SourceError::Invalid(format!(
                    "{} : {size} octets, moins qu'un en-tête",
                    crate::layout::MAP_NAME
                )));
            }
            Ok(Self { handle, view, size })
        }
    }

    impl RawMap for WindowsMap {
        fn copy(&mut self, at: usize, len: usize) -> Result<Vec<u8>, SourceError> {
            if at.checked_add(len).is_none_or(|end| end > self.size) {
                return Err(SourceError::Invalid(format!(
                    "{len} octets en {at} : la mémoire du jeu en fait {}",
                    self.size
                )));
            }
            let mut out = vec![0u8; len];
            // SAFETY: `at + len` is inside the view (checked above), which stays mapped
            // while `self` lives; reads of memory another process writes are racy by
            // nature, hence the tick count check of `latest_row`.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    (self.view.Value as *const u8).add(at),
                    out.as_mut_ptr(),
                    len,
                )
            };
            Ok(out)
        }

        fn describe(&self) -> String {
            format!("mémoire partagée du jeu ({})", crate::layout::MAP_NAME)
        }
    }

    impl Drop for WindowsMap {
        fn drop(&mut self) {
            // SAFETY: both were obtained in `open()` and are owned by us.
            unsafe {
                UnmapViewOfFile(self.view);
                CloseHandle(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::VarType;

    fn image() -> Image {
        let table = VarTable::of(&[
            ("SessionTime", VarType::Double, 1),
            ("Speed", VarType::Float, 1),
            ("Note", VarType::Char, 8),
        ]);
        let mut old = table.new_row();
        table.put(&mut old, "Speed", 0, 10.0);
        let mut new = table.new_row();
        table.put(&mut new, "Speed", 0, 20.0);
        table.put(&mut new, "Note", 0, b'x' as f64);
        Image {
            status: STATUS_CONNECTED,
            tick_rate: 60,
            session_info_update: 3,
            session_text: "---\nWeekendInfo:\n TrackName: spa up\nDriverInfo:\n Drivers:\n - CarIdx: 0\n   UserName: Somebody Real\n...\n".into(),
            table,
            rows: vec![(41, old), (42, new)],
        }
    }

    #[test]
    fn an_image_reads_back_as_it_was_written() {
        let img = image();
        let mut map = MemMap(img.to_bytes());
        let h = header(&mut map).unwrap();
        assert!(h.connected());
        assert_eq!((h.ver, h.tick_rate, h.num_buf), (SDK_VERSION, 60, 2));
        assert_eq!(session_text(&mut map, &h).unwrap(), img.session_text);
        let table = var_table(&mut map, &h).unwrap();
        assert_eq!(table.vars(), img.table.vars());
        // The newest of the rows.
        let tick = latest_row(&mut map).unwrap();
        assert_eq!(tick.tick, 42);
        assert_eq!(table.row(&tick.row).f32("Speed"), 20.0);
        // And the whole again.
        let again = Image::of(&mut map).unwrap();
        assert_eq!(again.session_text, img.session_text);
        assert_eq!(again.rows, [img.rows[1].clone()]);
    }

    #[test]
    fn a_diagnostic_copy_names_nobody() {
        let copy = image().for_diagnostic();
        assert!(!copy.session_text.contains("Somebody"));
        assert!(copy.session_text.contains("TrackName: spa up"));
        assert!(
            copy.session_text.contains("UserName:"),
            "the structure stays"
        );
        let note = copy.table.get("Note").unwrap().offset;
        assert!(copy.rows.iter().all(|(_, row)| row[note] == 0));
        let bytes = copy.to_bytes();
        assert!(!bytes.windows(8).any(|w| w == b"Somebody"));
    }

    #[test]
    fn nothing_outside_the_mapping_is_read() {
        let good = image().to_bytes();
        // Each field of the header that says where something is, pushed out of bounds or
        // made negative: an error, never a read elsewhere and never a panic.
        for (at, value) in [
            (16, i32::MAX), // length of the session text
            (16, -1),
            (20, i32::MAX), // where the session text is
            (20, -5),
            (24, i32::MAX), // number of variables
            (24, -1),
            (28, i32::MAX - 100), // where the table is
            (36, i32::MAX),       // length of a row
            (36, -1),
            (52, i32::MAX - 10), // where the first row is
            (68, -1),            // where the second row is
        ] {
            let mut bad = good.clone();
            bad[at..at + 4].copy_from_slice(&value.to_le_bytes());
            let mut map = MemMap(bad);
            let h = header(&mut map).unwrap();
            let all = session_text(&mut map, &h)
                .and_then(|_| var_table(&mut map, &h))
                .and_then(|_| latest_row(&mut map));
            // The first row is the older one: a wrong offset there is never followed.
            if at == 52 {
                assert!(all.is_ok());
            } else {
                assert!(
                    matches!(all, Err(SourceError::Invalid(_))),
                    "field at {at} = {value}: {all:?}"
                );
            }
        }
        // Another interface version: not ours.
        let mut other = good.clone();
        other[0..4].copy_from_slice(&3i32.to_le_bytes());
        assert!(matches!(
            latest_row(&mut MemMap(other)),
            Err(SourceError::NotAvailable(_))
        ));
        // Less than a header.
        assert!(header(&mut MemMap(good[..50].to_vec())).is_err());
        // No row at all.
        let mut none = good;
        none[32..36].copy_from_slice(&0i32.to_le_bytes());
        assert!(matches!(
            latest_row(&mut MemMap(none)),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn a_file_is_read_like_the_mapping() {
        let dir = std::env::temp_dir().join(format!("corde-iracing-reader-{}", std::process::id()));
        let path = dir.join("iracing.map");
        assert!(matches!(
            FileMap::open(&path),
            Err(SourceError::NotAvailable(_))
        ));
        image().write(&path).unwrap();
        let mut map = FileMap::open(&path).unwrap();
        assert_eq!(latest_row(&mut map).unwrap().tick, 42);
        assert!(matches!(map.copy(1 << 30, 4), Err(SourceError::Invalid(_))));
        assert!(matches!(
            map.copy(usize::MAX, 4),
            Err(SourceError::Invalid(_))
        ));
        std::fs::remove_file(&path).unwrap();
        assert!(matches!(map.copy(0, 4), Err(SourceError::NotAvailable(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
