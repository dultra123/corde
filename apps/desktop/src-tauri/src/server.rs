//! Le serveur Corde embarqué dans l'application de bureau.
//!
//! Même code que l'exécutable `corde-server` ([`corde_server::Server`]), avec sa
//! configuration par défaut (source `auto` = Le Mans Ultimate, enregistrement et import des
//! résultats actifs), mais lancé dans un thread de l'application avec son propre runtime
//! tokio, sans console :
//!
//! ```text
//! start() ─▶ thread « corde-server » ─▶ runtime tokio ─▶ Server::bind ─▶ Server::run
//!    │              └─ Started : Ok(adresse) ou Err(StartError)   (une seule fois)
//!    └─ EmbeddedServer::stop() ─▶ arrêt propre : axum se termine, le thread source est
//!                                 rejoint, l'enregistrement en cours est fermé (flush).
//! ```
//!
//! Ce module ne dépend pas de Tauri : il se teste seul.

use corde_server::{Server, ServerConfig, SourceKind};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use tokio::sync::oneshot;

/// Premier port essayé : celui de `corde-server` (8471).
pub const FIRST_PORT: u16 = corde_server::config::DEFAULT_PORT;
/// Dernier port de repli (8480).
pub const LAST_PORT: u16 = FIRST_PORT + 9;
/// Attente maximale du démarrage (ouverture de la base + `/api/health`).
pub const START_TIMEOUT: Duration = Duration::from_secs(20);
/// Attente maximale de l'arrêt propre avant de laisser le processus se terminer.
pub const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Ce que la coquille décide pour le serveur ; tout le reste est la configuration par
/// défaut de `corde-server`.
#[derive(Debug, Clone)]
pub struct Options {
    /// Dossier de données (`%LOCALAPPDATA%\Corde`).
    pub data_dir: PathBuf,
    /// Dossier de l'UI web construite ; `None` = recherche par défaut du serveur.
    pub ui_dir: Option<PathBuf>,
    /// Adresse d'écoute (voir `settings::resolve_bind_ip`).
    pub bind_ip: IpAddr,
    /// Source de télémétrie (`Auto` en production).
    pub source: SourceKind,
    /// Plage de ports essayée, bornes comprises.
    pub first_port: u16,
    pub last_port: u16,
}

impl Options {
    pub fn new(data_dir: PathBuf, ui_dir: Option<PathBuf>, bind_ip: IpAddr) -> Self {
        Self {
            data_dir,
            ui_dir,
            bind_ip,
            source: SourceKind::Auto,
            first_port: FIRST_PORT,
            last_port: LAST_PORT,
        }
    }
}

/// Pourquoi le serveur embarqué n'a pas démarré. `Display` donne le message montré à
/// l'utilisateur (page d'erreur), en français.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartError {
    /// Un autre serveur Corde répond déjà sur ce port (`corde-server.exe` en console).
    AlreadyRunning { port: u16 },
    /// Tous les ports de la plage sont pris.
    NoFreePort { first: u16, last: u16 },
    /// `Server::bind` a échoué (dossier de données, port…).
    Bind(String),
    /// Le thread ou le runtime du serveur n'a pas pu être créé, ou s'est arrêté.
    Runtime(String),
    /// Le serveur écoute mais `/api/health` n'a pas répondu à temps.
    Timeout { port: Option<u16> },
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // legacy name (Muret, before 0.3.0): l'ancienne application occupe le même port.
            StartError::AlreadyRunning { port } => write!(
                f,
                "Une autre copie tourne déjà sur ce PC (port {port}). Ce peut être l'ancienne \
                 application Muret, encore ouverte : clic droit sur son icône près de l'horloge \
                 → Quitter. Ou « corde-server.exe » resté ouvert dans une console : ferme-le. \
                 Puis clique sur Réessayer."
            ),
            StartError::NoFreePort { first, last } => write!(
                f,
                "Aucun port libre entre {first} et {last} : d'autres programmes les utilisent \
                 tous. Ferme-les ou redémarre le PC, puis clique sur Réessayer."
            ),
            StartError::Bind(e) => write!(f, "Le serveur Corde n'a pas pu démarrer : {e}"),
            StartError::Runtime(e) => write!(f, "Le serveur Corde s'est arrêté au démarrage : {e}"),
            StartError::Timeout { port: Some(port) } => write!(
                f,
                "Le serveur Corde (port {port}) n'a pas répondu en {} secondes.",
                START_TIMEOUT.as_secs()
            ),
            StartError::Timeout { port: None } => write!(
                f,
                "Le serveur Corde n'a pas démarré en {} secondes.",
                START_TIMEOUT.as_secs()
            ),
        }
    }
}

