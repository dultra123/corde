//! `corde-sim` — command-line front end of the simulator.
//!
//! ```text
//! corde-sim --json 100                # 100 frames as JSON lines (as fast as possible)
//! corde-sim --json 0 --realtime       # endless stream paced at --rate
//! corde-sim --rate 50 --seed 7 --class hypercar --rain 0.3 --ai 12
//! corde-sim --speed 10                # human-readable ticker (1 line / s), 10× faster than real time
//! corde-sim                           # human-readable ticker (1 line / s) until Ctrl-C
//! ```

use clap::Parser;
use corde_core::TelemetrySource;
use corde_protocol::TelemetryFrame;
use corde_sim::{PlayerClass, SimConfig, SimSource, SimWorld};
use std::io::{self, BufWriter, Write};
use std::time::{Duration, Instant};

#[derive(Debug, Parser)]
#[command(
    name = "corde-sim",
    version,
    about = "Simulateur de télémétrie Corde (sans le jeu)"
)]
struct Cli {
    /// Print N frames as JSON lines on stdout (0 = endless). Without this flag a
    /// human-readable ticker is shown.
    #[arg(long, value_name = "N")]
    json: Option<u64>,
    /// Tick rate of the world, Hz.
    #[arg(long, default_value_t = corde_sim::DEFAULT_RATE_HZ)]
    rate: f32,
    /// PRNG seed (same seed → same frames).
    #[arg(long, default_value_t = SimConfig::default().seed)]
    seed: u64,
    /// Player car class: gt3 | hypercar.
    #[arg(long, default_value_t = PlayerClass::Gt3)]
    class: PlayerClass,
    /// Rain intensity 0..1.
    #[arg(long, default_value_t = 0.0)]
    rain: f32,
    /// Number of AI cars.
    #[arg(long, default_value_t = corde_sim::DEFAULT_NUM_AI)]
    ai: usize,
    /// Nominal lap time of the player, seconds.
    #[arg(long, default_value_t = corde_sim::DEFAULT_LAP_TIME_S)]
    lap_time: f32,
    /// Track length, metres.
    #[arg(long, default_value_t = corde_sim::DEFAULT_TRACK_LENGTH_M)]
    track_length: f32,
    /// Laps already completed when the world starts.
    #[arg(long, default_value_t = 2)]
    laps_done: i32,
    /// Pace `--json` output on the real clock instead of generating instantly.
    #[arg(long)]
    realtime: bool,
    /// Real-time speed factor (ticker and `--realtime`): 10 = ten times faster than real time.
    #[arg(long, default_value_t = 1.0)]
    speed: f64,
    /// Pretty-print JSON (one frame per line is kept).
    #[arg(long)]
    pretty: bool,
    /// Write a native `LMU_Data` block to shared memory (`/dev/shm/LMU_Data`)
    /// so that `corde-lmu` can read it as if the game were running.
    #[arg(long)]
    shm: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    if cli.shm {
        // TODO(sim-shm): write a `SharedMemoryObjectOut` block (see
        // corde-lmu::layout) to /dev/shm/LMU_Data (Linux) or the "LMU_Data"
        // file mapping (Windows), converted from each TelemetryFrame.
        anyhow::bail!("--shm n'est pas encore implémenté (TODO(sim-shm))");
    }

    let cfg = SimConfig {
        track_length_m: cli.track_length,
        lap_time_s: cli.lap_time,
        num_ai: cli.ai,
        rate_hz: cli.rate,
        seed: cli.seed,
        rain: cli.rain,
        player_class: cli.class,
        start_laps_completed: cli.laps_done,
        time_scale: cli.speed,
        ..SimConfig::default()
    }
    .sanitized();

    match cli.json {
        Some(n) if cli.realtime => stream_json_realtime(cfg, n, cli.pretty),
        Some(n) => dump_json(cfg, n, cli.pretty),
        None => ticker(cfg),
    }
}

