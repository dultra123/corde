//! Raw readers of the `LMU_Data` block.
//!
//! Two implementations of the internal [`RawReader`] trait:
//!
//! * [`LinuxShmReader`] (unix): `mmap` in read-only mode of `/dev/shm/LMU_Data`
//!   (or any path), filled by a bridge running inside the Proton prefix
//!   (winecarte or equivalent). No Win32 events or lock are available there,
//!   so a snapshot is validated by a **double witness read**: the session time
//!   `scoring.scoringInfo.mCurrentET`, the player's
//!   `telemetry.telemInfo[player].mElapsedTime` and the last active vehicle's
//!   `mElapsedTime` must be identical before and after the copy, otherwise the
//!   copy is retried (3 attempts).
//! * [`WindowsShmReader`] (windows): `OpenFileMappingW("LMU_Data")` +
//!   `MapViewOfFile` in read-only mode, wait on `LMU_Data_HoldEvent` **then**
//!   `LMU_Data_DataEvent` (the only race-free order per the official header),
//!   copy under the cooperative lock `LMU_SharedMemoryLockData` /
//!   `LMU_SharedMemoryLockEvent`. When the events or the lock are missing
//!   (older build, bridge…), it degrades to the same double witness read.
//!
//! The readers never interpret the data; see `convert.rs` for that.

use crate::layout::{telem_elapsed_time_offset, MAX_VEHICLES};
use crate::layout::{
    SharedMemoryObjectOut, OFF_ACTIVE_VEHICLES, OFF_CURRENT_ET, OFF_PLAYER_HAS_VEHICLE,
    OFF_PLAYER_VEHICLE_IDX, SHM_TOTAL_SIZE, SHM_USEFUL_SIZE,
};
use corde_core::SourceError;
use std::ptr;

/// Result of one [`RawReader::read`] call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadOutcome {
    /// A consistent snapshot was copied into the destination.
    Fresh,
    /// Nothing new was signalled by the game (Windows events timed out).
    NoNewData,
    /// Every attempt was torn by a concurrent write; the destination holds
    /// the last (inconsistent) attempt and must not be used.
    Torn,
}

/// Maximum number of copy attempts when validating with the witness values.
pub const MAX_WITNESS_ATTEMPTS: u32 = 3;

/// Internal contract of a raw block reader.
pub(crate) trait RawReader: Send {
    /// Copy the newest snapshot into `dst`.
    fn read(&mut self, dst: &mut SharedMemoryObjectOut) -> Result<ReadOutcome, SourceError>;

    /// Check that the mapping still exists (re-opening it when needed).
    /// Returns [`SourceError::NotAvailable`] when the game / bridge is gone.
    fn probe(&mut self) -> Result<(), SourceError>;

    /// Human readable description for logs / status.
    fn describe(&self) -> String;

