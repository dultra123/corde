//! `corde-server` executable: parse the CLI, start the server, print the banner, wait for Ctrl-C.

use clap::Parser;
use corde_server::{shutdown_signal, Server, ServerConfig};
use std::io::IsTerminal;
use tracing_subscriber::EnvFilter;

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
    let cfg = ServerConfig::parse();
    init_logging(&cfg.log);
    if let Err(e) = run(cfg).await {
        eprintln!("corde-server : {e:#}");
        std::process::exit(1);
    }
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
