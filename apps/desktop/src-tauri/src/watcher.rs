//! Le veilleur : un thread qui interroge le serveur embarqué une fois par seconde
//! (`GET /api/status`) et réagit aux changements d'état du jeu.
//!
//! - le pilote entre en piste → la fenêtre est ressortie si elle était cachée ou réduite,
//!   sans prendre le premier plan au jeu
//!   (`shell::reveal(TrackEntry)`) ; l'UI passe d'elle-même au tableau de bord ;
//! - le jeu est détecté / fermé → seule l'info-bulle de la zone de notification change :
//!   une application démarrée cachée reste cachée tant que le pilote n'est pas en piste.

use crate::edges::{Edge, EdgeDetector};
use crate::server;
use crate::shell::{self, Reveal};
use crate::tray;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;
use tauri::AppHandle;

/// Période d'interrogation (1 Hz).
const POLL_PERIOD: Duration = Duration::from_secs(1);
/// Délai maximal d'une requête vers la boucle locale.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Poignée du thread veilleur.
pub struct Watcher {
    stop: mpsc::Sender<()>,
}

impl Watcher {
    /// Demande l'arrêt (le thread sort à sa prochaine itération ; on ne l'attend pas).
    pub fn stop(self) {
        let _ = self.stop.send(());
    }
}

/// Lance le veilleur sur le serveur embarqué qui écoute sur `port`.
pub fn spawn(app: &AppHandle, port: u16) -> Option<Watcher> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let app = app.clone();
    match std::thread::Builder::new()
        .name("corde-watcher".into())
        .spawn(move || run(&app, port, &stop_rx))
    {
        Ok(_) => Some(Watcher { stop: stop_tx }),
        Err(e) => {
            tracing::error!("veilleur impossible à lancer : {e}");
            None
        }
    }
}

fn run(app: &AppHandle, port: u16, stop: &mpsc::Receiver<()>) {
    let agent = server::local_agent(REQUEST_TIMEOUT);
    let mut edges = EdgeDetector::new();
    loop {
        // Serveur muet (arrêt en cours, surcharge) : on garde le dernier état connu.
        if let Some(status) = server::game_status(&agent, port) {
            match edges.update(status) {
                Edge::EnteredTrack => {
                    tracing::info!("le pilote entre en piste");
                    tray::set_tooltip(app, tray::TOOLTIP_ON_TRACK);
                    shell::reveal(app, Reveal::TrackEntry);
                }
                Edge::LeftTrack => {
                    tracing::info!("le pilote quitte la piste");
                    tray::set_tooltip(app, tray::TOOLTIP_GAME);
                }
                Edge::GameDetected => {
                    tracing::info!("jeu détecté");
                    tray::set_tooltip(app, tray::TOOLTIP_GAME);
                }
                Edge::GameLost => {
                    tracing::info!("jeu fermé");
                    tray::set_tooltip(app, tray::TOOLTIP_WAITING);
                }
                Edge::None => {}
            }
        }
        match stop.recv_timeout(POLL_PERIOD) {
            Err(RecvTimeoutError::Timeout) => {}
            // Arrêt demandé, ou coquille détruite.
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    tracing::debug!("veilleur arrêté");
}