/// Reçoit une seule fois le résultat du démarrage : l'adresse d'écoute, ou l'erreur.
pub type Started = mpsc::Receiver<Result<SocketAddr, StartError>>;

/// Poignée sur le thread du serveur embarqué.
pub struct EmbeddedServer {
    shutdown: Option<oneshot::Sender<()>>,
    done: mpsc::Receiver<()>,
    thread: Option<JoinHandle<()>>,
}

/// Prévient [`EmbeddedServer::stop`] que le thread est terminé, même s'il a paniqué.
struct DoneGuard(mpsc::Sender<()>);

impl Drop for DoneGuard {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

/// Lance le serveur dans son thread et rend la main tout de suite.
pub fn start(opts: Options) -> std::io::Result<(EmbeddedServer, Started)> {
    let (started_tx, started_rx) = mpsc::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("corde-server".into())
        .spawn(move || {
            let _done = DoneGuard(done_tx);
            serve(opts, started_tx, shutdown_rx);
        })?;
    Ok((
        EmbeddedServer {
            shutdown: Some(shutdown_tx),
            done: done_rx,
            thread: Some(thread),
        },
        started_rx,
    ))
}

impl EmbeddedServer {
    /// Demande l'arrêt propre et attend la fin du thread (au plus `timeout`).
    ///
    /// L'arrêt ferme l'enregistrement en cours : `Server::run` rejoint le thread source, qui
    /// vide et clôt les fichiers Parquet et la session SQLite. Retourne `false` si le thread
    /// n'a pas fini à temps (le processus peut alors se terminer quand même : un point de
    /// reprise est écrit au moins toutes les 30 s).
    pub fn stop(&mut self, timeout: Duration) -> bool {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        let Some(thread) = self.thread.take() else {
            return true;
        };
        match self.done.recv_timeout(timeout) {
            // Terminé (ou canal fermé : le thread n'existe plus).
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = thread.join();
                true
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                tracing::warn!(
                    "le serveur embarqué n'a pas fini de s'arrêter en {} s",
                    timeout.as_secs()
                );
                false
            }
        }
    }
}

impl Drop for EmbeddedServer {
    fn drop(&mut self) {
        // Filet de sécurité : sans `stop()` explicite, on demande quand même l'arrêt.
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Corps du thread serveur.
fn serve(
    opts: Options,
    started: mpsc::Sender<Result<SocketAddr, StartError>>,
    shutdown: oneshot::Receiver<()>,
) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name("corde-server-io")
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            let _ = started.send(Err(StartError::Runtime(format!("runtime tokio : {e}"))));
            return;
        }
    };

    runtime.block_on(async move {
        let port = match pick_port(opts.bind_ip, opts.first_port, opts.last_port) {
            Ok(port) => port,
            Err(e) => {
                tracing::error!("{e}");
                let _ = started.send(Err(e));
                return;
            }
        };
        let cfg = ServerConfig {
            port,
            // L'adresse exacte sur laquelle le port vient d'être vérifié.
            bind: Some(opts.bind_ip),
            source: opts.source,
            ui_dir: opts.ui_dir,
            data_dir: Some(opts.data_dir),
            // Tout le reste comme `corde-server` sans option : enregistrement automatique à
            // 50 Hz, import des fichiers de résultats, 20 Hz par tableau de bord.
            ..ServerConfig::default()
        };
        let server = match Server::bind(cfg).await {
            Ok(server) => server,
            Err(e) => {
                tracing::error!("démarrage du serveur embarqué impossible : {e:#}");
                let _ = started.send(Err(StartError::Bind(format!("{e:#}"))));
                return;
            }
        };
        let addr = server.local_addr();
        for line in server.banner() {
            tracing::info!("{line}");
        }
        let _ = started.send(Ok(addr));

        // Résolu par `stop()`, ou si la poignée est détruite.
        let shutdown = async move {
            let _ = shutdown.await;
        };
        match server.run(shutdown).await {
            Ok(()) => tracing::info!("serveur embarqué arrêté"),
            Err(e) => tracing::error!("le serveur embarqué s'est arrêté sur une erreur : {e:#}"),
        }
    });
    // Ne pas attendre indéfiniment une tâche bloquante en cours (analyse, import).
    runtime.shutdown_timeout(Duration::from_secs(2));
}

