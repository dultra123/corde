//! `corde-store` — inspect the recordings without the server.
//!
//! ```text
//! corde-store sessions list [--track …] [--type race] [--limit 20]
//! corde-store sessions show <id>
//! corde-store sessions delete <id>
//! corde-store export-csv <id> <lap> [--channels speed_kmh,throttle] [--points 0] [-o file.csv]
//! corde-store channels
//! corde-store stats
//! ```

use clap::{Parser, Subcommand};
use corde_store::{SessionFilter, Store, CHANNELS};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "corde-store",
    version,
    about = "Sessions enregistrées par Corde"
)]
struct Cli {
    /// Data directory (default: the server's, %LOCALAPPDATA%\Corde or ~/.local/share/corde).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List / show / delete sessions.
    Sessions {
        #[command(subcommand)]
        cmd: SessionsCmd,
    },
    /// Export one lap as CSV (all channels by default, full resolution).
    ExportCsv {
        id: String,
        lap: i32,
        /// Comma-separated channel names (default: every channel).
        #[arg(long)]
        channels: Option<String>,
        /// Decimate to at most this many points (0 = all rows).
        #[arg(long, default_value_t = 0)]
        points: usize,
        /// Output file (default: stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// The Parquet columns (name, unit, type).
    Channels,
    /// Sizes on disk.
    Stats,
    /// Check the laps of every recorded session against the game's result
    /// files already imported (official times, laps the game did not time,
    /// last lap of a race). Sessions already checked are skipped.
    Reconcile {
        /// Check every session again, including those already checked.
        #[arg(long)]
        all: bool,
    },
}

#[derive(Subcommand)]
enum SessionsCmd {
    List {
        #[arg(long)]
        track: Option<String>,
        /// race | qualify | practice | warmup | test_day
        #[arg(long = "type")]
        session_type: Option<String>,
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Show {
        id: String,
    },
    Delete {
        id: String,
    },
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("corde-store : {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    if let Cmd::Channels = cli.cmd {
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        writeln!(out, "{:<28} {:<8} type", "name", "unit")?;
        for c in CHANNELS {
            writeln!(out, "{:<28} {:<8} {:?}", c.name, c.unit, c.kind)?;
        }
        return out.flush().or_else(ignore_broken_pipe);
    }
    let dir = cli.data_dir.unwrap_or_else(|| {
        let migration = corde_store::prepare_default_data_dir();
        migration.log();
        migration.dir
    });
    let store = Store::open(&dir)?;
    match cli.cmd {
        Cmd::Channels => unreachable!(),
        Cmd::Stats => {
            let s = store.storage_stats()?;
            println!("{}", serde_json::to_string_pretty(&s)?);
        }
        Cmd::Reconcile { all } => {
            if all {
                store.reconcile_reset()?;
            }
            let reports = store.reconcile_pending(true)?;
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        Cmd::Sessions { cmd } => match cmd {
            SessionsCmd::List {
                track,
                session_type,
                limit,
            } => {
                let session_type = match session_type {
                    Some(t) => Some(serde_json::from_value(serde_json::Value::String(t))?),
                    None => None,
                };
                let rows = store.list_sessions(&SessionFilter {
                    track,
                    session_type,
                    limit: Some(limit),
                })?;
                println!(
                    "{:<44} {:<9} {:<24} {:>5} {:>8} {:>9}",
                    "id", "type", "vehicle", "laps", "best", "samples"
                );
                for r in rows {
                    println!(
                        "{:<44} {:<9} {:<24} {:>5} {:>8} {:>9}",
                        r.id,
                        corde_store::db::enum_str(&r.session_type),
                        truncate(&r.vehicle_model, 24),
                        r.lap_count,
                        r.best_lap_s
                            .map(|b| format!("{b:.3}"))
                            .unwrap_or_else(|| "-".into()),
                        r.sample_count
                    );
                }
            }
            SessionsCmd::Show { id } => {
                let d = store.get_session(&id)?;
                println!("{}", serde_json::to_string_pretty(&d)?);
            }
            SessionsCmd::Delete { id } => {
                store.delete_session(&id)?;
                println!("session {id} supprimée");
            }
        },
        Cmd::ExportCsv {
            id,
            lap,
            channels,
            points,
            output,
        } => {
            let names: Vec<String> = match channels {
                Some(c) => c
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect(),
                None => CHANNELS
                    .iter()
                    .filter(|c| c.name != "t_s" && c.name != "lap_dist_m")
                    .map(|c| c.name.to_string())
                    .collect(),
            };
            let refs: Vec<&str> = names.iter().map(String::as_str).collect();
            let max_points = if points == 0 {
                corde_store::query::MAX_MAX_POINTS
            } else {
                points
            };
            let s = store.lap_samples(&id, lap, &refs, max_points)?;
            let mut out: Box<dyn Write> = match output {
                Some(p) => Box::new(std::io::BufWriter::new(std::fs::File::create(p)?)),
                None => Box::new(std::io::BufWriter::new(std::io::stdout().lock())),
            };
            write!(out, "t_s,lap_dist_m")?;
            for c in &s.channels {
                write!(out, ",{}", c.name)?;
            }
            writeln!(out)?;
            for i in 0..s.points {
                write!(out, "{},{}", s.t_s[i], s.lap_dist_m[i])?;
                for c in &s.channels {
                    match c.values[i] {
                        Some(v) => write!(out, ",{v}")?,
                        None => write!(out, ",")?,
                    }
                }
                writeln!(out)?;
            }
            out.flush()?;
            eprintln!(
                "{} lignes × {} canaux (tour {lap} de {id})",
                s.points,
                s.channels.len()
            );
        }
    }
    Ok(())
}

/// `corde-store channels | head` must not panic.
fn ignore_broken_pipe(e: std::io::Error) -> Result<(), Box<dyn std::error::Error>> {
    if e.kind() == std::io::ErrorKind::BrokenPipe {
        Ok(())
    } else {
        Err(e.into())
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).chain(std::iter::once('…')).collect()
    }
}
