//! `lmu-shm-fake` — writes a plausible `LMU_Data` block to `/dev/shm` at
//! 100 Hz so that the rest of Corde can be developed on Linux without the
//! game (or the bridge). Uses [`corde_lmu::fake::FakeWorld`].
//!
//! ```text
//! lmu-shm-fake [--path /dev/shm/LMU_Data] [--hz 100] [--duration-s 0] [--quiet]
//! ```
//!
//! The file is created at its exact size (324 824 bytes) and removed on exit
//! when it was created by this tool.

#[cfg(unix)]
fn main() -> std::process::ExitCode {
    unix::run()
}

#[cfg(not(unix))]
fn main() -> std::process::ExitCode {
    eprintln!("lmu-shm-fake ne fonctionne que sous Linux/unix (miroir /dev/shm) ; sous Windows, lancez le jeu.");
    std::process::ExitCode::from(1)
}

#[cfg(unix)]
mod unix {
    use corde_lmu::fake::FakeWorld;
    use corde_lmu::layout::{SharedMemoryObjectOut, SHM_TOTAL_SIZE};
    use corde_lmu::reader::DEFAULT_SHM_PATH;
    use memmap2::MmapOptions;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    const USAGE: &str = "\
lmu-shm-fake — écrit un bloc LMU_Data synthétique dans /dev/shm (Linux)

Usage : lmu-shm-fake [OPTIONS]
  --path CHEMIN     fichier à alimenter (défaut /dev/shm/LMU_Data)
  --hz N            cadence d'écriture (défaut 100)
  --duration-s N    durée en secondes (défaut 0 = jusqu'à Ctrl+C)
  --quiet           pas d'affichage périodique
  -h, --help        cette aide";

    struct Args {
        path: PathBuf,
        hz: f64,
        duration: Option<Duration>,
        quiet: bool,
    }

    fn parse_args() -> Result<Args, String> {
        let mut a = Args {
            path: PathBuf::from(DEFAULT_SHM_PATH),
            hz: 100.0,
            duration: None,
            quiet: false,
        };
        let mut it = std::env::args().skip(1);
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--path" => a.path = PathBuf::from(it.next().ok_or("--path attend un chemin")?),
                "--hz" => {
                    let v = it.next().ok_or("--hz attend un nombre")?;
                    a.hz = v
                        .parse::<f64>()
                        .ok()
                        .filter(|h| *h > 0.0 && *h <= 1000.0)
                        .ok_or(format!("cadence invalide : {v}"))?;
                }
                "--duration-s" => {
                    let v = it.next().ok_or("--duration-s attend un nombre")?;
                    let s: u64 = v.parse().map_err(|_| format!("durée invalide : {v}"))?;
                    a.duration = (s > 0).then(|| Duration::from_secs(s));
                }
                "--quiet" => a.quiet = true,
                "-h" | "--help" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
            }
        }
        Ok(a)
    }

    pub fn run() -> ExitCode {
        let args = match parse_args() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        };

        let existed = args.path.exists();
        let file = match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&args.path)
        {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Impossible d'ouvrir {} : {e}", args.path.display());
                return ExitCode::from(1);
            }
        };
        if let Err(e) = file.set_len(SHM_TOTAL_SIZE as u64) {
            eprintln!("Impossible de dimensionner {} : {e}", args.path.display());
            return ExitCode::from(1);
        }
        // SAFETY: we are the only writer of this file; readers map it
        // read-only and validate their copies with witness values.
        let mut map = match unsafe { MmapOptions::new().len(SHM_TOTAL_SIZE).map_mut(&file) } {
            Ok(m) => m,
            Err(e) => {
                eprintln!("mmap de {} impossible : {e}", args.path.display());
                return ExitCode::from(1);
            }
        };

        let stop = Arc::new(AtomicBool::new(false));
        install_ctrlc(stop.clone());

        let mut world = FakeWorld::new();
        let mut block = SharedMemoryObjectOut::boxed();
        let period = Duration::from_secs_f64(1.0 / args.hz);
        let start = Instant::now();
        let mut next = start;
        let mut ticks: u64 = 0;
        let mut last_report = start;
        println!(
            "lmu-shm-fake : {} ({} octets) à {:.0} Hz — Ctrl+C pour arrêter",
            args.path.display(),
            SHM_TOTAL_SIZE,
            args.hz
        );

        while !stop.load(Ordering::Relaxed) {
            if let Some(d) = args.duration {
                if start.elapsed() >= d {
                    break;
                }
            }
            world.step(period.as_secs_f64());
            world.write_into(&mut block);
            map.copy_from_slice(block.as_bytes());
            ticks += 1;

            if !args.quiet && last_report.elapsed() >= Duration::from_secs(5) {
                let p = &world.cars[0];
                println!(
                    "t={:.1} s · {} écritures · joueur tour {} · {:.0} km/h · {:.1} L",
                    world.elapsed_s,
                    ticks,
                    p.laps + 1,
                    p.speed * 3.6,
                    p.fuel_l
                );
                last_report = Instant::now();
            }

            next += period;
            let now = Instant::now();
            if next > now {
                std::thread::sleep(next - now);
            } else {
                next = now; // we are late: do not try to catch up
            }
        }

        let _ = map.flush();
        drop(map);
        drop(file);
        if !existed {
            let _ = std::fs::remove_file(&args.path);
            println!("{} supprimé.", args.path.display());
        }
        ExitCode::SUCCESS
    }

    /// Minimal SIGINT/SIGTERM handling without extra dependencies: a flag set
    /// from a C signal handler. Only async-signal-safe operations are used.
    fn install_ctrlc(stop: Arc<AtomicBool>) {
        static STOP: std::sync::OnceLock<Arc<AtomicBool>> = std::sync::OnceLock::new();
        let _ = STOP.set(stop);
        extern "C" fn on_signal(_: libc_signal::c_int) {
            if let Some(s) = STOP.get() {
                s.store(true, Ordering::Relaxed);
            }
        }
        // SAFETY: `signal` with a plain `extern "C"` handler that only touches
        // an atomic; both signal numbers are standard.
        let handler = on_signal as extern "C" fn(libc_signal::c_int) as libc_signal::sighandler_t;
        unsafe {
            libc_signal::signal(libc_signal::SIGINT, handler);
            libc_signal::signal(libc_signal::SIGTERM, handler);
        }
    }

    /// The two libc symbols we need, declared by hand to avoid a dependency.
    #[allow(non_camel_case_types)]
    mod libc_signal {
        pub type c_int = i32;
        pub type sighandler_t = usize;
        pub const SIGINT: c_int = 2;
        pub const SIGTERM: c_int = 15;
        extern "C" {
            pub fn signal(signum: c_int, handler: sighandler_t) -> sighandler_t;
        }
    }
}
