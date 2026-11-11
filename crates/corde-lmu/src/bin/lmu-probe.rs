//! `lmu-probe` — sonde de la mémoire partagée de Le Mans Ultimate.
//!
//! Premier outil à lancer sur le PC de jeu : il ouvre `LMU_Data` (Windows) ou
//! `/dev/shm/LMU_Data` (Linux, via un pont), affiche la version du jeu, le
//! nombre de véhicules et l'état du joueur deux fois par seconde.
//!
//! ```text
//! lmu-probe [--once] [--dump frame.json] [--raw] [--rest] [--interval-ms 500]
//!           [--path /dev/shm/LMU_Data] [--timeout-s 30]
//! ```
//!
//! * `--once`        : attend la première trame, l'affiche et quitte ;
//! * `--dump FILE`   : écrit la dernière `TelemetryFrame` (JSON) à la sortie ;
//! * `--raw`         : affiche aussi quelques champs bruts du bloc ;
//! * `--rest`        : teste l'API REST locale (port lu dans `Settings.JSON`) ;
//! * `--path`        : chemin du miroir (Linux uniquement) ;
//! * `--timeout-s`   : durée maximale d'attente du jeu (défaut 30 s, 0 = infini).
//!
//! Codes de sortie : 0 ok, 1 erreur d'arguments ou d'écriture, 2 jeu
//! inaccessible, 3 jeu ouvert mais aucune donnée reçue dans le délai.

use corde_core::{SourceError, TelemetrySource};
use corde_lmu::source::{LmuSource, LmuSourceConfig};
use corde_lmu::{rest, settings};
use corde_protocol::TelemetryFrame;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// `println!` that quits quietly when stdout is closed (`lmu-probe | head`),
/// instead of panicking on a broken pipe.
macro_rules! out {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let mut so = std::io::stdout().lock();
        if writeln!(so, $($arg)*).and_then(|_| so.flush()).is_err() {
            std::process::exit(0);
        }
    }};
}

#[derive(Debug)]
struct Args {
    once: bool,
    dump: Option<PathBuf>,
    raw: bool,
    rest: bool,
    interval: Duration,
    path: Option<PathBuf>,
    timeout: Option<Duration>,
}

const USAGE: &str = "\
lmu-probe — sonde de la mémoire partagée Le Mans Ultimate (LMU_Data)

