//! Mises à jour de l'application (`tauri-plugin-updater`) : vérification, téléchargement,
//! installation, vérification automatique et notification.
//!
//! La logique sans Tauri (machine d'états, refus, comparaison de versions, calendrier,
//! anti-doublon des notifications) est dans [`crate::updates`] ; ce module la relie au plugin,
//! à l'interface (événement [`updates::EVENT`]) et au reste de la coquille.
//!
//! ```text
//! check_update / vérification automatique (30 s après le lancement, puis toutes les 6 h)
//!    └─ plugin : GET https://github.com/dultra123/corde/releases/latest/download/latest.json
//!         ├─ version plus récente → « disponible » (+ notification Windows, une fois, hors jeu)
//!         └─ à jour / erreur      → affiché tel quel
//! install_update (jamais si le jeu est détecté ou qu'un enregistrement tourne)
//!    └─ téléchargement de Corde_<version>_x64-setup.exe, signature vérifiée (clé publique
//!       de tauri.conf.json) → nouveau contrôle du jeu → `Update::install`, qui sous Windows :
//!         1. écrit l'installeur dans un dossier temporaire ;
//!         2. appelle notre `on_before_exit` : arrêt propre du serveur embarqué (l'enregistrement
//!            en cours est clos), position de la fenêtre enregistrée, nettoyage Tauri ;
//!         3. lance l'installeur NSIS (`/P /UPDATE /R /ARGS …` : barre de progression seule,
//!            pas de question ; il relance Corde à la fin) ;
//!         4. termine le processus par `std::process::exit(0)` — sans `RunEvent::Exit`, d'où
//!            l'arrêt du serveur à l'étape 2.
//! ```

use crate::server;
use crate::settings;
use crate::shell::{self, Boot, Shell};
use crate::updates::{self, Activity, Available, ErrorKind, Machine, Phase, Snapshot};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};
use tauri_plugin_window_state::AppHandleExt as _;

/// Délai maximal de la vérification (lecture de `latest.json`).
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// Délai maximal du téléchargement de l'installeur (quelques dizaines de Mo).
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);
/// Période du thread de vérification automatique.
const SCHEDULER_TICK: Duration = Duration::from_secs(15);
/// Délai des requêtes vers le serveur embarqué (état du jeu).
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);

/// Qui a demandé la vérification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Bouton « Rechercher une mise à jour ».
    Manual,
    /// Vérification automatique (30 s après le lancement, puis toutes les 6 h).
    Auto,
}

