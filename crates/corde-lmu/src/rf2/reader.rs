//! Reading the two buffers of rFactor 2's plugin: from the game's named mappings on
//! Windows, from two files of a folder elsewhere (tests, the fake game, a diagnostic copy).
//!
//! Read-only: the mappings are opened with `FILE_MAP_READ`, nothing is ever written to
//! the game, and none of the plugin's input buffers (the ones that accept commands) is
//! opened.

use super::layout::{
    i32_at, u32_at, MAX_VEHICLES, OFF_SCORING_INFO, OFF_SCORING_VEHICLES, OFF_TELEM_NUM,
    OFF_VERSION_BEGIN, OFF_VERSION_END, SCORING_INFO_PLAYER_NAME, SCORING_SIZE, TELEMETRY_SIZE,
    VEH_SCORING_DRIVER_NAME,
};
use crate::layout::SIZE_VEHICLE_SCORING_INFO;
use corde_core::SourceError;
use std::path::{Path, PathBuf};

/// File of the telemetry buffer in a folder.
pub const TELEMETRY_FILE: &str = "rf2-telemetry.bin";
/// File of the scoring buffer in a folder.
pub const SCORING_FILE: &str = "rf2-scoring.bin";

/// `ScoringInfoV01::mNumVehicles` inside the scoring buffer.
const OFF_SCORING_NUM: usize = OFF_SCORING_INFO + 104;

/// A copy of the two buffers, each of its full size.
#[derive(Clone)]
pub struct Rf2Raw {
    pub telemetry: Vec<u8>,
    pub scoring: Vec<u8>,
}

impl Default for Rf2Raw {
    fn default() -> Self {
        Self::new()
    }
}

impl Rf2Raw {
    /// Two blank buffers.
    pub fn new() -> Self {
        Self {
            telemetry: vec![0; TELEMETRY_SIZE],
            scoring: vec![0; SCORING_SIZE],
        }
    }

    /// The write counters of the two buffers (telemetry, scoring).
    pub fn versions(&self) -> (u32, u32) {
        (
            u32_at(&self.telemetry, OFF_VERSION_BEGIN),
            u32_at(&self.scoring, OFF_VERSION_BEGIN),
        )
    }

    /// Vehicles the telemetry buffer announces, within what it can hold.
    pub fn telemetry_vehicles(&self) -> usize {
        i32_at(&self.telemetry, OFF_TELEM_NUM).clamp(0, MAX_VEHICLES as i32) as usize
    }

    /// Vehicles the scoring buffer announces, within what it can hold.
    pub fn scoring_vehicles(&self) -> usize {
        i32_at(&self.scoring, OFF_SCORING_NUM).clamp(0, MAX_VEHICLES as i32) as usize
    }

    /// The same copy without any name of a person: the player's and every driver's.
    /// What a diagnostic copy keeps.
    pub fn without_names(mut self) -> Self {
        let info = OFF_SCORING_INFO;
        self.scoring[info + SCORING_INFO_PLAYER_NAME.start..info + SCORING_INFO_PLAYER_NAME.end]
            .fill(0);
        for i in 0..MAX_VEHICLES {
            let v = OFF_SCORING_VEHICLES + i * SIZE_VEHICLE_SCORING_INFO;
            self.scoring[v + VEH_SCORING_DRIVER_NAME.start..v + VEH_SCORING_DRIVER_NAME.end]
                .fill(0);
        }
        self
    }

    /// Writes the two buffers as the two files of `dir` (created if missing).
    pub fn write_dir(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        write_whole(&dir.join(TELEMETRY_FILE), &self.telemetry)?;
        write_whole(&dir.join(SCORING_FILE), &self.scoring)
    }
}

/// Written beside, then renamed: a reader never sees half a file.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

/// Is a buffer whole (its two counters agree)?
fn whole(buf: &[u8]) -> bool {
    u32_at(buf, OFF_VERSION_BEGIN) == u32_at(buf, OFF_VERSION_END)
}

/// Something that gives the two buffers.
pub trait Rf2Reader: Send {
    /// The write counters right now (telemetry, scoring), without copying the buffers.
    fn versions(&mut self) -> Result<(u32, u32), SourceError>;

    /// Copies the two buffers into `out`. `false`: one of them was being written on every
    /// try — `out` is then not to be used.
    fn read(&mut self, out: &mut Rf2Raw) -> Result<bool, SourceError>;

    /// Where the data comes from, for the log.
    fn describe(&self) -> String;
}

/// The two files of a folder.
pub struct DirReader {
    dir: PathBuf,
}

