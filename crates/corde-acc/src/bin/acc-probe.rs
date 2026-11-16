//! `acc-probe` — what the pages of Assetto Corsa Competizione hold, read the way Corde
//! reads them. The first thing to run with the game: start a session, then
//!
//! ```text
//! acc-probe             (Windows: the game's shared memory)
//! acc-probe --dump DIR  (also writes a copy of the three pages into DIR)
//! acc-probe --dir DIR   (reads three files instead: a copy made with --dump)
//! ```
//!
//! It prints the versions, the session, the car, the timing and a few live values, ten
//! times, one second apart. Nothing is written to the game. The player's name is neither
//! printed nor kept in a copy (`--dump` blanks it): the copy is the most useful thing to
//! send when a value looks wrong.

use corde_acc::convert::{is_competizione, session_type};
use corde_acc::layout::text;
use corde_acc::reader::open_default_reader;
use corde_acc::Converter;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut dump: Option<PathBuf> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--dump" => dump = args.next().map(PathBuf::from),
            _ => {
                eprintln!("usage : acc-probe [--dump DOSSIER] [--dir DOSSIER]");
                std::process::exit(2);
            }
        }
    }
    let mut reader = match open_default_reader(dir.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Pages du jeu introuvables : {e}");
            std::process::exit(1);
        }
    };
    println!("Source : {}", reader.describe());
    let origin = Instant::now();
    let mut conv = Converter::new();
    for i in 0..10 {
        let pages = match reader.read_raw().and_then(|raw| {
            if let (0, Some(to)) = (i, dump.as_deref()) {
                match raw.clone().without_names().write_dir(to) {
                    Ok(()) => println!(
                        "Copie des trois pages écrite dans {} (sans le nom du joueur)",
                        to.display()
                    ),
                    Err(e) => eprintln!("Copie impossible dans {} : {e}", to.display()),
                }
            }
            raw.pages()
        }) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Lecture impossible : {e}");
                std::process::exit(1);
            }
        };
        let (g, s) = (&pages.graphics, &pages.statics);
        if i == 0 {
            let (sm, ac, car, track) = (s.sm_version, s.ac_version, s.car_model, s.track);
            println!(
                "Interface {} · jeu {} · Competizione : {}",
                text(&sm),
                text(&ac),
                if is_competizione(s) { "oui" } else { "non" }
            );
            println!(
                "Circuit « {} » · voiture « {} » · réservoir {} L · régime max {}",
                text(&track),
                text(&car),
                { s.max_fuel },
                { s.max_rpm }
            );
        }
        let f = conv.frame(&pages, origin.elapsed(), "acc-probe");
        let Some(p) = f.player.as_ref() else { continue };
        println!(
            "état {} · {:?} n°{} · tour {} ({} officiels) · {:.3} s · position sur le tour {:.3} · {:.0} km/h · rapport {} · {:.0} tr/min · carburant {:.1} L · dernier {:?} · meilleur {:?}",
            { g.status },
            session_type(g.session),
            { g.session_index },
            p.lap.lap_number,
            p.lap.laps_completed,
            p.lap.current_lap_s,
            p.lap.lap_fraction,
            p.motion.speed_kmh,
            p.engine.gear,
            p.engine.rpm,
            p.energy.fuel_l,
            p.lap.last_lap_s,
            p.lap.best_lap_s,
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}