/// État des mises à jour (`app.state::<Updates>()`).
pub struct Updates {
    data_dir: PathBuf,
    machine: Mutex<Machine>,
    /// Mise à jour trouvée par la dernière vérification (objet du plugin, porte l'adresse et
    /// la signature).
    update: Mutex<Option<Update>>,
    /// Installeur déjà téléchargé et vérifié `(version, octets)` : si l'installation a été
    /// refusée après le téléchargement (jeu lancé entre-temps), le clic suivant ne
    /// retélécharge pas.
    downloaded: Mutex<Option<(String, Vec<u8>)>>,
    started: Instant,
    last_check: Mutex<Option<Instant>>,
    /// L'installeur est sur le point d'être lancé : le serveur embarqué est arrêté.
    exiting: AtomicBool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Updates {
    /// `updated_from` : version d'avant, si ce lancement suit une mise à jour.
    pub fn new(data_dir: PathBuf, current_version: &str, updated_from: Option<String>) -> Self {
        let auto = settings::update_auto_check(&data_dir);
        Self {
            machine: Mutex::new(Machine::new(current_version, auto, updated_from)),
            data_dir,
            update: Mutex::new(None),
            downloaded: Mutex::new(None),
            started: Instant::now(),
            last_check: Mutex::new(None),
            exiting: AtomicBool::new(false),
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        lock(&self.machine).snapshot()
    }

    pub fn auto_check(&self) -> bool {
        lock(&self.machine).snapshot().auto_check
    }

    pub fn set_auto_check(&self, enabled: bool) -> std::io::Result<()> {
        settings::set_update_auto_check(&self.data_dir, enabled)?;
        lock(&self.machine).set_auto_check(enabled);
        Ok(())
    }
}

/// Envoie l'état courant à l'interface.
fn emit(app: &AppHandle) {
    let snapshot = app.state::<Updates>().snapshot();
    if let Err(e) = app.emit(updates::EVENT, snapshot) {
        tracing::debug!("mises à jour : événement non envoyé ({e})");
    }
}

/// Texte d'erreur du plugin, sans chemin personnel.
fn clean<E: std::fmt::Display>(error: E) -> String {
    updates::redact(&error.to_string(), &updates::personal_values())
}

/// Ce que fait le pilote, lu à l'instant dans `GET /api/status` (la même lecture que le
/// veilleur) ; à défaut, la dernière lecture du veilleur. `None` si le serveur n'a jamais
/// répondu : l'installation est alors refusée.
fn current_activity(app: &AppHandle) -> Option<Activity> {
    let shell = app.state::<Shell>();
    let fresh = match shell.boot() {
        Boot::Ready { port } => server::game_status(&server::local_agent(STATUS_TIMEOUT), port),
        _ => None,
    };
    fresh
        .or_else(|| shell.last_game_status())
        .map(|s| Activity {
            game_running: s.connected || s.in_realtime,
            recording: s.recording,
        })
}

/// Le constructeur du plugin, avec nos réglages : délai, comparaison de versions de
/// [`updates::is_newer`], et `on_before_exit` qui arrête Corde proprement avant l'installeur.
fn updater(app: &AppHandle) -> tauri_plugin_updater::Result<tauri_plugin_updater::Updater> {
    let handle = app.clone();
    app.updater_builder()
        .timeout(CHECK_TIMEOUT)
        .version_comparator(|current, release| {
            updates::is_newer(&release.version.to_string(), &current.to_string())
        })
        .on_before_exit(move || before_exit(&handle))
        .build()
}

/// Vérifie s'il existe une version plus récente et met l'état à jour. Ne fait rien (et
/// renvoie l'état courant) si une vérification ou une installation est déjà en cours.
pub async fn check(app: &AppHandle, origin: Origin) -> Snapshot {
    {
        let state = app.state::<Updates>();
        let begun = lock(&state.machine).begin_check();
        if begun.is_err() {
            return state.snapshot();
        }
    }
    emit(app);
    tracing::info!(
        "mises à jour : vérification {} (version installée {})",
        match origin {
            Origin::Manual => "demandée",
            Origin::Auto => "automatique",
        },
        app.package_info().version
    );

    let result = match updater(app) {
        Ok(updater) => updater.check().await,
        Err(e) => Err(e),
    };

    let state = app.state::<Updates>();
    *lock(&state.last_check) = Some(Instant::now());
    let outcome = match result {
        Ok(Some(update)) => {
            let available = Available {
                version: update.version.clone(),
                date: update
                    .raw_json
                    .get("pub_date")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string),
                notes: update.body.clone().filter(|n| !n.trim().is_empty()),
            };
            tracing::info!(
                "mises à jour : version {} disponible (publiée le {})",
                available.version,
                available.date.as_deref().unwrap_or("?")
            );
            let version = available.version.clone();
            *lock(&state.update) = Some(update);
            // Vue dans l'interface : pas de notification pour elle ensuite.
            if origin == Origin::Manual {
                remember_notified(&state, &version);
            }
            Ok(Some(available))
        }
        Ok(None) => {
            tracing::info!("mises à jour : Corde est à jour");
            *lock(&state.update) = None;
            Ok(None)
        }
        Err(e) => {
            let detail = clean(e);
            tracing::warn!("mises à jour : vérification impossible ({detail})");
            Err(detail)
        }
    };
    lock(&state.machine).finish_check(SystemTime::now(), outcome);
    emit(app);
    app.state::<Updates>().snapshot()
}

fn remember_notified(state: &Updates, version: &str) {
    if let Err(e) = settings::set_update_notified_version(&state.data_dir, version) {
        tracing::warn!("mises à jour : version annoncée non mémorisée ({e})");
    }
}

/// Télécharge la mise à jour connue, la vérifie, puis lance l'installeur — ce qui termine
/// Corde. Refusé (avec un message clair) si le jeu est détecté, si un enregistrement est en
/// cours, si l'état du jeu est inconnu, ou si aucune mise à jour n'est connue.
pub async fn install(app: &AppHandle) -> Result<(), String> {
    let activity = {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || current_activity(&handle))
            .await
            .unwrap_or(None)
    };
    let begun = lock(&app.state::<Updates>().machine).begin_install(activity);
    if let Err(kind) = begun {
        let message = updates::refusal_message(kind);
        tracing::info!("mises à jour : installation refusée — {message}");
        emit(app);
        return Err(message.to_string());
    }
    let update = lock(&app.state::<Updates>().update).clone();
    let Some(mut update) = update else {
        // Ne devrait pas arriver : `available` et `update` sont écrits ensemble.
        let detail = "mise à jour introuvable, relance la recherche".to_string();
        lock(&app.state::<Updates>().machine).download_failed(detail.clone());
        emit(app);
        return Err(detail);
    };
    emit(app);