fn write_frame(out: &mut impl Write, frame: &TelemetryFrame, pretty: bool) -> io::Result<()> {
    if pretty {
        serde_json::to_writer_pretty(&mut *out, frame)?;
    } else {
        serde_json::to_writer(&mut *out, frame)?;
    }
    out.write_all(b"\n")
}

/// Generate `n` frames (0 = endless) as fast as possible.
fn dump_json(cfg: SimConfig, n: u64, pretty: bool) -> anyhow::Result<()> {
    let mut world = SimWorld::new(cfg);
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut i = 0u64;
    while n == 0 || i < n {
        let frame = world.tick();
        if let Err(e) = write_frame(&mut out, &frame, pretty) {
            if e.kind() == io::ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(e.into());
        }
        i += 1;
    }
    out.flush()?;
    Ok(())
}

/// Same as [`dump_json`] but paced on the real clock through [`SimSource`].
fn stream_json_realtime(cfg: SimConfig, n: u64, pretty: bool) -> anyhow::Result<()> {
    let mut src = SimSource::new(cfg);
    src.connect()?;
    let stdout = io::stdout();
    let mut out = BufWriter::new(stdout.lock());
    let mut i = 0u64;
    while n == 0 || i < n {
        match src.poll()? {
            Some(frame) => {
                if let Err(e) = write_frame(&mut out, &frame, pretty) {
                    if e.kind() == io::ErrorKind::BrokenPipe {
                        return Ok(());
                    }
                    return Err(e.into());
                }
                out.flush()?;
                i += 1;
            }
            None => std::thread::sleep(src.poll_interval()),
        }
    }
    Ok(())
}

/// One human-readable line per second, until Ctrl-C.
fn ticker(cfg: SimConfig) -> anyhow::Result<()> {
    let mut src = SimSource::new(cfg);
    src.connect()?;
    let info = src.info();
    eprintln!("{} — Ctrl-C pour arrêter", info.label);
    let mut last_print = Instant::now() - Duration::from_secs(1);
    let mut last_lap: Option<i32> = None;
    loop {
        let Some(frame) = src.poll()? else {
            std::thread::sleep(src.poll_interval());
            continue;
        };
        let Some(p) = frame.player.as_ref() else {
            continue;
        };
        let lap_changed = last_lap.is_some_and(|l| l != p.lap.lap_number);
        last_lap = Some(p.lap.lap_number);
        if lap_changed {
            if let Some(t) = p.lap.last_lap_s {
                eprintln!(
                    "── tour {} bouclé en {} (meilleur {})",
                    p.lap.lap_number,
                    fmt_lap(t),
                    p.lap.best_lap_s.map(fmt_lap).unwrap_or_else(|| "—".into())
                );
            }
        }
        if last_print.elapsed() >= Duration::from_secs(1) {
            last_print = Instant::now();
            let w = &p.wheels;
            eprintln!(
                "seq {:>7} | P{:<2} L{:<3} S{} {:>6.1}s | {:>5.0} km/h G{} {:>5.0} rpm | thr {:>3.0}% brk {:>3.0}% | pneus {:>3.0}/{:>3.0}/{:>3.0}/{:>3.0} °C | carb {:>5.1} L | Δ {:+.2}",
                frame.seq,
                p.place,
                p.lap.lap_number + 1,
                p.lap.current_sector,
                p.lap.current_lap_s,
                p.motion.speed_kmh,
                p.engine.gear,
                p.engine.rpm,
                p.inputs.throttle * 100.0,
                p.inputs.brake * 100.0,
                avg(&w[0].temp_c),
                avg(&w[1].temp_c),
                avg(&w[2].temp_c),
                avg(&w[3].temp_c),
                p.energy.fuel_l,
                p.lap.delta_best_s,
            );
        }
    }
}

fn avg(t: &[f32; 3]) -> f32 {
    (t[0] + t[1] + t[2]) / 3.0
}

fn fmt_lap(t: f32) -> String {
    let m = (t / 60.0).floor();
    format!("{}:{:06.3}", m as i32, t - m * 60.0)
}