impl DirReader {
    pub fn open(dir: &Path) -> Result<Self, SourceError> {
        for f in [TELEMETRY_FILE, SCORING_FILE] {
            if !dir.join(f).is_file() {
                return Err(SourceError::NotAvailable(format!(
                    "{} : fichier {f} absent",
                    dir.display()
                )));
            }
        }
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    fn file(&self, name: &str, size: usize) -> Result<Vec<u8>, SourceError> {
        let path = self.dir.join(name);
        let bytes = std::fs::read(&path)
            .map_err(|e| SourceError::NotAvailable(format!("{} : {e}", path.display())))?;
        if bytes.len() < size {
            return Err(SourceError::Invalid(format!(
                "{name} : {} octets, {size} attendus",
                bytes.len()
            )));
        }
        Ok(bytes)
    }
}

impl Rf2Reader for DirReader {
    fn versions(&mut self) -> Result<(u32, u32), SourceError> {
        use std::io::Read;
        let mut out = [0u32; 2];
        for (i, name) in [TELEMETRY_FILE, SCORING_FILE].into_iter().enumerate() {
            let path = self.dir.join(name);
            let mut head = [0u8; 4];
            std::fs::File::open(&path)
                .and_then(|mut f| f.read_exact(&mut head))
                .map_err(|e| SourceError::NotAvailable(format!("{} : {e}", path.display())))?;
            out[i] = u32::from_le_bytes(head);
        }
        Ok((out[0], out[1]))
    }

    fn read(&mut self, out: &mut Rf2Raw) -> Result<bool, SourceError> {
        let telemetry = self.file(TELEMETRY_FILE, TELEMETRY_SIZE)?;
        let scoring = self.file(SCORING_FILE, SCORING_SIZE)?;
        if !whole(&telemetry) || !whole(&scoring) {
            return Ok(false);
        }
        out.telemetry.clear();
        out.telemetry
            .extend_from_slice(&telemetry[..TELEMETRY_SIZE]);
        out.scoring.clear();
        out.scoring.extend_from_slice(&scoring[..SCORING_SIZE]);
        Ok(true)
    }