    // 1. Téléchargement (sauf si cette version est déjà en mémoire), signature vérifiée.
    let cached = lock(&app.state::<Updates>().downloaded)
        .as_ref()
        .filter(|(version, _)| *version == update.version)
        .map(|(_, bytes)| bytes.clone());
    let bytes = match cached {
        Some(bytes) => {
            tracing::info!(
                "mises à jour : installeur {} déjà téléchargé ({} octets)",
                update.version,
                bytes.len()
            );
            bytes
        }
        None => {
            tracing::info!(
                "mises à jour : téléchargement de {} ({})",
                update.version,
                update.download_url
            );
            update.timeout = Some(DOWNLOAD_TIMEOUT);
            let t0 = Instant::now();
            let progress_app = app.clone();
            let result = update
                .download(
                    move |chunk, total| {
                        let state = progress_app.state::<Updates>();
                        let worth = lock(&state.machine).progress(chunk as u64, total);
                        if worth {
                            emit(&progress_app);
                        }
                    },
                    || {},
                )
                .await;
            match result {
                Ok(bytes) => {
                    tracing::info!(
                        "mises à jour : {} octets téléchargés en {} s, signature vérifiée",
                        bytes.len(),
                        t0.elapsed().as_secs()
                    );
                    *lock(&app.state::<Updates>().downloaded) =
                        Some((update.version.clone(), bytes.clone()));
                    bytes
                }
                Err(e) => {
                    let detail = clean(e);
                    tracing::error!("mises à jour : téléchargement impossible ({detail})");
                    lock(&app.state::<Updates>().machine).download_failed(detail.clone());
                    emit(app);
                    return Err(format!(
                        "{} {detail}",
                        updates::refusal_message(ErrorKind::Download)
                    ));
                }
            }
        }
    };

    // 2. Le jeu a pu démarrer pendant le téléchargement : on vérifie de nouveau.
    let activity = {
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || current_activity(&handle))
            .await
            .unwrap_or(None)
    };
    if let Some(kind) = updates::install_blocker(activity) {
        let message = updates::refusal_message(kind);
        tracing::info!("mises à jour : installation reportée — {message}");
        lock(&app.state::<Updates>().machine).blocked_after_download(kind);
        emit(app);
        return Err(message.to_string());
    }

    // 3. Installation : l'installeur est lancé et Corde s'arrête (sous Windows, `install` ne
    //    revient qu'en cas d'échec).
    lock(&app.state::<Updates>().machine).begin_installing();
    emit(app);
    let from = app.package_info().version.to_string();
    let to = update.version.clone();
    tracing::info!("mises à jour : installation de {to} (depuis {from})");
    let data_dir = app.state::<Updates>().data_dir.clone();
    if let Err(e) = settings::set_pending_update(&data_dir, &from, &to) {
        tracing::warn!("mises à jour : impossible de noter l'installation en cours ({e})");
    }
    let installed =
        tauri::async_runtime::spawn_blocking(move || update.install(bytes).map_err(clean)).await;

    // Ici seulement si l'installeur n'a pas été lancé (ou hors Windows).
    let detail = match installed {
        Ok(Ok(())) => {
            // macOS / Linux : le plugin installe sans quitter. Corde n'est livrée que pour
            // Windows ; on redémarre pour être sur la nouvelle version.
            tracing::info!("mises à jour : installée, redémarrage");
            shell::request_restart(app);
            return Ok(());
        }
        Ok(Err(detail)) => detail,
        Err(e) => clean(e),
    };
    tracing::error!("mises à jour : l'installeur n'a pas pu être lancé ({detail})");
    let _ = settings::clear_pending_update(&data_dir);
    let state = app.state::<Updates>();
    if state.exiting.load(Ordering::SeqCst) {
        // Le serveur embarqué est déjà arrêté et Tauri nettoyé : on repart de zéro.
        tracing::warn!("mises à jour : Corde redémarre sans la mise à jour");
        shell::relaunch_and_exit(app);
    }
    lock(&state.machine).install_failed(detail.clone());
    emit(app);
    Err(format!(
        "{} {detail}",
        updates::refusal_message(ErrorKind::Install)
    ))
}

