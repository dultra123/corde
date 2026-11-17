//! `iracing-probe` — what the mapping of iRacing holds, read the way Corde reads it. The
//! first thing to run with the game: get in the car, then
//!
//! ```text
//! iracing-probe               (Windows: the game's shared memory)
//! iracing-probe --dump FILE   (also writes a copy of the mapping into FILE)
//! iracing-probe --file FILE   (reads a copy made with --dump instead)
//! iracing-probe --vars        (also lists every variable the car publishes)
//! ```
//!
//! It prints the session, the car, the timing and a few live values, ten times, one
//! second apart. Nothing is written to the game. No name is printed, and a copy
//! (`--dump`) keeps none: neither the drivers, nor the teams, nor any identifier — only
//! what describes the circuit, the session and the cars. The copy is the most useful
//! thing to send when a value looks wrong.

use corde_iracing::reader::{self, open_default_map};
use corde_iracing::{Converter, Image, SessionDesc};
use std::path::PathBuf;
use std::time::Duration;

fn main() {
    let mut args = std::env::args().skip(1);
    let mut file: Option<PathBuf> = None;
    let mut dump: Option<PathBuf> = None;
    let mut list = false;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--file" => file = args.next().map(PathBuf::from),
            "--dump" => dump = args.next().map(PathBuf::from),
            "--vars" => list = true,
            _ => {
                eprintln!("usage : iracing-probe [--dump FICHIER] [--file FICHIER] [--vars]");
                std::process::exit(2);
            }
        }
    }
    let fail = |what: &str, e: &dyn std::fmt::Display| -> ! {
        eprintln!("{what} : {e}");
        std::process::exit(1);
    };
    let mut map = match open_default_map(file.as_deref()) {
        Ok(m) => m,
        Err(e) => fail("Mémoire du jeu introuvable", &e),
    };
    println!("Source : {}", map.describe());
    let h = match reader::header(map.as_mut()) {
        Ok(h) => h,
        Err(e) => fail("Lecture impossible", &e),
    };
    println!(
        "Interface {} · {} lignes par seconde · {} variables · session en cours : {}",
        h.ver,
        h.tick_rate,
        h.num_vars,
        if h.connected() { "oui" } else { "non" }
    );
    if let Err(e) = reader::check_version(&h) {
        fail("Interface inconnue", &e);
    }
    if let Some(to) = dump.as_deref() {
        match Image::of(map.as_mut()).map(Image::for_diagnostic) {
            Ok(image) => match image.write(to) {
                Ok(()) => println!(
                    "Copie écrite dans {} (sans aucun nom ni identifiant)",
                    to.display()
                ),
                Err(e) => eprintln!("Copie impossible dans {} : {e}", to.display()),
            },
            Err(e) => eprintln!("Copie impossible : {e}"),
        }
    }
    let table = match reader::var_table(map.as_mut(), &h) {
        Ok(t) => t,
        Err(e) => fail("Table des variables illisible", &e),
    };
    let desc = match reader::session_text(map.as_mut(), &h) {
        Ok(text) => SessionDesc::parse(&text),
        Err(e) => fail("Texte de session illisible", &e),
    };
    let me = desc.player();
    println!(
        "Circuit « {} » ({:.0} m) · voiture « {} » ({}) · réservoir {:.1} L · régime max {:.0} · {} voitures · secteurs {:?}",
        desc.track_name,
        desc.track_length_m,
        me.map(|d| d.car.as_str()).unwrap_or("?"),
        me.map(|d| d.class_name.as_str()).unwrap_or("?"),
        desc.fuel_capacity_l,
        desc.red_line_rpm,
        desc.drivers.len(),
        desc.sectors,
    );
    if list {
        for v in table.vars() {
            println!(
                "  {:<28} {:?} × {:<3} {:<8} {}",
                v.name, v.kind, v.count, v.unit, v.desc
            );
        }
    }
    let mut conv = Converter::new();
    for _ in 0..10 {
        let tick = match reader::latest_row(map.as_mut()) {
            Ok(t) => t,
            Err(e) => fail("Lecture impossible", &e),
        };
        let row = table.row(&tick.row);
        let f = conv.frame(&desc, row, "iracing-probe");
        match f.player.as_ref() {
            Some(p) => println!(
                "ligne {} · {:?} n°{} · {:?} · en piste {} · tour {} ({} officiels, compteurs du jeu {:?} / {:?}) · {:.3} s · position sur le tour {:.3} · {:.0} km/h · rapport {} · {:.0} tr/min · carburant {:.1} L · dernier {:?} (jeu : {:?}) · meilleur {:?}",
                tick.tick,
                f.session.session_type,
                f.session.session_index,
                f.session.phase,
                f.session.in_realtime,
                p.lap.lap_number,
                p.lap.laps_completed,
                row.i32("Lap"),
                row.i32("LapCompleted"),
                p.lap.current_lap_s,
                p.lap.lap_fraction,
                p.motion.speed_kmh,
                p.engine.gear,
                p.engine.rpm,
                p.energy.fuel_l,
                p.lap.last_lap_s,
                row.num("LapLastLapTime"),
                p.lap.best_lap_s,
            ),
            None => println!("ligne {} · pas de voiture du joueur", tick.tick),
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}