Usage : lmu-probe [OPTIONS]
  --once              affiche la première trame reçue puis quitte
  --dump FICHIER      écrit la dernière TelemetryFrame en JSON dans FICHIER
  --raw               affiche aussi quelques champs bruts du bloc
  --rest              teste l'API REST locale (port lu dans Settings.JSON)
  --interval-ms N     intervalle d'affichage en millisecondes (défaut 500)
  --path CHEMIN       (Linux) chemin du miroir, défaut /dev/shm/LMU_Data
  --timeout-s N       attente maximale du jeu en secondes (défaut 30, 0 = infini)
  -h, --help          cette aide";

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        once: false,
        dump: None,
        raw: false,
        rest: false,
        interval: Duration::from_millis(500),
        path: None,
        timeout: Some(Duration::from_secs(30)),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--once" => args.once = true,
            "--raw" => args.raw = true,
            "--rest" => args.rest = true,
            "--dump" => {
                let v = it.next().ok_or("--dump attend un nom de fichier")?;
                args.dump = Some(PathBuf::from(v));
            }
            "--path" => {
                let v = it.next().ok_or("--path attend un chemin")?;
                args.path = Some(PathBuf::from(v));
            }
            "--interval-ms" => {
                let v = it.next().ok_or("--interval-ms attend un nombre")?;
                let ms: u64 = v
                    .parse()
                    .map_err(|_| format!("intervalle invalide : {v}"))?;
                args.interval = Duration::from_millis(ms.max(50));
            }
            "--timeout-s" => {
                let v = it.next().ok_or("--timeout-s attend un nombre")?;
                let s: u64 = v.parse().map_err(|_| format!("délai invalide : {v}"))?;
                args.timeout = (s > 0).then(|| Duration::from_secs(s));
            }
            "-h" | "--help" => {
                out!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("option inconnue : {other}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn not_available_message(err: &SourceError) -> String {
    if cfg!(windows) {
        format!(
            "LMU n'est pas lancé ou \"Enable Plugins\" est désactivé.\n\
             → Dans le jeu : Settings > Gameplay > Enable Plugins = ON, puis redémarrer complètement LMU.\n\
             → Vérifier aussi que lmu-probe tourne sur la même session Windows que le jeu.\n\
             (détail : {err})"
        )
    } else {
        format!(
            "Le miroir de la mémoire partagée est introuvable.\n\
             → Lancer le pont dans le préfixe Proton (winecarte) ou, pour un test, `lmu-shm-fake`.\n\
             → Chemin attendu : /dev/shm/LMU_Data (modifiable avec --path).\n\
             (détail : {err})"
        )
    }
}

fn fmt_opt_time(t: Option<f32>) -> String {
    match t {
        Some(s) if s > 0.0 => {
            let m = (s / 60.0).floor();
            let r = s - m * 60.0;
            if m >= 1.0 {
                format!("{m:.0}:{r:06.3}")
            } else {
                format!("{r:.3}")
            }
        }
        _ => "--".to_string(),
    }
}

fn print_frame(src: &LmuSource, f: &TelemetryFrame, raw: bool) {
    let st = src.status();
    let s = &f.session;
    let version = st
        .game_version
        .map(|v| format!("{} ({v})", corde_lmu::source::format_game_version(v)))
        .unwrap_or_else(|| "?".into());
    out!(
        "── trame #{} · jeu v{} · {} véhicule(s) · {:?} / {:?} · ET {:.1} s · temps réel : {} · {:.0} Hz",
        f.seq,
        version,
        s.num_vehicles,
        s.session_type,
        s.phase,
        s.elapsed_s,
        if s.in_realtime { "oui" } else { "non" },
        st.input_rate_hz
    );
    out!(
        "   circuit : {} ({:.0} m) · air {:.1} °C · piste {:.1} °C · pluie {:.0} %",
        s.track_name,
        s.track_length_m,
        s.weather.ambient_c,
        s.weather.track_c,
        s.weather.rain * 100.0
    );
    match &f.player {
        None => out!("   joueur : aucun véhicule (menu, spectateur ou chargement)"),
        Some(p) => {
            let gear = match p.engine.gear {
                -1 => "R".to_string(),
                0 => "N".to_string(),
                g => g.to_string(),
            };
            out!(
                "   joueur : {} [{}] {} · P{} · tour {} · {:.0} km/h · rapport {} · {:.0} tr/min · essence {:.1} L{}",
                p.vehicle_name,
                p.class_name,
                p.vehicle_model,
                p.place,
                p.lap.lap_number,
                p.motion.speed_kmh,
                gear,
                p.engine.rpm,
                p.energy.fuel_l,
                p.energy
                    .virtual_energy
                    .map(|v| format!(" · VE {v:.2}"))
                    .unwrap_or_default()
            );
            let t = |i: usize| {
                let w = &p.wheels[i];
                format!("{:.0}/{:.0}/{:.0}", w.temp_c[0], w.temp_c[1], w.temp_c[2])
            };
            out!(
                "   pneus °C (g/c/d) : AVG {} · AVD {} · ARG {} · ARD {} · pressions {:.0}/{:.0}/{:.0}/{:.0} kPa",
                t(0),
                t(1),
                t(2),
                t(3),
                p.wheels[0].pressure_kpa,
                p.wheels[1].pressure_kpa,
                p.wheels[2].pressure_kpa,
                p.wheels[3].pressure_kpa
            );
            out!(
                "   tours : dernier {} · meilleur {} · secteur {}{} · delta {:+.2} s · TC {}/{} ABS {}/{} · bias AR {:.1} %",
                fmt_opt_time(p.lap.last_lap_s),
                fmt_opt_time(p.lap.best_lap_s),
                p.lap.current_sector,
                if p.lap.in_pit_lane { " (voie des stands)" } else { "" },
                p.lap.delta_best_s,
                p.electronics.tc,
                p.electronics.tc_max,
                p.electronics.abs,
                p.electronics.abs_max,
                p.electronics.brake_bias_rear * 100.0
            );
        }
    }
    if raw {
        let b = src.raw();
        let t = b.player_telemetry();
        let (ver, active, idx, has) = (
            b.generic.game_version,
            b.telemetry.active_vehicles,
            b.telemetry.player_vehicle_idx,
            b.telemetry.player_has_vehicle,
        );
        out!(
            "   brut : gameVersion={ver} activeVehicles={active} playerVehicleIdx={idx} playerHasVehicle={has} events={:?}",
            b.generic.events
        );
        out!(
            "   brut : userData=\"{}\" options_location={}",
            b.paths.user_data(),
            b.generic.app_info.options_location
        );
        if let Some(t) = t {
            let (id, et, bt, w0) = (t.id, t.elapsed_time, t.wheel[0].brake_temp, t.wheel[0].wear);
            let (ve, soc, regen) = (t.virtual_energy, t.soc, t.regen);
            out!(
                "   brut joueur : mID={id} mElapsedTime={et:.3} mBrakeTemp[FL]={bt:.1} mWear[FL]={w0:.3} mVirtualEnergy={ve} mSoC={soc} mRegen={regen}"
            );
        }
    }
}

fn probe_rest(src: &LmuSource) {
    let paths = settings::locate_game_with_shm(Some(src.raw()));
    let port = paths
        .as_ref()
        .map(|p| p.webui_port())
        .unwrap_or(settings::DEFAULT_WEBUI_PORT);
    match &paths {
        Some(p) => out!(
            "── REST : dossier LMU {} · port {port}",
            p.game_dir.display()
        ),
        None => out!("── REST : dossier LMU introuvable, port par défaut {port}"),
    }
    let client = rest::RestClient::new(port);
    match client.session_info() {
        Ok(v) => out!(
            "   {} → OK ({} clés)",
            rest::PATH_SESSION_INFO,
            v.as_object().map(|o| o.len()).unwrap_or(0)
        ),
        Err(e) => out!("   {} → {e}", rest::PATH_SESSION_INFO),
    }
}

fn dump_frame(path: &PathBuf, frame: &TelemetryFrame) -> Result<(), String> {
    let json = serde_json::to_string_pretty(frame).map_err(|e| e.to_string())?;
    std::fs::write(path, json)
        .map_err(|e| format!("impossible d'écrire {} : {e}", path.display()))?;
    out!("TelemetryFrame écrite dans {}", path.display());
    Ok(())
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(1);
        }
    };

    let mut src = LmuSource::with_config(LmuSourceConfig {
        shm_path: args.path.clone(),
        ..Default::default()
    });
    out!(
        "lmu-probe · source {} · {}",
        src.info().id,
        src.info().label
    );

    // Connexion (avec attente bornée).
    let start = Instant::now();
    let mut announced = false;
    loop {
        match src.connect() {
            Ok(()) => break,
            Err(e @ SourceError::NotAvailable(_)) => {
                if !announced {
                    eprintln!("{}", not_available_message(&e));
                    eprintln!("En attente du jeu…");
                    announced = true;
                }
                if let Some(t) = args.timeout {
                    if start.elapsed() >= t {
                        eprintln!("Abandon après {} s.", t.as_secs());
                        return ExitCode::from(2);
                    }
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                eprintln!("Erreur à l'ouverture de la mémoire partagée : {e}");
                return ExitCode::from(2);
            }
        }
    }
    out!(
        "Mémoire partagée ouverte : {}",
        src.status().reader.unwrap_or_default()
    );

    let mut last_print = Instant::now() - args.interval;
    let mut last_frame: Option<TelemetryFrame> = None;
    let first_wait = Instant::now();
    let mut rest_done = false;
    loop {
        match src.poll() {
            Ok(Some(f)) => {
                last_frame = Some(f);
            }
            Ok(None) => {
                if last_frame.is_none() {
                    if let Some(t) = args.timeout {
                        if first_wait.elapsed() >= t {
                            eprintln!(
                                "Le bloc est ouvert mais aucune donnée ne bouge depuis {} s : le jeu est-il en session (pas dans les menus) ?",
                                t.as_secs()
                            );
                            return ExitCode::from(3);
                        }
                    }
                }
                std::thread::sleep(src.poll_interval());
            }
            Err(SourceError::NotAvailable(msg)) => {
                eprintln!("Connexion perdue : {msg}");
                if args.once || args.timeout.is_some_and(|t| start.elapsed() >= t) {
                    break;
                }
                // reconnexion
                std::thread::sleep(Duration::from_secs(1));
                let _ = src.connect();
                continue;
            }
            Err(e) => {
                eprintln!("Lecture invalide : {e}");
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
        }

        if let Some(f) = &last_frame {
            if last_print.elapsed() >= args.interval || args.once {
                print_frame(&src, f, args.raw);
                last_print = Instant::now();
                if args.rest && !rest_done {
                    probe_rest(&src);
                    rest_done = true;
                }
                if args.once {
                    break;
                }
            }
        }
    }

    match (&args.dump, &last_frame) {
        (Some(path), Some(f)) => {
            if let Err(e) = dump_frame(path, f) {
                eprintln!("{e}");
                return ExitCode::from(1);
            }
        }
        (Some(_), None) => eprintln!("Aucune trame reçue : rien à écrire."),
        _ => {}
    }
    if last_frame.is_some() {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}
