//! `rf2-probe`: shows what Corde reads of rFactor 2 (through the shared-memory plugin
//! shipped with the game), once a second for ten seconds. The first thing to run with
//! the game: enable the plugin, start a session, drive, run this.
//!
//! ```text
//! rf2-probe                    read the running game (Windows)
//! rf2-probe --dump DOSSIER     also write a copy of the two buffers, without any name
//! rf2-probe --dir DOSSIER      read the two files of a folder instead of the game
//! ```

use corde_lmu::rf2::{open_reader, Rf2Converter, Rf2Raw};
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let mut dump: Option<PathBuf> = None;
    let mut dir: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dump" => dump = args.next().map(PathBuf::from),
            "--dir" => dir = args.next().map(PathBuf::from),
            _ => {
                eprintln!("usage : rf2-probe [--dump DOSSIER] [--dir DOSSIER]");
                std::process::exit(2);
            }
        }
    }
    let mut reader = match open_reader(dir.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("rFactor 2 introuvable : {e}");
            eprintln!(
                "Vérifie : le jeu est lancé, une session est en cours, et le plugin \
                 rFactor2SharedMemoryMapPlugin64.dll est activé (voir docs/JEUX.md)."
            );
            std::process::exit(1);
        }
    };
    println!("Source : {}", reader.describe());
    let mut raw = Rf2Raw::new();
    let mut conv = Rf2Converter::new();
    for i in 0..10 {
        match reader.read(&mut raw) {
            Ok(true) => {}
            Ok(false) => {
                println!("(zones en cours d'écriture, nouvel essai)");
                std::thread::sleep(Duration::from_millis(50));
                continue;
            }
            Err(e) => {
                eprintln!("Lecture impossible : {e}");
                std::process::exit(1);
            }
        }
        if let (0, Some(to)) = (i, dump.as_deref()) {
            match raw.clone().without_names().write_dir(to) {
                Ok(()) => println!(
                    "Copie des deux zones écrite dans {} (sans aucun nom de pilote)",
                    to.display()
                ),
                Err(e) => eprintln!("Copie impossible dans {} : {e}", to.display()),
            }
        }
        let (tv, sv) = raw.versions();
        let f = conv.frame(&raw, "rf2-probe");
        if i == 0 {
            println!(
                "Circuit « {} » ({:.0} m) · session {:?} · {} véhicule(s) au classement, {} en télémétrie",
                f.session.track_name,
                f.session.track_length_m,
                f.session.session_type,
                raw.scoring_vehicles(),
                raw.telemetry_vehicles()
            );
        }
        match f.player.as_ref() {
            Some(p) => println!(
                "écritures {tv} / {sv} · temps {:.1} s · « {} » ({}) · place {} · tour {} · {:.0} km/h · rapport {} · {:.0} tr/min · carburant {:.1} L · tour en cours {:.3} s · dernier {:?} · meilleur {:?}",
                f.session.elapsed_s,
                p.vehicle_name,
                p.class_name,
                p.place,
                p.lap.lap_number,
                p.motion.speed_kmh,
                p.engine.gear,
                p.engine.rpm,
                p.energy.fuel_l,
                p.lap.current_lap_s,
                p.lap.last_lap_s,
                p.lap.best_lap_s
            ),
            None => println!(
                "écritures {tv} / {sv} · temps {:.1} s · pas de voiture du joueur (au garage ou spectateur ?)",
                f.session.elapsed_s
            ),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
