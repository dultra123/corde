//! `corde-server` executable: parse the CLI, start the server, print the banner, wait for Ctrl-C.
//!
//! On the home server, three commands talk to the running service (`corde_server::hub::cli`):
//! `corde-server pair` (a pairing code), `corde-server devices` (paired devices) and
//! `corde-server revoke <id>`.

use clap::{Parser, Subcommand};
use corde_server::hub::cli;
use corde_server::{shutdown_signal, Server, ServerConfig};
use std::io::IsTerminal;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "corde-server",
    version,
    about = "Serveur local Corde : télémétrie → WebSocket/HTTP pour les tableaux de bord ; avec --role hub, serveur Corde de la maison",
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    #[command(flatten)]
    server: ServerConfig,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Home server: print a pairing code (8 digits, single use, valid 10 minutes) to type on
    /// the PC or the phone. Run it on the server itself (sudo corde-server pair).
    Pair {
        /// The device paired with this code becomes an administrator.
        #[arg(long)]
        admin: bool,
        /// Data folder of the service (default: /var/lib/corde when installed).
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// Home server: list the paired devices.
    Devices {
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
    /// Home server: revoke a paired device at once (id from `corde-server devices`).
    Revoke {
        id: String,
        #[arg(long)]
        data_dir: Option<PathBuf>,
    },
}

/// Logs go to stderr (the banner uses stdout); colours only on a terminal.
fn init_logging(filter: &str) {
    let filter = EnvFilter::try_new(filter).unwrap_or_else(|e| {
        eprintln!("--log {filter:?} invalide ({e}), utilisation de `info`");
        EnvFilter::new("info")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .with_target(false)
        .compact()
        .init();
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Some(cmd) = cli.command {
        if let Err(e) = run_command(cmd) {
            eprintln!("corde-server : {e:#}");
            std::process::exit(1);
        }
        return;
    }
    let cfg = cli.server;
    init_logging(&cfg.log);
    if let Err(e) = run(cfg).await {
        eprintln!("corde-server : {e:#}");
        std::process::exit(1);
    }
}

fn run_command(cmd: Command) -> anyhow::Result<()> {
    match cmd {
        Command::Pair { admin, data_dir } => {
            let dir = cli::resolve_data_dir(data_dir);
            let code = cli::pair(&dir, admin)?;
            println!();
            println!("  Code d'appairage : {}", cli::format_code(&code.code));
            println!();
            println!(
                "  Valable {} minutes, une seule fois{}.",
                code.expires_in_s / 60,
                if code.admin {
                    " — l'appareil appairé sera administrateur"
                } else {
                    ""
                }
            );
            println!("  Sur le PC : Corde → Réglages → Serveur Corde → Appairer.");
            println!("  Sur le téléphone : ouvre l'adresse du serveur et tape ce code.");
            println!("  Trois codes faux l'annulent : relance alors « sudo corde-server pair ».");
            println!();
        }
        Command::Devices { data_dir } => {
            let dir = cli::resolve_data_dir(data_dir);
            let list = cli::devices(&dir)?;
            if list.is_empty() {
                println!("Aucun appareil appairé. Code : sudo corde-server pair");
            }
            for d in list {
                println!(
                    "{}  {:<30}  {}  appairé {}  vu {}",
                    d.id,
                    d.name,
                    if d.admin { "admin" } else { "     " },
                    cli::format_date(Some(d.created_ms)),
                    cli::format_date(d.last_used_ms)
                );
            }
        }
        Command::Revoke { id, data_dir } => {
            let dir = cli::resolve_data_dir(data_dir);
            if cli::revoke(&dir, &id)? {
                println!("Appareil {id} révoqué : il n'a plus accès au serveur.");
            } else {
                anyhow::bail!("aucun appareil {id} (voir : sudo corde-server devices)");
            }
        }
    }
    Ok(())
}

async fn run(cfg: ServerConfig) -> anyhow::Result<()> {
    let server = Server::bind(cfg).await?;
    println!();
    for line in server.banner() {
        println!("{line}");
    }
    println!("  Ctrl-C pour arrêter.");
    println!();

    server.run(shutdown_signal()).await?;
    tracing::info!("serveur arrêté");
    Ok(())
}