    /// Stable identifier used in [`corde_protocol::TelemetryFrame::source`].
    fn source_id(&self) -> &'static str;
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// "Said once" flag for a log line that may come back every few seconds.
///
/// In the menus the game signals no update: after a few timeouts the reader
/// polls without the events, the next stall probe re-arms them, and they give
/// up again — every ≈ 5 s. The line is worth saying once per connection (a
/// reader lives for one connection), then only at `debug` level.
#[derive(Debug, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) struct OnceNotice {
    said: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
impl OnceNotice {
    /// `true` the first time only.
    pub(crate) fn first(&mut self) -> bool {
        !std::mem::replace(&mut self.said, true)
    }
}

/// Read a little-endian `f64` byte by byte with volatile loads.
///
/// The block is written by another process while we read it, and the witness
/// offsets are not 8-aligned (`mCurrentET` is at 1700), so neither
/// `read_volatile::<f64>` (alignment) nor `read_unaligned` (may be elided or
/// merged by the optimiser) is suitable.
///
/// # Safety
/// `p` must point to at least 8 readable bytes.
#[inline]
unsafe fn volatile_f64(p: *const u8) -> f64 {
    let mut b = [0u8; 8];
    for (i, slot) in b.iter_mut().enumerate() {
        // SAFETY: caller guarantees 8 readable bytes from `p`.
        *slot = unsafe { ptr::read_volatile(p.add(i)) };
    }
    f64::from_le_bytes(b)
}

/// Witness values read from a mapped block: `mCurrentET`, the player's
/// `mElapsedTime` and the `mElapsedTime` of the **last active vehicle**.
///
/// A writer fills the block sequentially, so a witness only protects the
/// bytes located *before* it: the last vehicle's clock covers the whole
/// telemetry array, the player's clock covers everything up to its own entry.
///
/// # Safety
/// `base` must point to at least `usable` (≥ [`SHM_USEFUL_SIZE`]) readable bytes.
unsafe fn read_witness(base: *const u8) -> (f64, f64, f64) {
    // SAFETY: all offsets below are < SHM_USEFUL_SIZE (checked by the layout tests).
    unsafe {
        let has_vehicle = ptr::read_volatile(base.add(OFF_PLAYER_HAS_VEHICLE)) != 0;
        let idx = ptr::read_volatile(base.add(OFF_PLAYER_VEHICLE_IDX)) as usize;
        let idx = if has_vehicle && idx < MAX_VEHICLES {
            idx
        } else {
            0
        };
        let active = ptr::read_volatile(base.add(OFF_ACTIVE_VEHICLES)) as usize;
        let last = active.clamp(1, MAX_VEHICLES) - 1;
        let et = volatile_f64(base.add(OFF_CURRENT_ET));
        let el = volatile_f64(base.add(telem_elapsed_time_offset(idx)));
        let el_last = volatile_f64(base.add(telem_elapsed_time_offset(last)));
        (et, el, el_last)
    }
}

/// Copy `usable` bytes from `base` into `dst`, validating the copy with the
/// witness values before/after. Retries up to [`MAX_WITNESS_ATTEMPTS`] times.
///
/// # Safety
/// `base` must point to at least `usable` readable bytes, with
/// `SHM_USEFUL_SIZE <= usable <= SHM_TOTAL_SIZE`.
pub(crate) unsafe fn copy_with_witness(
    base: *const u8,
    usable: usize,
    dst: &mut SharedMemoryObjectOut,
    torn_counter: &mut u64,
) -> ReadOutcome {
    debug_assert!((SHM_USEFUL_SIZE..=SHM_TOTAL_SIZE).contains(&usable));
    for _ in 0..MAX_WITNESS_ATTEMPTS {
        // SAFETY: `base` is valid for `usable` bytes (caller), `dst` is a
        // plain-old-data block of SHM_TOTAL_SIZE ≥ usable bytes, and the two
        // regions cannot overlap (dst is our own heap allocation).
        unsafe {
            let before = read_witness(base);
            let out = dst.as_bytes_mut();
            ptr::copy_nonoverlapping(base, out.as_mut_ptr(), usable);
            for b in &mut out[usable..] {
                *b = 0;
            }
            let after = read_witness(base);
            if before == after {
                return ReadOutcome::Fresh;
            }
        }
        *torn_counter += 1;
    }
    ReadOutcome::Torn
}

// ---------------------------------------------------------------------------
// Linux / unix: /dev/shm mirror
// ---------------------------------------------------------------------------

#[cfg(unix)]
pub use unix_impl::LinuxShmReader;

#[cfg(unix)]
mod unix_impl {
    use super::*;
    use memmap2::{Mmap, MmapOptions};
    use std::fs::File;
    use std::os::unix::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    /// Default location of the mirrored block on Linux.
    pub const DEFAULT_SHM_PATH: &str = "/dev/shm/LMU_Data";

    /// Read-only `mmap` of the mirrored block.
    pub struct LinuxShmReader {
        path: PathBuf,
        mmap: Mmap,
        /// bytes we copy: `min(file_len, SHM_TOTAL_SIZE)`
        usable: usize,
        inode: u64,
        pub torn_reads: u64,
    }

    impl LinuxShmReader {
        /// Open `path` (default [`DEFAULT_SHM_PATH`]). The file must be at
        /// least [`SHM_USEFUL_SIZE`] bytes long.
        pub fn open(path: impl AsRef<Path>) -> Result<Self, SourceError> {
            let path = path.as_ref().to_path_buf();
            let (mmap, usable, inode) = Self::map(&path)?;
            Ok(Self {
                path,
                mmap,
                usable,
                inode,
                torn_reads: 0,
            })
        }

