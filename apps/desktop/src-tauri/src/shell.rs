//! État de la coquille et pilotage de la fenêtre principale.
//!
//! ```text
//! lancement ─▶ fenêtre « main » sur la page locale « Démarrage de Corde… » (cachée si --hidden)
//!           ─▶ thread « corde-boot » : attend le serveur embarqué puis /api/health (20 s max)
//!                 ├─ prêt   ─▶ Boot::Ready { port } ─▶ l'UI est chargée dès que la fenêtre est à l'écran
//!                 └─ échec  ─▶ Boot::Failed { message } ─▶ la page locale affiche l'erreur
//! ```
//!
//! Règle de conception : **tout ce qui touche à la fenêtre s'exécute sur le thread
//! principal** (`AppHandle::run_on_main_thread`). Les autres threads (démarrage, veilleur)
//! ne font que poster une tâche. Les décisions « l'UI est-elle chargée ? la fenêtre
//! est-elle visible ? » sont ainsi prises en série, sans verrou tenu pendant un appel à la
//! fenêtre.

use crate::server::{self, EmbeddedServer, StartError, Started};
use crate::settings;
use crate::tray;
use crate::watcher::{self, Watcher};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;
use tauri::{AppHandle, Manager, WebviewWindow};
use tauri_plugin_notification::NotificationExt;

/// Label de la fenêtre principale (tauri.conf.json, capabilities/*.json).
pub const MAIN_WINDOW: &str = "main";
/// Argument passé par le démarrage automatique de Windows : démarrer sans fenêtre.
pub const ARG_HIDDEN: &str = "--hidden";

/// Où en est le démarrage du serveur embarqué.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Boot {
    Starting,
    Ready { port: u16 },
    Failed { message: String },
}

/// Pages de l'UI web que la coquille sait ouvrir (routes de `packages/ui/src/nav/routes.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Écran d'accueil.
    Home,
    /// Tableau de bord en direct (route par défaut de l'UI).
    Dashboard,
}

impl Route {
    pub fn hash(self) -> &'static str {
        match self {
            Route::Home => "#/home",
            Route::Dashboard => "#/",
        }
    }
}

/// Pourquoi on montre la fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reveal {
    /// Icône du bureau (second lancement), clic sur la zone de notification, « Ouvrir Corde ».
    Open,
    /// Menu « Dashboard » de la zone de notification.
    Dashboard,
    /// Le pilote vient d'entrer en piste (veilleur).
    TrackEntry,
}

/// Chargement de l'UI web dans la fenêtre (modifié sur le thread principal uniquement).
#[derive(Debug)]
struct Ui {
    /// La fenêtre a quitté la page locale pour l'UI servie par le serveur embarqué.
    loaded: bool,
    /// Page à ouvrir au premier chargement.
    first_route: Route,
}

