//! Plays a fake Assetto Corsa EVO session into three files of a directory, the way the
//! game writes its shared memory: for developing without the game (the game only runs on
//! Windows).
//!
//! ```text
//! cargo run -p corde-acc --example fake-acevo -- /tmp/acevo [lap seconds]
//! corde-server --source acevo --acc-shm /tmp/acevo
//! ```

use corde_acc::evo::fake::FakeEvoSession;
use corde_acc::reader::EVO_PAGES;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next().map(PathBuf::from) else {
        eprintln!("usage : fake-acevo DOSSIER [durée d'un tour en secondes]");
        std::process::exit(2);
    };
    let lap_time_s = args.next().and_then(|v| v.parse().ok()).unwrap_or(84.0);
    let fake = FakeEvoSession {
        lap_time_s,
        time_step_ms: 3,
        ..FakeEvoSession::default()
    };
    println!(
        "Faux Assetto Corsa EVO dans {} (un tour en {lap_time_s} s). Ctrl+C pour arrêter.",
        dir.display()
    );
    let start = Instant::now();
    loop {
        if let Err(e) = fake
            .at(start.elapsed().as_secs_f64())
            .write_dir_of(&dir, &EVO_PAGES)
        {
            eprintln!("écriture impossible : {e}");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