        fn map(path: &Path) -> Result<(Mmap, usize, u64), SourceError> {
            let file = File::open(path).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    SourceError::NotAvailable(format!("{} does not exist", path.display()))
                }
                _ => SourceError::Io(e),
            })?;
            let meta = file.metadata()?;
            let len = meta.len() as usize;
            if len < SHM_USEFUL_SIZE {
                return Err(SourceError::Invalid(format!(
                    "{} is {} bytes, expected at least {}",
                    path.display(),
                    len,
                    SHM_USEFUL_SIZE
                )));
            }
            let usable = len.min(SHM_TOTAL_SIZE);
            // SAFETY: read-only mapping of a file that another process keeps
            // writing. We never create a `&[u8]` over the mapped region: every
            // access goes through raw volatile reads / `copy_nonoverlapping`
            // in `copy_with_witness`, which tolerates concurrent modification
            // (a torn copy is detected by the witness values and retried).
            // Truncation of the file by the writer while mapped would raise
            // SIGBUS; bridges keep the file at a fixed size.
            let mmap = unsafe { MmapOptions::new().len(usable).map(&file)? };
            Ok((mmap, usable, meta.ino()))
        }

        pub fn path(&self) -> &Path {
            &self.path
        }
    }

    impl RawReader for LinuxShmReader {
        fn read(&mut self, dst: &mut SharedMemoryObjectOut) -> Result<ReadOutcome, SourceError> {
            // SAFETY: `mmap` is valid for `usable` bytes for as long as it lives,
            // and `usable` was bounded at open().
            let outcome = unsafe {
                copy_with_witness(self.mmap.as_ptr(), self.usable, dst, &mut self.torn_reads)
            };
            Ok(outcome)
        }

        fn probe(&mut self) -> Result<(), SourceError> {
            match std::fs::metadata(&self.path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(
                    SourceError::NotAvailable(format!("{} disappeared", self.path.display())),
                ),
                Err(e) => Err(SourceError::Io(e)),
                Ok(meta) => {
                    if (meta.len() as usize) < SHM_USEFUL_SIZE {
                        return Err(SourceError::NotAvailable(format!(
                            "{} shrank to {} bytes",
                            self.path.display(),
                            meta.len()
                        )));
                    }
                    if meta.ino() != self.inode {
                        // The bridge re-created the file: re-map it.
                        let (mmap, usable, inode) = Self::map(&self.path)?;
                        self.mmap = mmap;
                        self.usable = usable;
                        self.inode = inode;
                    }
                    Ok(())
                }
            }
        }

        fn describe(&self) -> String {
            format!("{} ({} bytes mapped)", self.path.display(), self.usable)
        }

        fn source_id(&self) -> &'static str {
            "lmu-shm-linux"
        }
    }
}

// ---------------------------------------------------------------------------
// Windows: native file mapping, events and cooperative lock
// ---------------------------------------------------------------------------