/// Choisit le port d'écoute : `first`, sinon le premier port libre jusqu'à `last`.
///
/// Sous Windows, écouter sur `127.0.0.1:8471` peut réussir alors qu'un `corde-server.exe`
/// écoute déjà sur `0.0.0.0:8471` (les deux liaisons coexistent) : on interroge donc d'abord
/// `/api/health` pour ne pas lancer deux serveurs sur les mêmes données.
fn pick_port(ip: IpAddr, first: u16, last: u16) -> Result<u16, StartError> {
    if health(first, Duration::from_millis(400)).is_some() {
        return Err(StartError::AlreadyRunning { port: first });
    }
    for port in first..=last {
        match TcpListener::bind(SocketAddr::new(ip, port)) {
            Ok(listener) => {
                drop(listener);
                if port != first {
                    tracing::warn!(
                        "port {first} occupé : le serveur embarqué utilise le port {port}"
                    );
                }
                return Ok(port);
            }
            Err(e) => tracing::warn!("port {port} indisponible sur {ip} : {e}"),
        }
    }
    Err(StartError::NoFreePort { first, last })
}

/// Client HTTP pour la boucle locale : pas de proxy (même si `HTTP_PROXY` est défini sur
/// la machine), délai court, les statuts 4xx/5xx ne sont pas des erreurs.
pub fn local_agent(timeout: Duration) -> ureq::Agent {
    ureq::config::Config::builder()
        .proxy(None)
        .http_status_as_error(false)
        .timeout_global(Some(timeout))
        .build()
        .new_agent()
}

/// URL d'une route du serveur embarqué, toujours par la boucle locale.
pub fn local_url(port: u16, path: &str) -> String {
    format!("http://{}:{port}{path}", Ipv4Addr::LOCALHOST)
}

fn get_json(agent: &ureq::Agent, url: &str) -> Option<serde_json::Value> {
    let mut response = agent.get(url).call().ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.body_mut().read_json::<serde_json::Value>().ok()
}

/// `GET /api/health` : `Some(version)` si un serveur Corde répond sur ce port.
pub fn health(port: u16, timeout: Duration) -> Option<String> {
    let v = get_json(&local_agent(timeout), &local_url(port, "/api/health"))?;
    let is_corde = v.get("ok").and_then(serde_json::Value::as_bool) == Some(true)
        && v.get("protocol_version").is_some();
    is_corde.then(|| {
        v.get("version")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("?")
            .to_string()
    })
}

