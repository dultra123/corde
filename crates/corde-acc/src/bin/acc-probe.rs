//! `acc-probe` — what the pages of Assetto Corsa Competizione, of the first Assetto Corsa
//! or of Assetto Corsa EVO hold, read the way Corde reads them (the game is recognised by
//! the names of its pages and the version of its interface). The first thing to run with
//! the game: start a session, then
//!
//! ```text
//! acc-probe             (Windows: the game's shared memory)
//! acc-probe --dump DIR  (also writes a copy of the three pages into DIR)
//! acc-probe --dir DIR   (reads three files instead: a copy made with --dump)
//! acc-probe --evo       (Assetto Corsa EVO only, without looking for the others first)
//! ```
//!
//! It prints the versions, the session, the car, the timing and a few live values, ten
//! times, one second apart. Nothing is written to the game. The player's name is neither
//! printed nor kept in a copy (`--dump` blanks it, and replaces the identifiers of the
//! cars of Assetto Corsa EVO by small numbers): the copy is the most useful thing to send
//! when a value looks wrong.

use corde_acc::convert::session_type;
use corde_acc::evo::EvoConverter;
use corde_acc::layout::text;
use corde_acc::reader::{open_default_reader, open_reader, PageReader, EVO_PAGES};
use corde_acc::Converter;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Assetto Corsa EVO: its own pages, its own conversion.
fn evo(mut reader: Box<dyn PageReader>, dump: Option<&Path>) {
    use corde_acc::evo::layout::text;
    println!("Source : {}", reader.describe());
    println!("Jeu reconnu : Assetto Corsa EVO");
    let origin = Instant::now();
    let mut conv = EvoConverter::new();
    for i in 0..10 {
        let raw = match reader.read_raw() {
            Ok(raw) => raw,
            Err(e) => {
                eprintln!("Lecture impossible : {e}");
                std::process::exit(1);
            }
        };
        if i == 0 {
            println!(
                "Tailles des pages lues : physique {} · graphique {} · statique {} octets",
                raw.physics.len(),
                raw.graphics.len(),
                raw.statics.len()
            );
            if let Some(to) = dump {
                match raw
                    .clone()
                    .evo_without_names()
                    .write_dir_of(to, &EVO_PAGES)
                {
                    Ok(()) => println!(
                        "Copie des trois pages écrite dans {} (sans le nom du joueur ni les identifiants des voitures)",
                        to.display()
                    ),
                    Err(e) => eprintln!("Copie impossible dans {} : {e}", to.display()),
                }
            }
        }
        let pages = match raw.evo() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Lecture impossible : {e}");
                std::process::exit(1);
            }
        };
        let (ph, g, s) = (&pages.physics, &pages.graphics, &pages.statics);
        if i == 0 {
            let (sm, build, track, layout, name, car) = (
                s.sm_version,
                s.ac_evo_version,
                s.track,
                s.track_configuration,
                s.session_name,
                g.car_model,
            );
            println!("Interface {} · version du jeu {}", text(&sm), text(&build));
            println!(
                "Circuit « {} » tracé « {} » ({} m) · session « {} » (type {}) · voiture « {} » · réservoir {} L · régime max {}",
                text(&track),
                text(&layout),
                { s.track_length_m },
                text(&name),
                { s.session },
                text(&car),
                { g.max_fuel },
                { ph.current_max_rpm }
            );
        }
        let f = conv.frame(&pages, origin.elapsed(), "acc-probe");
        let Some(p) = f.player.as_ref() else { continue };
        let (acc, e) = (ph.acc_g, g.electronics);
        println!(
            "état {} · tour {} ({} officiels) · {:.3} s · position sur le tour {:.3} · {:.0} km/h · rapport {} · {:.0} tr/min · carburant {:.1} L · dernier {:?} · meilleur {:?}",
            { g.status },
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
        // What still has to be confirmed against the game: the raw values, as written.
        println!(
            "    brut : accélérations [{:+.2} {:+.2} {:+.2}] g · cap {:+.2} rad · volant {:+.2} ({}°) · embrayage physique {:.2} / affiché {:.2} · case {} de {} voitures · position [{:.0} {:.0} {:.0}] · TC {} ABS {} carto {} · tour valide {}",
            acc[0],
            acc[1],
            acc[2],
            { ph.heading },
            { g.steering_percent },
            { g.steer_degrees },
            { ph.clutch },
            { g.clutch_percent },
            p.id,
            { g.active_cars },
            p.motion.pos[0],
            p.motion.pos[1],
            -p.motion.pos[2],
            { e.tc_level },
            { e.abs_level },
            { e.engine_map_level },
            { g.is_valid_lap },
        );
        std::thread::sleep(Duration::from_secs(1));
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut dir: Option<PathBuf> = None;
    let mut dump: Option<PathBuf> = None;
    let mut only_evo = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--dir" => dir = args.next().map(PathBuf::from),
            "--dump" => dump = args.next().map(PathBuf::from),
            "--evo" => only_evo = true,
            _ => {
                eprintln!("usage : acc-probe [--evo] [--dump DOSSIER] [--dir DOSSIER]");
                std::process::exit(2);
            }
        }
    }
    // Competizione and the first Assetto Corsa share the names of their pages; Assetto
    // Corsa EVO has its own: looked for when the others are not there.
    let found = if only_evo {
        Err(None)
    } else {
        open_default_reader(dir.as_deref()).map_err(Some)
    };
    let mut reader = match found {
        Ok(r) => r,
        Err(first) => match open_reader(&EVO_PAGES, dir.as_deref()) {
            Ok(r) => return evo(r, dump.as_deref()),
            Err(e) => {
                if let Some(first) = first {
                    eprintln!("Pages du jeu introuvables : {first}");
                }
                eprintln!("Pages d'Assetto Corsa EVO introuvables : {e}");
                std::process::exit(1);
            }
        },
    };
    println!("Source : {}", reader.describe());
    let origin = Instant::now();
    // Which of the two games: said by the pages themselves.
    let game = match reader.read_raw().and_then(|raw| raw.game()) {
        Ok(Some(game)) => game,
        Ok(None) => {
            eprintln!("Pages sans numéro d'interface : le jeu est-il en piste ?");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Lecture impossible : {e}");
            std::process::exit(1);
        }
    };
    println!("Jeu reconnu : {}", game.label());
    let mut conv = Converter::for_game(game);
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
            raw.pages_of(game)
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
            println!("Interface {} · version du jeu {}", text(&sm), text(&ac));
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