#[cfg(windows)]
pub use windows_impl::WindowsShmReader;

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use windows_sys::core::w;
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, FALSE, HANDLE, INVALID_HANDLE_VALUE,
        WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, VirtualQuery,
        FILE_MAP_ALL_ACCESS, FILE_MAP_READ, MEMORY_BASIC_INFORMATION, MEMORY_MAPPED_VIEW_ADDRESS,
        PAGE_READWRITE,
    };
    use windows_sys::Win32::System::Threading::{
        CreateEventW, OpenEventW, SetEvent, WaitForSingleObject, SYNCHRONIZATION_SYNCHRONIZE,
    };

    /// Timeout of each wait on the Hold / Data events.
    pub const EVENT_WAIT_TIMEOUT_MS: u32 = 200;
    /// Spin count before sleeping on the lock event (same as the header).
    const LOCK_MAX_SPINS: u32 = 4000;
    /// Never block on the lock longer than this: a reader that cannot get the
    /// lock falls back to the witness copy instead of stalling the pipeline.
    const LOCK_WAIT_TIMEOUT_MS: u32 = 20;
    /// After this many consecutive event timeouts (× 2 × 200 ms) we stop
    /// waiting on the events and poll the section instead.
    const EVENT_TIMEOUTS_BEFORE_POLLING: u32 = 10;
    /// `sizeof(LockData)` = 2 × `LONG`.
    const LOCK_DATA_SIZE: u32 = 8;

    /// Owned view of the `LMU_Data` mapping.
    struct Mapping {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        /// bytes we copy: `min(region_size, SHM_TOTAL_SIZE)`
        usable: usize,
    }

    impl Mapping {
        fn open() -> Result<Self, SourceError> {
            // SAFETY: plain Win32 calls with a valid NUL-terminated wide name.
            let handle = unsafe { OpenFileMappingW(FILE_MAP_READ, FALSE, w!("LMU_Data")) };
            if handle.is_null() {
                let err = unsafe { GetLastError() };
                let hint = match err {
                    2 => "LMU n'est pas lancé, ou « Enable Plugins » est désactivé (Settings > Gameplay, puis redémarrer le jeu)",
                    5 => "accès refusé : le jeu tourne probablement en administrateur ; lance Corde en administrateur aussi",
                    _ => "le jeu est-il lancé avec « Enable Plugins » activé ?",
                };
                return Err(SourceError::NotAvailable(format!(
                    "OpenFileMappingW(\"LMU_Data\") failed (error {err}): {hint}"
                )));
            }
            // SAFETY: `handle` is a valid mapping handle; size 0 maps the whole section.
            let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, 0) };
            if view.Value.is_null() {
                let err = unsafe { GetLastError() };
                // SAFETY: handle is valid and owned by us.
                unsafe { CloseHandle(handle) };
                return Err(SourceError::Other(format!(
                    "MapViewOfFile(\"LMU_Data\") failed (error {err})"
                )));
            }
            // Find out how large the view really is before trusting any offset.
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
            if region < SHM_USEFUL_SIZE {
                // SAFETY: both were obtained above and are owned by us.
                unsafe {
                    UnmapViewOfFile(view);
                    CloseHandle(handle);
                }
                return Err(SourceError::Invalid(format!(
                    "LMU_Data view is {region} bytes, expected at least {SHM_USEFUL_SIZE} (game build too old?)"
                )));
            }
            Ok(Self {
                handle,
                view,
                usable: region.min(SHM_TOTAL_SIZE),
            })
        }

        fn base(&self) -> *const u8 {
            self.view.Value as *const u8
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

    /// The two update events (`LMU_Data_HoldEvent`, `LMU_Data_DataEvent`).
    struct UpdateEvents {
        hold: HANDLE,
        data: HANDLE,
    }

    impl UpdateEvents {
        fn open() -> Option<Self> {
            // SAFETY: plain Win32 calls with valid NUL-terminated wide names.
            let hold =
                unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, FALSE, w!("LMU_Data_HoldEvent")) };
            if hold.is_null() {
                return None;
            }
            let data =
                unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, FALSE, w!("LMU_Data_DataEvent")) };
            if data.is_null() {
                // SAFETY: `hold` is valid and owned by us.
                unsafe { CloseHandle(hold) };
                return None;
            }
            Some(Self { hold, data })
        }

        /// Wait for the next frame: Hold **then** Data (header order).
        /// `Ok(true)` = signalled, `Ok(false)` = timeout, `Err` = wait failed.
        fn wait_next_frame(&self, timeout_ms: u32) -> Result<bool, u32> {
            for h in [self.hold, self.data] {
                // SAFETY: `h` is a valid event handle owned by us.
                let r = unsafe { WaitForSingleObject(h, timeout_ms) };
                if r == WAIT_TIMEOUT {
                    return Ok(false);
                }
                if r != WAIT_OBJECT_0 {
                    return Err(r);
                }
            }
            Ok(true)
        }
    }

    impl Drop for UpdateEvents {
        fn drop(&mut self) {
            // SAFETY: both handles are valid and owned by us.
            unsafe {
                CloseHandle(self.hold);
                CloseHandle(self.data);
            }
        }
    }

    /// The cooperative lock shared with the game:
    /// mapping `LMU_SharedMemoryLockData` = `{ volatile LONG waiters; volatile LONG busy; }`
    /// + auto-reset event `LMU_SharedMemoryLockEvent`.
    struct CoopLock {
        map: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        event: HANDLE,
    }

    impl CoopLock {
        fn open() -> Option<Self> {
            // Same calls as the header: create-or-open a pagefile-backed mapping.
            // SAFETY: plain Win32 calls; INVALID_HANDLE_VALUE requests a pagefile section.
            let map = unsafe {
                CreateFileMappingW(
                    INVALID_HANDLE_VALUE,
                    ptr::null(),
                    PAGE_READWRITE,
                    0,
                    LOCK_DATA_SIZE,
                    w!("LMU_SharedMemoryLockData"),
                )
            };
            if map.is_null() {
                return None;
            }
            // Must be read right after CreateFileMappingW (MapViewOfFile may reset it).
            // SAFETY: trivial call.
            let already_exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
            // SAFETY: `map` is valid; we map exactly the two LONGs.
            let view =
                unsafe { MapViewOfFile(map, FILE_MAP_ALL_ACCESS, 0, 0, LOCK_DATA_SIZE as usize) };
            if view.Value.is_null() {
                // SAFETY: `map` is valid and owned by us.
                unsafe { CloseHandle(map) };
                return None;
            }
            // SAFETY: NULL security attributes, auto-reset, initially non-signalled.
            let event =
                unsafe { CreateEventW(ptr::null(), FALSE, FALSE, w!("LMU_SharedMemoryLockEvent")) };
            if event.is_null() {
                // SAFETY: both are valid and owned by us.
                unsafe {
                    UnmapViewOfFile(view);
                    CloseHandle(map);
                }
                return None;
            }
            let lock = Self { map, view, event };
            if !already_exists {
                // We created the section: the header resets it in that case
                // (pagefile sections are zero-filled anyway).
                lock.waiters().store(0, Ordering::SeqCst);
                lock.busy().store(0, Ordering::SeqCst);
            }
            Some(lock)
        }

        fn waiters(&self) -> &AtomicI32 {
            // SAFETY: the view is ≥ 8 bytes, page-aligned, and lives as long as
            // `self`; `LONG` has the same size/alignment as `AtomicI32`.
            unsafe { &*(self.view.Value as *const AtomicI32) }
        }

        fn busy(&self) -> &AtomicI32 {
            // SAFETY: see `waiters()`; `busy` is the second LONG.
            unsafe { &*((self.view.Value as *const AtomicI32).add(1)) }
        }

        fn try_acquire(&self) -> bool {
            self.busy()
                .compare_exchange(0, 1, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
        }

        /// `SharedMemoryLock::Lock()` with a bounded wait.
        fn lock(&self, timeout_ms: u32) -> bool {
            for _ in 0..LOCK_MAX_SPINS {
                if self.try_acquire() {
                    return true;
                }
                std::hint::spin_loop();
            }
            self.waiters().fetch_add(1, Ordering::SeqCst);
            let mut acquired = false;
            loop {
                if self.try_acquire() {
                    acquired = true;
                    break;
                }
                // SAFETY: `event` is a valid handle owned by us.
                if unsafe { WaitForSingleObject(self.event, timeout_ms) } != WAIT_OBJECT_0 {
                    break; // timeout or error: give up
                }
            }
            self.waiters().fetch_sub(1, Ordering::SeqCst);
            acquired
        }

        /// `SharedMemoryLock::Unlock()`.
        fn unlock(&self) {
            self.busy().store(0, Ordering::SeqCst);
            if self.waiters().load(Ordering::SeqCst) > 0 {
                // SAFETY: `event` is a valid handle owned by us.
                unsafe { SetEvent(self.event) };
            }
        }
    }

    impl Drop for CoopLock {
        fn drop(&mut self) {
            // SAFETY: all three were obtained in `open()` and are owned by us.
            unsafe {
                CloseHandle(self.event);
                UnmapViewOfFile(self.view);
                CloseHandle(self.map);
            }
        }
    }

    /// Native Windows reader.
    pub struct WindowsShmReader {
        /// `None` only transiently, while `probe()` re-opens the section.
        mapping: Option<Mapping>,
        events: Option<UpdateEvents>,
        lock: Option<CoopLock>,
        pub wait_timeout_ms: u32,
        pub torn_reads: u64,
        pub lock_failures: u64,
        /// Consecutive event waits that timed out while events were enabled.
        event_timeouts: u32,
        /// "polling without events" is logged once per connection.
        polling_notice: super::OnceNotice,
    }

    // SAFETY: Win32 handles and section views are process-wide objects, not
    // bound to the creating thread; the reader is only ever used from one
    // thread at a time (`&mut self`).
    unsafe impl Send for WindowsShmReader {}

    impl WindowsShmReader {
        /// Open the mapping; events and lock are optional extras.
        pub fn open() -> Result<Self, SourceError> {
            let mapping = Mapping::open()?;
            let events = UpdateEvents::open();
            let lock = CoopLock::open();
            if events.is_none() {
                tracing::warn!("LMU_Data_HoldEvent/DataEvent not found: polling without events");
            }
            if lock.is_none() {
                tracing::warn!("LMU_SharedMemoryLockData not available: witness copy only");
            }
            Ok(Self {
                mapping: Some(mapping),
                events,
                lock,
                wait_timeout_ms: EVENT_WAIT_TIMEOUT_MS,
                torn_reads: 0,
                lock_failures: 0,
                event_timeouts: 0,
                polling_notice: super::OnceNotice::default(),
            })
        }

        pub fn has_events(&self) -> bool {
            self.events.is_some()
        }

        pub fn has_lock(&self) -> bool {
            self.lock.is_some()
        }
    }

    impl RawReader for WindowsShmReader {
        fn read(&mut self, dst: &mut SharedMemoryObjectOut) -> Result<ReadOutcome, SourceError> {
            let timeout = self.wait_timeout_ms;
            match self.events.as_ref().map(|ev| ev.wait_next_frame(timeout)) {
                None => {}
                Some(Ok(true)) => self.event_timeouts = 0,
                Some(Ok(false)) => {
                    // The events are auto-reset: another telemetry tool (SimHub,
                    // Crew Chief, TinyPedal…) may consume the signal before us, and
                    // a paused game signals nothing at all. Either way we still
                    // copy (the witness check + the caller's de-duplication make
                    // this safe), and after a few consecutive timeouts we give up
                    // on the events for this connection and simply poll.
                    self.event_timeouts += 1;
                    if self.event_timeouts >= EVENT_TIMEOUTS_BEFORE_POLLING {
                        // Back every ≈ 5 s in the menus (the stall probe re-arms
                        // the events): said once per connection, then debug.
                        if self.polling_notice.first() {
                            tracing::info!(
                                "LMU_Data events not delivered for {} waits; polling without events",
                                self.event_timeouts
                            );
                        } else {
                            tracing::debug!(
                                "LMU_Data events not delivered for {} waits; polling without events",
                                self.event_timeouts
                            );
                        }
                        self.events = None;
                    }
                }
                Some(Err(code)) => {
                    tracing::warn!(code, "wait on LMU_Data events failed; switching to polling");
                    self.events = None;
                }
            }

            let Some(mapping) = self.mapping.as_ref() else {
                return Err(SourceError::NotAvailable("LMU_Data mapping closed".into()));
            };
            let base = mapping.base();
            let usable = mapping.usable;

            if let Some(lock) = &self.lock {
                if lock.lock(LOCK_WAIT_TIMEOUT_MS) {
                    // SAFETY: the view is valid for `usable` bytes (checked with
                    // VirtualQuery) and `dst` is our own allocation of
                    // SHM_TOTAL_SIZE ≥ usable bytes. The game should not write
                    // while we hold the cooperative lock, but we still validate
                    // the copy with the witnesses: if the lock is honoured this
                    // costs nothing, and if it is not (older build, writer using
                    // TryLockSpin) a torn copy is detected instead of trusted.
                    let outcome =
                        unsafe { copy_with_witness(base, usable, dst, &mut self.torn_reads) };
                    lock.unlock();
                    return Ok(outcome);
                }
                self.lock_failures += 1;
            }

            // SAFETY: the view is valid for `usable` bytes (checked with VirtualQuery).
            let outcome = unsafe { copy_with_witness(base, usable, dst, &mut self.torn_reads) };
            Ok(outcome)
        }

        fn probe(&mut self) -> Result<(), SourceError> {
            // Our own handle keeps the named section alive after the game exits,
            // so the only reliable check is to release it first and open it again.
            self.mapping = None;
            match Mapping::open() {
                Ok(m) => {
                    self.mapping = Some(m);
                    if self.events.is_none() {
                        self.events = UpdateEvents::open();
                        self.event_timeouts = 0;
                    }
                    if self.lock.is_none() {
                        self.lock = CoopLock::open();
                    }
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }

        fn describe(&self) -> String {
            format!(
                "LMU_Data ({} bytes, events: {}, lock: {})",
                self.mapping.as_ref().map(|m| m.usable).unwrap_or(0),
                if self.events.is_some() { "yes" } else { "no" },
                if self.lock.is_some() { "yes" } else { "no" }
            )
        }

        fn source_id(&self) -> &'static str {
            "lmu-shm"
        }
    }
}

/// Open the platform's default reader.
///
/// * Windows: the native `LMU_Data` mapping.
/// * Unix: `path` (default `/dev/shm/LMU_Data`).
pub(crate) fn open_default_reader(
    #[allow(unused_variables)] path: Option<&std::path::Path>,
) -> Result<Box<dyn RawReader>, SourceError> {
    #[cfg(windows)]
    {
        Ok(Box::new(WindowsShmReader::open()?))
    }
    #[cfg(unix)]
    {
        let p = path
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| std::path::PathBuf::from(unix_impl::DEFAULT_SHM_PATH));
        Ok(Box::new(LinuxShmReader::open(p)?))
    }
    #[cfg(not(any(windows, unix)))]
    {
        Err(SourceError::NotAvailable(
            "no shared memory reader for this platform".into(),
        ))
    }
}

