//! Copies of the three pages of the game.
//!
//! * Windows: the named mappings of the game, opened **read-only** ([`WindowsPages`]).
//! * Everywhere: three files of a directory ([`DirPages`]) — what the tests and the
//!   `acc-probe --dir` tool use, written by [`crate::fake`]. The game itself only runs on
//!   Windows; nothing mirrors it on Linux.
//!
//! The game takes no lock while it rewrites a page. A copy is therefore checked against
//! the step counter of the page (its first field): if the counter moved during the copy,
//! the copy is taken again.

use crate::layout::{Game, Graphics, GraphicsAc, Physics, Static};
use corde_core::SourceError;
use std::path::{Path, PathBuf};

/// Names of the three mappings (without the `Local\` prefix), also the file names of a
/// directory mirror.
pub const PHYSICS_NAME: &str = "acpmf_physics";
pub const GRAPHICS_NAME: &str = "acpmf_graphics";
pub const STATIC_NAME: &str = "acpmf_static";

/// The three pages of one family of games: their names, and the least each must hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageSet {
    pub physics: &'static str,
    pub graphics: &'static str,
    pub statics: &'static str,
    /// Bytes of the physics, graphics and static structures.
    pub sizes: [usize; 3],
}

/// Assetto Corsa Competizione, the first Assetto Corsa.
pub const ACC_PAGES: PageSet = PageSet {
    physics: PHYSICS_NAME,
    graphics: GRAPHICS_NAME,
    statics: STATIC_NAME,
    sizes: [
        crate::layout::PHYSICS_SIZE,
        crate::layout::GRAPHICS_SIZE,
        crate::layout::STATIC_SIZE,
    ],
};

/// Assetto Corsa EVO. Its graphics page grew in April 2026 (the table of the cars'
/// identifiers, at its end): the page of an older build is accepted without it.
pub const EVO_PAGES: PageSet = PageSet {
    physics: "acevo_pmf_physics",
    graphics: "acevo_pmf_graphics",
    statics: "acevo_pmf_static",
    sizes: [
        crate::layout::PHYSICS_SIZE,
        crate::evo::layout::GRAPHICS_EVO_MIN_SIZE,
        crate::evo::layout::STATIC_EVO_SIZE,
    ],
};

/// The three pages of Assetto Corsa EVO, as copied at one instant.
#[derive(Clone, Copy)]
pub struct EvoPages {
    pub physics: Physics,
    pub graphics: crate::evo::layout::GraphicsEvo,
    pub statics: crate::evo::layout::StaticEvo,
}

/// The three pages, as copied at one instant.
#[derive(Clone, Copy)]
pub struct Pages {
    pub physics: Physics,
    pub graphics: Graphics,
    pub statics: Static,
}

/// The three pages as the bytes the game maps.
#[derive(Debug, Clone)]
pub struct RawPages {
    pub physics: Vec<u8>,
    pub graphics: Vec<u8>,
    pub statics: Vec<u8>,
}

impl RawPages {
    /// The pages of Competizione, decoded. Fails when one is shorter than its structure.
    pub fn pages(&self) -> Result<Pages, SourceError> {
        self.pages_of(Game::Competizione)
    }

    /// The pages of `game`, decoded. Those of the first Assetto Corsa are read as mapped
    /// (padded with zeros to the sizes of Competizione's) and its graphics page is turned
    /// into ours ([`GraphicsAc::upgrade`]).
    pub fn pages_of(&self, game: Game) -> Result<Pages, SourceError> {
        Ok(Pages {
            physics: page(&self.physics, PHYSICS_NAME)?,
            graphics: match game {
                Game::Competizione => page(&self.graphics, GRAPHICS_NAME)?,
                Game::AssettoCorsa => page::<GraphicsAc>(&self.graphics, GRAPHICS_NAME)?.upgrade(),
            },
            statics: page(&self.statics, STATIC_NAME)?,
        })
    }

    /// Which game these pages come from (see [`Game::of`]).
    pub fn game(&self) -> Result<Option<Game>, SourceError> {
        Ok(Game::of(&page::<Static>(&self.statics, STATIC_NAME)?))
    }

    /// The same pages without the player's first name, surname and nickname (the static
    /// page holds them): what a diagnostic copy must be.
    pub fn without_names(mut self) -> Self {
        let from = std::mem::offset_of!(Static, player_name);
        let to = std::mem::offset_of!(Static, sector_count);
        if let Some(names) = self.statics.get_mut(from..to) {
            names.fill(0);
        }
        self
    }