/// État partagé de la coquille (`app.state::<Shell>()`).
pub struct Shell {
    pub data_dir: PathBuf,
    pub log_path: PathBuf,
    /// Lancée avec `--hidden` (démarrage avec Windows).
    pub started_hidden: bool,
    boot: Mutex<Boot>,
    ui: Mutex<Ui>,
    quitting: AtomicBool,
    restart_requested: AtomicBool,
    tray_hint_shown: AtomicBool,
    server: Mutex<Option<EmbeddedServer>>,
    watcher: Mutex<Option<Watcher>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Shell {
    pub fn new(data_dir: PathBuf, log_path: PathBuf, started_hidden: bool) -> Self {
        let tray_hint_shown = settings::tray_hint_shown(&data_dir);
        Self {
            data_dir,
            log_path,
            started_hidden,
            boot: Mutex::new(Boot::Starting),
            ui: Mutex::new(Ui {
                loaded: false,
                first_route: Route::Home,
            }),
            quitting: AtomicBool::new(false),
            restart_requested: AtomicBool::new(false),
            tray_hint_shown: AtomicBool::new(tray_hint_shown),
            server: Mutex::new(None),
            watcher: Mutex::new(None),
        }
    }

    pub fn boot(&self) -> Boot {
        lock(&self.boot).clone()
    }

    /// Fermer la fenêtre la cache (Corde reste dans la zone de notification), sauf si on
    /// est en train de quitter ou si le serveur n'a pas démarré (rien à garder en vie).
    pub fn hide_on_close(&self) -> bool {
        !self.quitting.load(Ordering::Relaxed) && !matches!(self.boot(), Boot::Failed { .. })
    }

    pub fn set_server(&self, server: EmbeddedServer) {
        *lock(&self.server) = Some(server);
    }
}

pub fn main_window(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(MAIN_WINDOW)
}

/// Adresse de l'UI dans la fenêtre : toujours la boucle locale, quel que soit `lan_access`.
pub fn ui_url(port: u16, route: Route) -> String {
    server::local_url(port, &format!("/{}", route.hash()))
}

fn notify(app: &AppHandle, body: &str) {
    if let Err(e) = app
        .notification()
        .builder()
        .title("Corde")
        .body(body)
        .show()
    {
        tracing::warn!("notification impossible : {e}");
    }
}

/// Exécute `task` sur le thread principal (tout de suite si on y est déjà).
pub fn on_main(app: &AppHandle, task: impl FnOnce(&AppHandle) + Send + 'static) {
    let handle = app.clone();
    if let Err(e) = app.run_on_main_thread(move || task(&handle)) {
        tracing::warn!("tâche sur le thread principal refusée : {e}");
    }
}

/// Thread principal. Charge l'UI dans la fenêtre si le serveur est prêt et que ce n'est
/// pas déjà fait. Retourne `true` si l'UI est (maintenant) chargée.
fn ensure_ui_loaded(app: &AppHandle, window: &WebviewWindow) -> bool {
    let shell = app.state::<Shell>();
    let Boot::Ready { port } = shell.boot() else {
        return false;
    };
    let mut ui = lock(&shell.ui);
    if ui.loaded {
        return true;
    }
    let url = ui_url(port, ui.first_route);
    match url.parse::<tauri::Url>() {
        Ok(parsed) => match window.navigate(parsed) {
            Ok(()) => {
                tracing::info!("interface chargée : {url}");
                ui.loaded = true;
            }
            Err(e) => tracing::error!("navigation vers {url} impossible : {e}"),
        },
        Err(e) => tracing::error!("adresse d'interface invalide {url} : {e}"),
    }
    ui.loaded
}

/// Montre la fenêtre principale (et la met au premier plan, sauf à l'entrée en piste).
/// Appelable de n'importe quel
/// thread ; le travail se fait sur le thread principal.
pub fn reveal(app: &AppHandle, why: Reveal) {
    on_main(app, move |app| reveal_now(app, why));
}

fn reveal_now(app: &AppHandle, why: Reveal) {
    let shell = app.state::<Shell>();
    let Some(window) = main_window(app) else {
        tracing::warn!("fenêtre principale introuvable ({why:?})");
        return;
    };

    if why == Reveal::TrackEntry {
        // Entrée en piste : on n'amène la fenêtre que si elle n'est pas déjà à l'écran.
        // Visible (sur un second écran, ou derrière le jeu) → on ne touche à rien. Cachée ou
        // réduite → on la ressort (voir plus bas) **sans jamais demander le premier plan** :
        // le pilote est en train de quitter les stands, le jeu doit garder le clavier et
        // rester devant.
        let on_screen =
            window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false);
        if on_screen {
            tracing::info!("entrée en piste : fenêtre déjà à l'écran");
            return;
        }
        tracing::info!("entrée en piste : affichage de la fenêtre");
    }

