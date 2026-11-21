//! Plays a fake iRacing session into a file, the way the game writes its shared memory:
//! for developing without the game (the game only runs on Windows).
//!
//! ```text
//! cargo run -p corde-iracing --example fake-iracing -- /tmp/iracing.map [lap seconds]
//! corde-server --source iracing --iracing-shm /tmp/iracing.map
//! ```

use corde_iracing::fake::FakeSession;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(path) = args.next().map(PathBuf::from) else {
        eprintln!("usage : fake-iracing FICHIER [durée d'un tour en secondes]");
        std::process::exit(2);
    };
    let lap_s = args.next().and_then(|v| v.parse().ok()).unwrap_or(90.0);
    let fake = FakeSession {
        lap_s,
        ..FakeSession::default()
    };
    println!(
        "Faux iRacing dans {} (un tour en {lap_s} s). Ctrl+C pour arrêter.",
        path.display()
    );
    let start = Instant::now();
    loop {
        if let Err(e) = fake.at(start.elapsed().as_secs_f64()).write(&path) {
            eprintln!("écriture impossible : {e}");
            std::process::exit(1);
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