    /// The pages of Assetto Corsa EVO, decoded. Fails when one is shorter than its
    /// structure.
    pub fn evo(&self) -> Result<EvoPages, SourceError> {
        use crate::evo::layout::{GRAPHICS_EVO_MIN_SIZE, GRAPHICS_EVO_SIZE};
        // A page that stops before the table of identifiers (an older build) is read
        // with that table at zero.
        let graphics = if (GRAPHICS_EVO_MIN_SIZE..GRAPHICS_EVO_SIZE).contains(&self.graphics.len())
        {
            let mut padded = self.graphics.clone();
            padded.resize(GRAPHICS_EVO_SIZE, 0);
            page(&padded, EVO_PAGES.graphics)?
        } else {
            page(&self.graphics, EVO_PAGES.graphics)?
        };
        Ok(EvoPages {
            physics: page(&self.physics, EVO_PAGES.physics)?,
            graphics,
            statics: page(&self.statics, EVO_PAGES.statics)?,
        })
    }

    /// Pages of Assetto Corsa EVO without what names the player: the first name and the
    /// surname are blanked, and the identifiers of the cars are replaced by small numbers
    /// (the same car keeps the same number, so the player's slot can still be found).
    /// What a diagnostic copy must be.
    pub fn evo_without_names(mut self) -> Self {
        use crate::evo::layout::{GraphicsEvo, MAX_CARS};
        use std::mem::offset_of;
        let from = offset_of!(GraphicsEvo, driver_name);
        let to = offset_of!(GraphicsEvo, car_model);
        if let Some(names) = self.graphics.get_mut(from..to) {
            names.fill(0);
        }
        // Each identifier is 16 bytes: the camera's, the player's, then one per slot.
        let mut places = vec![
            offset_of!(GraphicsEvo, focused_car_id),
            offset_of!(GraphicsEvo, player_car_id),
        ];
        places.extend((0..MAX_CARS).map(|i| offset_of!(GraphicsEvo, car_ids) + 16 * i));
        let mut seen: Vec<[u8; 16]> = Vec::new();
        for at in places {
            let Some(slot) = self.graphics.get_mut(at..at + 16) else {
                continue;
            };
            let mut id = [0u8; 16];
            id.copy_from_slice(slot);
            if id == [0u8; 16] {
                continue;
            }
            let number = match seen.iter().position(|s| *s == id) {
                Some(i) => i,
                None => {
                    seen.push(id);
                    seen.len() - 1
                }
            } as u64
                + 1;
            slot.fill(0);
            slot[..8].copy_from_slice(&number.to_le_bytes());
        }
        self
    }

    /// Written as the three files a [`DirPages`] reads (whole files, replaced one after
    /// the other).
    pub fn write_dir(&self, dir: &Path) -> std::io::Result<()> {
        self.write_dir_of(dir, &ACC_PAGES)
    }

    /// The same under the names of another family of pages.
    pub fn write_dir_of(&self, dir: &Path, set: &PageSet) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        for (name, bytes) in [
            (set.statics, &self.statics),
            (set.physics, &self.physics),
            (set.graphics, &self.graphics),
        ] {
            let tmp = dir.join(format!("{name}.tmp"));
            std::fs::write(&tmp, bytes)?;
            std::fs::rename(&tmp, dir.join(name))?;
        }
        Ok(())
    }
}

pub trait PageReader: Send {
    /// Copies the three pages as bytes. [`SourceError::NotAvailable`] when the game is not
    /// there (any more).
    fn read_raw(&mut self) -> Result<RawPages, SourceError>;
    /// Copies and decodes the three pages.
    fn read(&mut self) -> Result<Pages, SourceError> {
        self.read_raw()?.pages()
    }
    /// One line for the logs: where the pages come from.
    fn describe(&self) -> String;
}

fn page<T: bytemuck::AnyBitPattern>(bytes: &[u8], name: &str) -> Result<T, SourceError> {
    let size = std::mem::size_of::<T>();
    match bytes.get(..size) {
        Some(b) => Ok(bytemuck::pod_read_unaligned(b)),
        None => Err(SourceError::Invalid(format!(
            "{name} : {} octets, {size} attendus",
            bytes.len()
        ))),
    }
}

/// Three files of a directory, read whole at each call.
pub struct DirPages {
    dir: PathBuf,
    set: PageSet,
}

impl DirPages {
    pub fn open(dir: &Path) -> Result<Self, SourceError> {
        Self::open_of(dir, &ACC_PAGES)
    }

    pub fn open_of(dir: &Path, set: &PageSet) -> Result<Self, SourceError> {
        if !dir.join(set.graphics).is_file() {
            return Err(SourceError::NotAvailable(format!(
                "{} : pas de page {}",
                dir.display(),
                set.graphics
            )));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            set: *set,
        })
    }

    fn file(&self, name: &str) -> Result<Vec<u8>, SourceError> {
        std::fs::read(self.dir.join(name)).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                SourceError::NotAvailable(format!("{name} a disparu"))
            } else {
                SourceError::Io(e)
            }
        })
    }
}

