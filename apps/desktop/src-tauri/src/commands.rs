//! Commandes IPC de la coquille.
//!
//! **Contrat avec l'UI web** (`packages/ui`, servie par le serveur embarqué) : elle appelle
//! exactement ces quatre commandes par `window.__TAURI__.core.invoke(...)` :
//!
//! | Commande | Arguments | Retour |
//! |---|---|---|
//! | `get_autostart` | — | `boolean` : Corde démarre-t-il avec Windows ? |
//! | `set_autostart` | `{ enabled: boolean }` | — |
//! | `restart_app` | — | — (l'application s'arrête puis se relance) |
//! | `app_version` | — | `string` (ex. `"0.3.0"`) |
//!
//! `boot_status` ne sert qu'à la page locale de démarrage (`apps/desktop/dist`).
//! Les droits sont dans `capabilities/*.json` ; la liste des commandes dans `build.rs`.
//! Les erreurs sont renvoyées sous forme de texte (promesse rejetée côté JavaScript).

use crate::shell::{self, Boot, Shell};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt as _;

/// Corde est-elle inscrite au démarrage de Windows ?
#[tauri::command]
pub async fn get_autostart(app: AppHandle) -> Result<bool, String> {
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

/// Inscrit (ou retire) Corde du démarrage de Windows. L'entrée lance l'application avec
/// `--hidden` : pas de fenêtre, seulement l'icône de la zone de notification.
#[tauri::command]
pub async fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let result = if enabled {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    match result {
        Ok(()) => {
            tracing::info!(
                "démarrage avec Windows : {}",
                if enabled { "activé" } else { "désactivé" }
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!("démarrage avec Windows : échec ({e})");
            Err(e.to_string())
        }
    }
}

/// Arrête proprement Corde (serveur, enregistrement) puis le relance, fenêtre visible.
/// Sert à appliquer un réglage lu au démarrage (accès depuis le réseau local).
#[tauri::command]
pub async fn restart_app(app: AppHandle) -> Result<(), String> {
    tracing::info!("redémarrage demandé par l'interface");
    shell::request_restart(&app);
    Ok(())
}

/// Version de l'application (celle de `tauri.conf.json`).
#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// État du démarrage, pour la page locale.
#[derive(Debug, Serialize)]
pub struct BootStatus {
    /// `"starting"`, `"ready"` ou `"failed"`.
    phase: &'static str,
    /// Message d'erreur lisible quand `phase = "failed"`.
    message: Option<String>,
    /// Port du serveur embarqué quand `phase = "ready"`.
    port: Option<u16>,
    /// Chemin du journal à communiquer en cas de problème.
    log_path: String,
    version: String,
}

#[tauri::command]
pub fn boot_status(app: AppHandle) -> BootStatus {
    let shell = app.state::<Shell>();
    let (phase, message, port) = match shell.boot() {
        Boot::Starting => ("starting", None, None),
        Boot::Ready { port } => ("ready", None, Some(port)),
        Boot::Failed { message } => ("failed", Some(message), None),
    };
    BootStatus {
        phase,
        message,
        port,
        log_path: shell.log_path.display().to_string(),
        version: app.package_info().version.to_string(),
    }
}