#[cfg(unix)]
pub use unix_impl::DEFAULT_SHM_PATH;

#[cfg(all(test, unix))]
mod tests {
    #[test]
    fn once_notice_says_it_once() {
        let mut n = super::OnceNotice::default();
        assert!(n.first());
        assert!(!n.first());
        assert!(!n.first());
        // A new connection (a new reader) says it again.
        assert!(super::OnceNotice::default().first());
    }

    use super::*;
    use crate::layout::SharedMemoryObjectOut;
    use std::io::Write;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        dir.join(format!("corde-reader-{}-{}", std::process::id(), name))
    }

    #[test]
    fn rejects_small_file_and_missing_file() {
        let p = temp_path("small");
        std::fs::write(&p, [0u8; 1024]).unwrap();
        let err = LinuxShmReader::open(&p)
            .err()
            .expect("small file must be rejected");
        assert!(matches!(err, SourceError::Invalid(_)), "{err}");
        std::fs::remove_file(&p).unwrap();
        let err = LinuxShmReader::open(&p).err().expect("missing file");
        assert!(matches!(err, SourceError::NotAvailable(_)), "{err}");
    }

    #[test]
    fn reads_block_and_detects_disappearance() {
        let p = temp_path("block");
        let mut block = SharedMemoryObjectOut::boxed();
        block.scoring.scoring_info.current_et = 123.5;
        block.telemetry.player_has_vehicle = 1;
        block.telemetry.player_vehicle_idx = 1;
        block.telemetry.telem_info[1].elapsed_time = 99.25;
        {
            let mut f = std::fs::File::create(&p).unwrap();
            // write only the useful part: the reader must accept ≥ 324 820 bytes
            f.write_all(&block.as_bytes()[..SHM_USEFUL_SIZE]).unwrap();
        }
        let mut r = LinuxShmReader::open(&p).unwrap();
        let mut dst = SharedMemoryObjectOut::boxed();
        assert_eq!(r.read(&mut dst).unwrap(), ReadOutcome::Fresh);
        // copy the packed fields out before comparing (unaligned references are UB)
        let (et, el) = (
            dst.scoring.scoring_info.current_et,
            dst.telemetry.telem_info[1].elapsed_time,
        );
        assert_eq!(et, 123.5);
        assert_eq!(el, 99.25);
        assert_eq!(r.torn_reads, 0);
        assert!(r.probe().is_ok());
        assert_eq!(r.source_id(), "lmu-shm-linux");
        std::fs::remove_file(&p).unwrap();
        assert!(matches!(r.probe(), Err(SourceError::NotAvailable(_))));
    }
}