    fn describe(&self) -> String {
        format!("fichiers de {}", self.dir.display())
    }
}

/// How many bytes of the telemetry buffer are in use for `num` vehicles.
#[cfg(any(windows, test))]
fn telemetry_used(num: i32) -> usize {
    super::layout::OFF_TELEM_VEHICLES
        + num.clamp(0, MAX_VEHICLES as i32) as usize * crate::layout::SIZE_TELEM_INFO
}

/// How many bytes of the scoring buffer are in use for `num` vehicles.
#[cfg(any(windows, test))]
fn scoring_used(num: i32) -> usize {
    OFF_SCORING_VEHICLES + num.clamp(0, MAX_VEHICLES as i32) as usize * SIZE_VEHICLE_SCORING_INFO
}

/// The reader of the running game (Windows), or of the files of `dir` when one is given.
pub fn open_reader(dir: Option<&Path>) -> Result<Box<dyn Rf2Reader>, SourceError> {
    if let Some(dir) = dir {
        return Ok(Box::new(DirReader::open(dir)?));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(windows_impl::WindowsReader::open()?))
    }
    #[cfg(not(windows))]
    {
        Err(SourceError::NotAvailable(
            "rFactor 2 : la mémoire partagée du jeu n'existe que sous Windows (ou donne un dossier avec --rf2-shm)"
                .into(),
        ))
    }
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use crate::rf2::layout::{SCORING_NAME, TELEMETRY_NAME};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FALSE, HANDLE};
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
        MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    };

    /// How many times a copy is retaken when the buffer was rewritten meanwhile.
    const COPY_TRIES: usize = 4;

    /// One mapping of the plugin, read-only.
    struct Mapping {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        size: usize,
        /// How much of it the last copy took.
        last_used: usize,
    }

    // SAFETY: the view is only read, and only through `&mut self` of the owning reader.
    unsafe impl Send for Mapping {}

    impl Mapping {
        fn open(name: &str, size: usize) -> Result<Self, SourceError> {
            let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
            // SAFETY: plain Win32 call with a valid NUL-terminated wide name that lives
            // until the call returns.
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, wide.as_ptr()) };
            if handle.is_null() {
                // SAFETY: no precondition.
                let err = unsafe { GetLastError() };
                let hint = match err {
                    2 => "le jeu n'est pas lancé, ou son plugin de mémoire partagée n'est pas activé",
                    5 => "accès refusé : le jeu tourne probablement en administrateur ; lance Corde en administrateur aussi",
                    _ => "le jeu est-il lancé ?",
                };
                return Err(SourceError::NotAvailable(format!(
                    "OpenFileMappingW(\"{name}\") : erreur {err} ({hint})"
                )));
            }
            // SAFETY: `handle` is a valid mapping handle; size 0 maps the whole section.
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
            if view.Value.is_null() {
                // SAFETY: no precondition; the handle is valid and owned by us.
                let err = unsafe { GetLastError() };
                unsafe { CloseHandle(handle) };
                return Err(SourceError::Other(format!(
                    "MapViewOfFile(\"{name}\") : erreur {err}"
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
            let region = if got == 0 { 0 } else { info.RegionSize };
            if region < size {
                // SAFETY: both were obtained above and are owned by us.
                unsafe {
                    UnmapViewOfFile(view);
                    CloseHandle(handle);
                }
                return Err(SourceError::Invalid(format!(
                    "{name} : {region} octets, {size} attendus (autre version du plugin ?)"
                )));
            }
            Ok(Self {
                handle,
                view,
                size,
                last_used: size,
            })
        }

        /// Four bytes of the mapping as a little-endian number. `off + 4 <= self.size`.
        fn u32(&self, off: usize) -> u32 {
            debug_assert!(off + 4 <= self.size);
            let mut b = [0u8; 4];
            // SAFETY: the view is mapped for at least `self.size` bytes (checked in
            // `open`) and stays mapped while `self` lives; `off + 4 <= self.size`.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    (self.view.Value as *const u8).add(off),
                    b.as_mut_ptr(),
                    4,
                )
            };
            u32::from_le_bytes(b)
        }

        /// Copies what is in use (`used` says how much, from the number of vehicles read
        /// at `num_off`) into `out`, whose length is `self.size`. `false` when the plugin
        /// was writing on every try.
        fn copy(&mut self, out: &mut [u8], num_off: usize, used: fn(i32) -> usize) -> bool {
            debug_assert_eq!(out.len(), self.size);
            let base = self.view.Value as *const u8;
            for _ in 0..COPY_TRIES {
                let begin = self.u32(OFF_VERSION_BEGIN);
                if begin != self.u32(OFF_VERSION_END) {
                    std::hint::spin_loop();
                    continue;
                }
                let n = used(self.u32(num_off) as i32).min(self.size).min(out.len());
                // SAFETY: the view is mapped for at least `self.size` bytes and stays
                // mapped while `self` lives; `n` is within both the view and `out`. Reads
                // of memory another process writes are racy by nature, hence the counters
                // checked before and after.
                unsafe { std::ptr::copy_nonoverlapping(base, out.as_mut_ptr(), n) };
                if self.u32(OFF_VERSION_BEGIN) == begin && whole(out) {
                    // What an earlier, larger copy left behind is not this copy's.
                    if n < self.last_used {
                        let end = self.last_used.min(out.len());
                        out[n..end].fill(0);
                    }
                    self.last_used = n;
                    return true;
                }
            }
            false
        }
    }

    impl Drop for Mapping {
        fn drop(&mut self) {
            // SAFETY: both were obtained in `open()` and are owned by us.
            unsafe {
                UnmapViewOfFile(self.view);
                CloseHandle(self.handle);
            }
        }
    }

    /// The two mappings of the running game.
    pub struct WindowsReader {
        telemetry: Mapping,
        scoring: Mapping,
    }

    /// Does a mapping of this name exist right now?
    fn mapping_exists(name: &str) -> bool {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: plain Win32 call with a valid NUL-terminated wide name that lives
        // until the call returns.
        let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, wide.as_ptr()) };
        if handle.is_null() {
            return false;
        }
        // SAFETY: the handle is valid and owned by us.
        unsafe { CloseHandle(handle) };
        true
    }

    impl WindowsReader {
        pub fn open() -> Result<Self, SourceError> {
            // Le Mans Ultimate can load the same plugin (other tools install it there):
            // these buffers would then be its own. Its native block says so, and the
            // Le Mans Ultimate adapter is the one that reads that game.
            if mapping_exists(crate::layout::SHM_FILE_NAME) {
                return Err(SourceError::NotAvailable(
                    "ces zones sont celles de Le Mans Ultimate, pas de rFactor 2".into(),
                ));
            }
            Ok(Self {
                // Scoring first: it is the one that says whether a session is live.
                scoring: Mapping::open(SCORING_NAME, SCORING_SIZE)?,
                telemetry: Mapping::open(TELEMETRY_NAME, TELEMETRY_SIZE)?,
            })
        }
    }

    impl Rf2Reader for WindowsReader {
        fn versions(&mut self) -> Result<(u32, u32), SourceError> {
            Ok((
                self.telemetry.u32(OFF_VERSION_BEGIN),
                self.scoring.u32(OFF_VERSION_BEGIN),
            ))
        }

        fn read(&mut self, out: &mut Rf2Raw) -> Result<bool, SourceError> {
            out.telemetry.resize(TELEMETRY_SIZE, 0);
            out.scoring.resize(SCORING_SIZE, 0);
            Ok(self
                .telemetry
                .copy(&mut out.telemetry, OFF_TELEM_NUM, telemetry_used)
                && self
                    .scoring
                    .copy(&mut out.scoring, OFF_SCORING_NUM, scoring_used))
        }

        fn describe(&self) -> String {
            format!("mémoire partagée du plugin de rFactor 2 ({SCORING_NAME})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::SIZE_TELEM_INFO;
    use crate::rf2::layout::OFF_TELEM_VEHICLES;

    #[test]
    fn used_sizes_follow_the_number_of_vehicles() {
        assert_eq!(telemetry_used(0), OFF_TELEM_VEHICLES);
        assert_eq!(telemetry_used(1), OFF_TELEM_VEHICLES + SIZE_TELEM_INFO);
        assert_eq!(telemetry_used(500), TELEMETRY_SIZE);
        assert_eq!(telemetry_used(-3), OFF_TELEM_VEHICLES);
        assert_eq!(scoring_used(128), SCORING_SIZE);
        assert_eq!(scoring_used(-1), OFF_SCORING_VEHICLES);
    }

    #[test]
    fn a_folder_gives_back_what_was_written_and_refuses_a_buffer_being_written() {
        let dir = std::env::temp_dir().join(format!("corde-rf2-reader-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(DirReader::open(&dir).is_err());

        let mut raw = Rf2Raw::new();
        raw.telemetry[OFF_VERSION_BEGIN..OFF_VERSION_BEGIN + 4]
            .copy_from_slice(&7u32.to_le_bytes());
        raw.telemetry[OFF_VERSION_END..OFF_VERSION_END + 4].copy_from_slice(&7u32.to_le_bytes());
        raw.scoring[OFF_VERSION_BEGIN..OFF_VERSION_BEGIN + 4].copy_from_slice(&3u32.to_le_bytes());
        raw.scoring[OFF_VERSION_END..OFF_VERSION_END + 4].copy_from_slice(&3u32.to_le_bytes());
        raw.write_dir(&dir).unwrap();

        let mut reader = DirReader::open(&dir).unwrap();
        assert_eq!(reader.versions().unwrap(), (7, 3));
        let mut out = Rf2Raw::new();
        assert!(reader.read(&mut out).unwrap());
        assert_eq!(out.versions(), (7, 3));

        // The plugin is in the middle of a write: the first counter is ahead.
        raw.scoring[OFF_VERSION_BEGIN..OFF_VERSION_BEGIN + 4].copy_from_slice(&4u32.to_le_bytes());
        raw.write_dir(&dir).unwrap();
        assert!(!reader.read(&mut out).unwrap());

        // A file cut short is another layout: refused, not read.
        std::fs::write(dir.join(SCORING_FILE), [0u8; 100]).unwrap();
        assert!(matches!(
            reader.read(&mut out),
            Err(SourceError::Invalid(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_diagnostic_copy_keeps_no_name() {
        let mut raw = Rf2Raw::new();
        let info = OFF_SCORING_INFO + SCORING_INFO_PLAYER_NAME.start;
        raw.scoring[info..info + 5].copy_from_slice(b"Alice");
        let v3 =
            OFF_SCORING_VEHICLES + 3 * SIZE_VEHICLE_SCORING_INFO + VEH_SCORING_DRIVER_NAME.start;
        raw.scoring[v3..v3 + 3].copy_from_slice(b"Bob");
        let car = OFF_SCORING_VEHICLES + 3 * SIZE_VEHICLE_SCORING_INFO + 36;
        raw.scoring[car..car + 4].copy_from_slice(b"Car7");
        let clean = raw.without_names();
        assert!(clean.scoring[info..info + 32].iter().all(|b| *b == 0));
        assert!(clean.scoring[v3..v3 + 32].iter().all(|b| *b == 0));
        assert_eq!(&clean.scoring[car..car + 4], b"Car7");
    }
}
