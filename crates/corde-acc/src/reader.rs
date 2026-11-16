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

use crate::layout::{Graphics, Physics, Static};
use corde_core::SourceError;
use std::path::{Path, PathBuf};

/// Names of the three mappings (without the `Local\` prefix), also the file names of a
/// directory mirror.
pub const PHYSICS_NAME: &str = "acpmf_physics";
pub const GRAPHICS_NAME: &str = "acpmf_graphics";
pub const STATIC_NAME: &str = "acpmf_static";

/// The three pages, as copied at one instant.
#[derive(Clone, Copy)]
pub struct Pages {
    pub physics: Physics,
    pub graphics: Graphics,
    pub statics: Static,
}

pub trait PageReader: Send {
    /// Copies the three pages. [`SourceError::NotAvailable`] when the game is not there
    /// (any more).
    fn read(&mut self) -> Result<Pages, SourceError>;
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
}

impl DirPages {
    pub fn open(dir: &Path) -> Result<Self, SourceError> {
        if !dir.join(GRAPHICS_NAME).is_file() {
            return Err(SourceError::NotAvailable(format!(
                "{} : pas de page {GRAPHICS_NAME}",
                dir.display()
            )));
        }
        Ok(Self {
            dir: dir.to_path_buf(),
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
    fn read(&mut self) -> Result<Pages, SourceError> {
        Ok(Pages {
            physics: page(&self.file(PHYSICS_NAME)?, PHYSICS_NAME)?,
            graphics: page(&self.file(GRAPHICS_NAME)?, GRAPHICS_NAME)?,
            statics: page(&self.file(STATIC_NAME)?, STATIC_NAME)?,
        })
    }

    fn describe(&self) -> String {
        format!("fichiers de {}", self.dir.display())
    }
}

/// The reader of this platform: the game's mappings on Windows; elsewhere, or when `dir`
/// is given, a directory mirror.
pub fn open_default_reader(dir: Option<&Path>) -> Result<Box<dyn PageReader>, SourceError> {
    if let Some(dir) = dir {
        return Ok(Box::new(DirPages::open(dir)?));
    }
    #[cfg(windows)]
    {
        Ok(Box::new(WindowsPages::open()?))
    }
    #[cfg(not(windows))]
    {
        Err(SourceError::NotAvailable(
            "Assetto Corsa Competizione ne tourne que sous Windows".into(),
        ))
    }
}

#[cfg(windows)]
pub use windows_impl::WindowsPages;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use crate::layout::{GRAPHICS_SIZE, PHYSICS_SIZE, STATIC_SIZE};
    use windows_sys::core::{w, PCWSTR};
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, FALSE, HANDLE};
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery, FILE_MAP_READ,
        MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
    };

    /// How many times a copy is retaken when the page was rewritten meanwhile.
    const COPY_TRIES: usize = 4;

    /// One mapping of the game, read-only.
    struct Mapping {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        size: usize,
    }

    // SAFETY: the view is only read, and only through `&mut self` of the owning reader.
    unsafe impl Send for Mapping {}

    impl Mapping {
        fn open(wide: PCWSTR, name: &str, size: usize) -> Result<Self, SourceError> {
            // SAFETY: plain Win32 call with a valid NUL-terminated wide name.
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, wide) };
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
            Ok(Self { handle, view, size })
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
    }

    impl WindowsPages {
        pub fn open() -> Result<Self, SourceError> {
            Ok(Self {
                // Graphics first: it is the one that says whether a session is live.
                graphics: Mapping::open(w!("Local\\acpmf_graphics"), GRAPHICS_NAME, GRAPHICS_SIZE)?,
                physics: Mapping::open(w!("Local\\acpmf_physics"), PHYSICS_NAME, PHYSICS_SIZE)?,
                statics: Mapping::open(w!("Local\\acpmf_static"), STATIC_NAME, STATIC_SIZE)?,
            })
        }
    }

    impl PageReader for WindowsPages {
        fn read(&mut self) -> Result<Pages, SourceError> {
            Ok(Pages {
                physics: page(&self.physics.copy(), PHYSICS_NAME)?,
                graphics: page(&self.graphics.copy(), GRAPHICS_NAME)?,
                statics: page(&self.statics.copy(), STATIC_NAME)?,
            })
        }

        fn describe(&self) -> String {
            "mémoire partagée du jeu (Local\\acpmf_*)".into()
        }
    }
}