    // Contenu : au premier chargement, la page dépend de la raison ; ensuite l'UI gère
    // seule sa navigation, sauf demande explicite du menu « Dashboard ».
    let was_loaded = {
        let mut ui = lock(&shell.ui);
        if !ui.loaded && why != Reveal::Open {
            ui.first_route = Route::Dashboard;
        }
        ui.loaded
    };
    let loaded = ensure_ui_loaded(app, &window);
    if loaded && was_loaded && why == Reveal::Dashboard {
        let script = format!("window.location.hash = {:?};", Route::Dashboard.hash());
        if let Err(e) = window.eval(script) {
            tracing::warn!("changement de page impossible : {e}");
        }
    }

    let mut steps = vec![("show", window.show()), ("unminimize", window.unminimize())];
    // `set_focus` force le premier plan sous Windows (il contourne le verrou de premier plan
    // du système) : réservé aux demandes explicites de l'utilisateur (icône, zone de
    // notification). À l'entrée en piste, Windows laisse le jeu devant : la fenêtre réapparaît
    // sur le second écran, ou derrière le jeu s'il n'y a qu'un écran.
    if why != Reveal::TrackEntry {
        steps.push(("set_focus", window.set_focus()));
    }
    for (step, result) in steps {
        if let Err(e) = result {
            tracing::warn!("fenêtre : {step} a échoué : {e}");
        }
    }
}

/// Fermeture de la fenêtre par l'utilisateur : la cacher, et dire une fois que Corde reste
/// actif. Thread principal (gestionnaire d'événements de fenêtre).
pub fn hide_to_tray(app: &AppHandle) {
    let shell = app.state::<Shell>();
    if let Some(window) = main_window(app) {
        if let Err(e) = window.hide() {
            tracing::warn!("fenêtre : hide a échoué : {e}");
        }
    }
    if !shell.tray_hint_shown.swap(true, Ordering::Relaxed) {
        notify(
            app,
            "Corde reste active dans la zone de notification : elle attend le jeu et enregistre \
             tes sessions. Clic droit sur l'icône → Quitter pour l'arrêter.",
        );
        if let Err(e) = settings::set_tray_hint_shown(&shell.data_dir) {
            tracing::warn!("impossible de mémoriser l'info-bulle : {e}");
        }
    }
}

/// Lance le thread qui attend le serveur embarqué, puis passe à `Ready` ou `Failed`.
pub fn spawn_boot(app: &AppHandle, started: Started) {
    let thread_app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("corde-boot".into())
        .spawn(move || {
            let app = thread_app;
            let t0 = Instant::now();
            let result = match started.recv_timeout(server::START_TIMEOUT) {
                Ok(Ok(addr)) => {
                    let port = addr.port();
                    let left = server::START_TIMEOUT.saturating_sub(t0.elapsed());
                    if server::wait_healthy(port, left) {
                        Ok(port)
                    } else {
                        Err(StartError::Timeout { port: Some(port) })
                    }
                }
                Ok(Err(e)) => Err(e),
                Err(RecvTimeoutError::Timeout) => Err(StartError::Timeout { port: None }),
                Err(RecvTimeoutError::Disconnected) => Err(StartError::Runtime(
                    "le thread du serveur s'est terminé sans rien dire".into(),
                )),
            };
            match result {
                Ok(port) => {
                    tracing::info!(
                        "serveur embarqué prêt sur le port {port} ({} ms)",
                        t0.elapsed().as_millis()
                    );
                    on_ready(&app, port);
                }
                Err(e) => on_failed(&app, e),
            }
        });
    if let Err(e) = spawned {
        on_failed(
            app,
            StartError::Runtime(format!("thread de démarrage impossible : {e}")),
        );
    }
}

/// Le serveur embarqué n'a même pas pu être lancé (thread refusé par le système).
pub fn fail_boot(app: &AppHandle, error: StartError) {
    on_failed(app, error);
}

