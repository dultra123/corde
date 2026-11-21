//! Plays a fake Assetto Corsa Rally stage into three files of a directory, the way the
//! game writes its shared memory: for developing without the game (the game only runs on
//! Windows).
//!
//! ```text
//! cargo run -p corde-acc --example fake-acrally -- /tmp/acrally [stage seconds]
//! corde-server --source acrally --acc-shm /tmp/acrally
//! ```

use corde_acc::rally::fake::FakeRallyStage;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next().map(PathBuf::from) else {
        eprintln!("usage : fake-acrally DOSSIER [durée de la spéciale en secondes]");
        std::process::exit(2);
    };
    let run_s = args.next().and_then(|v| v.parse().ok()).unwrap_or(95.0);
    let fake = FakeRallyStage {
        run_s,
        ..FakeRallyStage::default()
    };
    println!(
        "Faux Assetto Corsa Rally dans {} (une spéciale en {run_s} s, relancée sans fin). Ctrl+C pour arrêter.",
        dir.display()
    );
    let start = Instant::now();
    loop {
        if let Err(e) = fake.at(start.elapsed().as_secs_f64()).write_dir(&dir) {
            eprintln!("écriture impossible : {e}");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