/// Appelé par le plugin juste avant de lancer l'installeur et de terminer le processus
/// (thread de l'installation, pas le thread principal).
fn before_exit(app: &AppHandle) {
    app.state::<Updates>().exiting.store(true, Ordering::SeqCst);
    tracing::info!("mises à jour : arrêt de Corde pour laisser la place à l'installeur");
    if let Err(e) = app.save_window_state(shell::window_state_flags()) {
        tracing::debug!("position de la fenêtre non enregistrée : {e}");
    }
    shell::stop_for_update(app);
    tracing::info!("mises à jour : lancement de l'installeur, Corde s'arrête");
    // Ce que fait le plugin par défaut (remplacé par ce rappel) : icône de la zone de
    // notification retirée, fenêtres cachées.
    app.cleanup_before_exit();
}

/// Lance le thread des vérifications automatiques (et des notifications).
pub fn spawn_scheduler(app: &AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("corde-updates".into())
        .spawn(move || loop {
            if app.state::<Shell>().is_quitting() {
                break;
            }
            let due = {
                let state = app.state::<Updates>();
                let since_last = lock(&state.last_check).map(|t| t.elapsed());
                updates::auto_check_due(state.auto_check(), state.started.elapsed(), since_last)
            };
            if due {
                tauri::async_runtime::block_on(check(&app, Origin::Auto));
            }
            maybe_notify(&app);
            std::thread::sleep(SCHEDULER_TICK);
        });
    if let Err(e) = spawned {
        tracing::error!("mises à jour : vérification automatique impossible à lancer ({e})");
    }
}

/// Notification Windows « Corde X est disponible » : une fois par version, jamais pendant
/// que le jeu tourne (elle attend sa fermeture), jamais quand l'interface l'a déjà montrée.
fn maybe_notify(app: &AppHandle) {
    let state = app.state::<Updates>();
    let version = {
        let machine = lock(&state.machine);
        if machine.phase() != Phase::Idle {
            return;
        }
        match machine.available() {
            Some(a) => a.version.clone(),
            None => return,
        }
    };
    let notified = settings::update_notified_version(&state.data_dir);
    let game_running = app
        .state::<Shell>()
        .last_game_status()
        .is_some_and(|s| s.connected || s.in_realtime);
    if !updates::should_notify(&version, notified.as_deref(), game_running) {
        return;
    }
    tracing::info!("mises à jour : notification de la version {version}");
    shell::notify(
        app,
        &format!(
            "Corde {version} est disponible. Ouvre Corde → Réglages → Mises à jour pour \
             l'installer."
        ),
    );
    remember_notified(&state, &version);
}

/// Ouvre la page des versions dans le navigateur par défaut (adresse fixe).
pub fn open_releases_page() -> Result<(), String> {
    #[cfg(windows)]
    let mut command = {
        // `url.dll,FileProtocolHandler` : le navigateur par défaut, sans passer par un
        // interpréteur de commandes (rien à échapper).
        let mut c = std::process::Command::new("rundll32.exe");
        c.args(["url.dll,FileProtocolHandler", updates::RELEASES_PAGE]);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(updates::RELEASES_PAGE);
        c
    };
    #[cfg(not(any(windows, target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(updates::RELEASES_PAGE);
        c
    };
    match command.spawn() {
        Ok(_) => {
            tracing::info!("page des versions ouverte : {}", updates::RELEASES_PAGE);
            Ok(())
        }
        Err(e) => {
            tracing::warn!("page des versions : navigateur introuvable ({e})");
            Err(format!(
                "Le navigateur n'a pas pu être ouvert. Adresse : {}",
                updates::RELEASES_PAGE
            ))
        }
    }
}