/// Attend que `/api/health` réponde (interrogé toutes les 100 ms, au plus `max`).
pub fn wait_healthy(port: u16, max: Duration) -> bool {
    let deadline = Instant::now() + max;
    loop {
        if health(port, Duration::from_millis(1000)).is_some() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Ce que le veilleur lit dans `GET /api/status` (`corde_protocol::SourceStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameStatus {
    /// Le jeu est détecté (mémoire partagée ouverte et vivante).
    pub connected: bool,
    /// Le pilote est en piste.
    pub in_realtime: bool,
}

/// `GET /api/status` ; `None` si le serveur ne répond pas.
pub fn game_status(agent: &ureq::Agent, port: u16) -> Option<GameStatus> {
    let v = get_json(agent, &local_url(port, "/api/status"))?;
    let flag = |key: &str| {
        v.get(key)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
    };
    Some(GameStatus {
        connected: flag("connected"),
        in_realtime: flag("in_realtime"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "corde-desktop-server-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    /// Une plage de 4 ports consécutifs libres, loin de 8471 (qui peut être pris par un
    /// vrai Corde sur la machine de développement). Chaque appel reçoit une plage
    /// différente : les tests tournent en parallèle.
    fn free_range() -> (u16, u16) {
        use std::sync::atomic::{AtomicU16, Ordering};
        static NEXT_SLOT: AtomicU16 = AtomicU16::new(0);
        for _ in 0..500 {
            let first = 18471 + NEXT_SLOT.fetch_add(1, Ordering::Relaxed) * 8;
            let free =
                (first..first + 4).all(|p| TcpListener::bind((Ipv4Addr::LOCALHOST, p)).is_ok());
            if free {
                return (first, first + 3);
            }
        }
        panic!("aucune plage de ports libre");
    }

    fn sim_options(tag: &str, first: u16, last: u16) -> Options {
        Options {
            data_dir: temp_dir(tag),
            ui_dir: None,
            bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            source: SourceKind::Sim,
            first_port: first,
            last_port: last,
        }
    }

    #[test]
    fn starts_answers_health_and_status_then_stops_cleanly() {
        let (first, last) = free_range();
        let opts = sim_options("basic", first, last);
        let data_dir = opts.data_dir.clone();
        let (mut server, started) = start(opts).unwrap();
        let addr = started.recv_timeout(START_TIMEOUT).unwrap().unwrap();
        assert_eq!(addr.port(), first);
        assert!(addr.ip().is_loopback());
        assert!(wait_healthy(first, Duration::from_secs(10)));
        assert!(health(first, Duration::from_secs(1)).is_some());

        // Le simulateur est « en piste » au bout de quelques instants.
        let agent = local_agent(Duration::from_secs(2));
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut status = GameStatus::default();
        while Instant::now() < deadline {
            status = game_status(&agent, first).unwrap();
            if status.connected && status.in_realtime {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(status.connected && status.in_realtime, "{status:?}");

        let t0 = Instant::now();
        assert!(server.stop(STOP_TIMEOUT));
        assert!(t0.elapsed() < Duration::from_secs(8), "{:?}", t0.elapsed());
        // Le port est rendu, la base existe.
        assert!(health(first, Duration::from_millis(300)).is_none());
        assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, first)).is_ok());
        assert!(data_dir.join("corde.db").is_file());
        // Un second stop est sans effet.
        assert!(server.stop(Duration::from_millis(10)));
        std::fs::remove_dir_all(&data_dir).ok();
    }

    #[test]
    fn falls_back_to_the_next_free_port() {
        let (first, last) = free_range();
        // Un programme quelconque (pas Corde) occupe le premier port.
        let squatter = TcpListener::bind((Ipv4Addr::LOCALHOST, first)).unwrap();
        let opts = sim_options("fallback", first, last);
        let data_dir = opts.data_dir.clone();
        let (mut server, started) = start(opts).unwrap();
        let addr = started.recv_timeout(START_TIMEOUT).unwrap().unwrap();
        assert_eq!(addr.port(), first + 1);
        assert!(wait_healthy(first + 1, Duration::from_secs(10)));
        assert!(server.stop(STOP_TIMEOUT));
        drop(squatter);
        std::fs::remove_dir_all(&data_dir).ok();
    }

    #[test]
    fn refuses_to_start_next_to_another_corde() {
        let (first, last) = free_range();
        let opts = sim_options("first", first, last);
        let dir_a = opts.data_dir.clone();
        let (mut a, started) = start(opts).unwrap();
        started.recv_timeout(START_TIMEOUT).unwrap().unwrap();
        assert!(wait_healthy(first, Duration::from_secs(10)));

        let opts = sim_options("second", first, last);
        let dir_b = opts.data_dir.clone();
        let (mut b, started) = start(opts).unwrap();
        let err = started.recv_timeout(START_TIMEOUT).unwrap().unwrap_err();
        assert_eq!(err, StartError::AlreadyRunning { port: first });
        assert!(err.to_string().contains("corde-server.exe"));
        assert!(err.to_string().contains("Quitter"), "old application hint");
        assert!(b.stop(STOP_TIMEOUT));
        // Le second n'a rien créé.
        assert!(!dir_b.join("corde.db").exists());

        assert!(a.stop(STOP_TIMEOUT));
        std::fs::remove_dir_all(&dir_a).ok();
    }

    #[test]
    fn no_free_port_is_reported() {
        let (first, _) = free_range();
        let _squatters: Vec<TcpListener> = (first..=first + 1)
            .map(|p| TcpListener::bind((Ipv4Addr::LOCALHOST, p)).unwrap())
            .collect();
        let opts = sim_options("full", first, first + 1);
        let (mut server, started) = start(opts).unwrap();
        let err = started.recv_timeout(START_TIMEOUT).unwrap().unwrap_err();
        assert_eq!(
            err,
            StartError::NoFreePort {
                first,
                last: first + 1
            }
        );
        assert!(server.stop(STOP_TIMEOUT));
    }
}
