//! Plays a fake rFactor 2 session into two files of a directory, the way the game's
//! plugin writes its shared memory: for developing without the game (the game only runs
//! on Windows).
//!
//! ```text
//! cargo run -p corde-lmu --example fake-rf2 -- /tmp/rf2 [circuit length in metres]
//! corde-server --source rf2 --rf2-shm /tmp/rf2
//! ```

use corde_lmu::rf2::fake::FakeRf2;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(dir) = args.next().map(PathBuf::from) else {
        eprintln!("usage : fake-rf2 DOSSIER [longueur du circuit en mètres]");
        std::process::exit(2);
    };
    let mut game = FakeRf2::new();
    if let Some(len) = args.next().and_then(|v| v.parse::<f64>().ok()) {
        game.world.track_length_m = len.clamp(200.0, 30_000.0);
    }
    println!(
        "Faux rFactor 2 dans {} (circuit de {:.0} m). Ctrl+C pour arrêter.",
        dir.display(),
        game.world.track_length_m
    );
    let mut last = Instant::now();
    loop {
        let now = Instant::now();
        if let Err(e) = game
            .step(now.duration_since(last).as_secs_f64())
            .write_dir(&dir)
        {
            eprintln!("écriture impossible : {e}");
            std::process::exit(1);
        }
        last = now;
        std::thread::sleep(Duration::from_millis(16));
    }
}