impl PageReader for DirPages {
    fn read_raw(&mut self) -> Result<RawPages, SourceError> {
        Ok(RawPages {
            physics: self.file(self.set.physics)?,
            graphics: self.file(self.set.graphics)?,
            statics: self.file(self.set.statics)?,
        })
    }

    fn describe(&self) -> String {
        format!("fichiers de {}", self.dir.display())
    }
}

/// The reader of this platform: the game's mappings on Windows; elsewhere, or when `dir`
/// is given, a directory mirror.
pub fn open_default_reader(dir: Option<&Path>) -> Result<Box<dyn PageReader>, SourceError> {
    open_reader(&ACC_PAGES, dir)
}

/// The same for the pages of another family of games.
pub fn open_reader(set: &PageSet, dir: Option<&Path>) -> Result<Box<dyn PageReader>, SourceError> {
    if let Some(dir) = dir {
        return Ok(Box::new(DirPages::open_of(dir, set)?));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(WindowsPages::open_of(set)?))
    }
    #[cfg(not(windows))]
    {
        Err(SourceError::NotAvailable(format!(
            "{} : ce jeu ne tourne que sous Windows",
            set.graphics
        )))
    }
}

#[cfg(windows)]
pub use windows_impl::WindowsPages;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FALSE, HANDLE};
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
        MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    };

    /// How many times a copy is retaken when the page was rewritten meanwhile.
    const COPY_TRIES: usize = 4;
    /// The most that is copied of one mapping.
    const MAX_COPY: usize = 64 * 1024;

    /// One mapping of the game, read-only.
    struct Mapping {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        size: usize,
    }

    // SAFETY: the view is only read, and only through `&mut self` of the owning reader.
    unsafe impl Send for Mapping {}

    impl Mapping {
        fn open(name: &str, size: usize) -> Result<Self, SourceError> {
            let wide: Vec<u16> = format!("Local\\{name}")
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            // SAFETY: plain Win32 call with a valid NUL-terminated wide name that lives
            // until the call returns.
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, wide.as_ptr()) };
            if handle.is_null() {
                // SAFETY: no precondition.
                let err = unsafe { GetLastError() };
                let hint = match err {
                    2 => "le jeu n'est pas lancé",
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
                    "{name} : {region} octets, {size} attendus (autre jeu ou version trop ancienne ?)"
                )));
            }
            // Everything that is mapped is copied (a whole number of memory pages, zeros
            // after what the game writes): a diagnostic copy then also holds what a newer
            // build of the game would have added.
            Ok(Self {
                handle,
                view,
                size: region.min(MAX_COPY),
            })
        }

        /// The page, copied; taken again while its step counter (the first 4 bytes)
        /// moves during the copy.
        fn copy(&self) -> Vec<u8> {
            let base = self.view.Value as *const u8;
            let mut out = vec![0u8; self.size];
            for _ in 0..COPY_TRIES {
                // SAFETY: the view is mapped for at least `self.size` bytes (checked in
                // `open`) and stays mapped while `self` lives; reads of memory another
                // process writes are racy by nature, hence the counter check below.
                unsafe { std::ptr::copy_nonoverlapping(base, out.as_mut_ptr(), self.size) };
                let mut after = [0u8; 4];
                // SAFETY: as above, the first 4 bytes are mapped.
                unsafe { std::ptr::copy_nonoverlapping(base, after.as_mut_ptr(), 4) };
                if out[..4] == after {
                    break;
                }
            }
            out
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

    /// The three mappings of the running game.
    pub struct WindowsPages {
        physics: Mapping,
        graphics: Mapping,
        statics: Mapping,
        set: PageSet,
    }

    impl WindowsPages {
        pub fn open() -> Result<Self, SourceError> {
            Self::open_of(&ACC_PAGES)
        }

        pub fn open_of(set: &PageSet) -> Result<Self, SourceError> {
            let [physics, graphics, statics] = set.sizes;
            Ok(Self {
                // Graphics first: it is the one that says whether a session is live.
                graphics: Mapping::open(set.graphics, graphics)?,
                physics: Mapping::open(set.physics, physics)?,
                statics: Mapping::open(set.statics, statics)?,
                set: *set,
            })
        }
    }

    impl PageReader for WindowsPages {
        fn read_raw(&mut self) -> Result<RawPages, SourceError> {
            Ok(RawPages {
                physics: self.physics.copy(),
                graphics: self.graphics.copy(),
                statics: self.statics.copy(),
            })
        }

        fn describe(&self) -> String {
            format!("mémoire partagée du jeu (Local\\{})", self.set.graphics)
        }
    }
}