fn on_ready(app: &AppHandle, port: u16) {
    let shell = app.state::<Shell>();
    *lock(&shell.boot) = Boot::Ready { port };
    *lock(&shell.watcher) = watcher::spawn(app, port);
    tray::set_tooltip(app, tray::TOOLTIP_WAITING);
    on_main(app, |app| {
        // Fenêtre déjà à l'écran (lancement normal, ou ouverte pendant le démarrage) :
        // l'UI est chargée tout de suite. Démarrage caché : au premier affichage, pour ne
        // pas faire tourner le tableau de bord dans une fenêtre que personne ne voit.
        if let Some(window) = main_window(app) {
            if window.is_visible().unwrap_or(false) {
                ensure_ui_loaded(app, &window);
            }
        }
    });
}

fn on_failed(app: &AppHandle, error: StartError) {
    let shell = app.state::<Shell>();
    let message = error.to_string();
    tracing::error!("démarrage impossible : {message}");
    *lock(&shell.boot) = Boot::Failed { message };
    // Si le serveur écoute mais ne répond pas, on ne le laisse pas tourner à moitié.
    // (Le verrou n'est pas tenu pendant l'arrêt, qui peut durer quelques secondes.)
    let server = lock(&shell.server).take();
    if let Some(mut server) = server {
        server.stop(server::STOP_TIMEOUT);
    }
    tray::set_tooltip(app, tray::TOOLTIP_FAILED);
    on_main(app, |app| {
        let shell = app.state::<Shell>();
        let Some(window) = main_window(app) else {
            return;
        };
        if shell.started_hidden && !window.is_visible().unwrap_or(false) {
            // Démarré avec Windows : pas de fenêtre qui surgit, une notification suffit ;
            // la page d'erreur attend l'ouverture depuis la zone de notification.
            notify(
                app,
                "Corde n'a pas pu démarrer. Clique sur l'icône Corde près de l'horloge pour \
                 voir le détail.",
            );
        } else {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    });
}

/// « Quitter » : cache la fenêtre tout de suite, puis arrêt complet (voir [`shutdown`]).
pub fn quit(app: &AppHandle) {
    let shell = app.state::<Shell>();
    shell.quitting.store(true, Ordering::Relaxed);
    if let Some(window) = main_window(app) {
        let _ = window.hide();
    }
    app.exit(0);
}

/// Commande `restart_app` : arrêt propre puis relance, sans `--hidden` (la fenêtre revient).
pub fn request_restart(app: &AppHandle) {
    let shell = app.state::<Shell>();
    shell.restart_requested.store(true, Ordering::Relaxed);
    shell.quitting.store(true, Ordering::Relaxed);
    app.exit(0);
}

/// Dernière étape avant la fin du processus (`RunEvent::Exit`, thread principal) : arrête
/// le veilleur puis le serveur embarqué — ce qui ferme l'enregistrement en cours — et
/// relance l'application si un redémarrage a été demandé.
pub fn shutdown(app: &AppHandle) {
    let shell = app.state::<Shell>();
    tracing::info!("arrêt de Corde…");
    if let Some(watcher) = lock(&shell.watcher).take() {
        watcher.stop();
    }
    let server = lock(&shell.server).take();
    if let Some(mut server) = server {
        let t0 = Instant::now();
        let clean = server.stop(server::STOP_TIMEOUT);
        tracing::info!(
            "serveur embarqué arrêté en {} ms (propre : {clean})",
            t0.elapsed().as_millis()
        );
    }
    if shell.restart_requested.load(Ordering::Relaxed) {
        relaunch(app);
    }
    tracing::info!("Corde arrêtée");
}

/// Relance l'exécutable sans argument. Le verrou d'instance unique est libéré d'abord,
/// sinon le nouveau processus se prendrait pour un second lancement et s'arrêterait.
fn relaunch(app: &AppHandle) {
    tauri_plugin_single_instance::destroy(app);
    match tauri::process::current_binary(&app.env()) {
        Ok(exe) => match std::process::Command::new(&exe).spawn() {
            Ok(child) => tracing::info!("redémarrage : {} (pid {})", exe.display(), child.id()),
            Err(e) => tracing::error!("redémarrage impossible ({}) : {e}", exe.display()),
        },
        Err(e) => tracing::error!("exécutable introuvable pour le redémarrage : {e}"),
    }
}
