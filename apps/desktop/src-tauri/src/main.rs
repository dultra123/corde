//! Corde — application de bureau (coquille Tauri 2).
//!
//! Une icône, une fenêtre, pas de console : la coquille démarre le serveur Corde embarqué
//! (le même code que `corde-server.exe`), affiche l'UI web qu'il sert sur la boucle locale,
//! reste dans la zone de notification quand on ferme la fenêtre, et ressort la fenêtre
//! (sans prendre le premier plan au jeu) quand le pilote entre en piste.
//!
//! | Module | Rôle |
//! |---|---|
//! | [`logging`] | journal `<données>/logs/corde-desktop.log`, paniques |
//! | [`migration`] | reprise des dossiers de l'ancien nom (données, profil WebView), avant toute ouverture |
//! | [`settings`] | lecture de `settings.json` (`lan_access`) et de `desktop.json` |
//! | [`server`] | serveur embarqué : thread + runtime tokio, port 8471…8480, arrêt propre |
//! | [`shell`] | état de la coquille, fenêtre principale, démarrage, arrêt, redémarrage |
//! | [`tray`] | icône et menu de la zone de notification |
//! | [`watcher`] + [`edges`] | veilleur 1 Hz sur `/api/status` |
//! | [`commands`] | commandes IPC appelées par l'UI (contrat documenté dans le module) |
//!
//! Voir `apps/desktop/README.md`.

// Pas de console sous Windows dans un build release (elle reste en `cargo run` de dev).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod edges;
mod logging;
mod migration;
mod server;
mod settings;
mod shell;
mod tray;
mod watcher;

use corde_server::SourceKind;
use server::StartError;
use shell::{Reveal, Shell};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_window_state::StateFlags;

/// Variable d'environnement de développement : `sim` remplace le jeu par le simulateur.
const SOURCE_ENV: &str = "CORDE_SOURCE";

fn has_hidden_arg<S: AsRef<str>>(args: &[S]) -> bool {
    args.iter().skip(1).any(|a| a.as_ref() == shell::ARG_HIDDEN)
}

fn source_from_env() -> SourceKind {
    match corde_store::migrate::env_var(SOURCE_ENV).ok().as_deref() {
        Some("sim") => SourceKind::Sim,
        Some("lmu") => SourceKind::Lmu,
        _ => SourceKind::Auto,
    }
}

fn main() {
    // Même dossier que le serveur : %LOCALAPPDATA%\Corde, repris de l'ancien nom avant toute
    // ouverture (le journal est dans ce dossier : la migration passe avant, journalisée après).
    let migration = corde_store::prepare_default_data_dir();
    let data_dir = migration.dir.clone();
    let log_path = logging::init(&data_dir);
    let args: Vec<String> = std::env::args().collect();
    let started_hidden = has_hidden_arg(&args);
    tracing::info!(
        "──── Corde {} — lancement (pid {}, caché : {started_hidden}) ────",
        env!("CARGO_PKG_VERSION"),
        std::process::id()
    );
    migration.log();
    // Dossier de données resté sous l'ancien nom (ancienne application encore ouverte ?) :
    // son profil WebView n'est pas touché non plus ; tout sera repris au prochain démarrage.
    if !migration.uses_legacy() {
        migration::migrate_identifier_dirs(&migration::identifier_bases());
    }

    let builder = tauri::Builder::default()
        // Instance unique, EN PREMIER : un second lancement s'arrête dans l'initialisation
        // de ce plugin, après avoir réveillé la première instance (qui exécute ce rappel).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            let hidden = has_hidden_arg(&args);
            tracing::info!("second lancement reçu (caché : {hidden})");
            if !hidden {
                shell::reveal(app, Reveal::Open);
            }
        }))
        // Démarrage avec Windows : clé « Corde » de HKCU\…\Run, avec --hidden.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Corde")
                .arg(shell::ARG_HIDDEN)
                .build(),
        )
        // Taille, position et état agrandi mémorisés ; la visibilité est gérée ici.
        .plugin(
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .manage(Shell::new(data_dir, log_path, started_hidden))
        .invoke_handler(tauri::generate_handler![
            commands::get_autostart,
            commands::set_autostart,
            commands::restart_app,
            commands::app_version,
            commands::boot_status,
        ])
        .on_window_event(|window, event| {
            if window.label() != shell::MAIN_WINDOW {
                return;
            }
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                // Fermer = cacher : Corde continue d'attendre le jeu et d'enregistrer.
                if app.state::<Shell>().hide_on_close() {
                    api.prevent_close();
                    shell::hide_to_tray(app);
                }
            }
        })
        .setup(|app| {
            setup(app.handle());
            Ok(())
        });

    let app = match builder.build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(e) => {
            tracing::error!("initialisation de Tauri impossible : {e}");
            std::process::exit(1);
        }
    };
    app.run(|app, event| {
        if let RunEvent::Exit = event {
            shell::shutdown(app);
        }
    });
}

/// Appelé une fois, la fenêtre principale créée (cachée) et les plugins initialisés.
fn setup(app: &AppHandle) {
    let shell = app.state::<Shell>();

    let tray_ok = match tray::create(app) {
        Ok(()) => true,
        Err(e) => {
            tracing::error!("zone de notification indisponible : {e}");
            false
        }
    };

    // L'UI web construite est une ressource de l'installeur : <dossier d'installation>\ui.
    let ui_dir = app
        .path()
        .resource_dir()
        .ok()
        .map(|dir| dir.join("ui"))
        .filter(|dir| dir.join("index.html").is_file());
    if ui_dir.is_none() {
        tracing::warn!("ressource ui/ absente : recherche par défaut du serveur");
    }
    let bind_ip = settings::resolve_bind_ip(&shell.data_dir);
    let mut options = server::Options::new(shell.data_dir.clone(), ui_dir, bind_ip);
    options.source = source_from_env();
    tracing::info!(
        "serveur embarqué : écoute sur {bind_ip}, source {:?}, données {}",
        options.source,
        shell.data_dir.display()
    );
    match server::start(options) {
        Ok((server, started)) => {
            shell.set_server(server);
            shell::spawn_boot(app, started);
        }
        Err(e) => shell::fail_boot(
            app,
            StartError::Runtime(format!("thread du serveur impossible à créer : {e}")),
        ),
    }

    // Lancement normal : la fenêtre apparaît tout de suite sur « Démarrage de Corde… ».
    // Avec --hidden elle reste cachée — sauf s'il n'y a pas d'icône pour la rouvrir.
    if !shell.started_hidden || !tray_ok {
        if let Some(window) = shell::main_window(app) {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}
