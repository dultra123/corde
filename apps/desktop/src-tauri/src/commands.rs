//! Commandes IPC de la coquille.
//!
//! **Contrat avec l'UI web** (`packages/ui`, servie par le serveur embarqué) : elle appelle
//! exactement ces commandes par `window.__TAURI__.core.invoke(...)` :
//!
//! | Commande | Arguments | Retour |
//! |---|---|---|
//! | `get_autostart` | — | `boolean` : Corde démarre-t-il avec Windows ? |
//! | `set_autostart` | `{ enabled: boolean }` | — |
//! | `restart_app` | — | — (l'application s'arrête puis se relance) |
//! | `app_version` | — | `string` (ex. `"0.6.0"`) |
//! | `update_status` | — | état des mises à jour ([`Snapshot`]) |
//! | `check_update` | — | état après la vérification (un échec de vérification est dans l'état) |
//! | `install_update` | — | — (téléchargement, puis Corde s'arrête et l'installeur la relance) ; rejet avec un message clair si le jeu tourne ou qu'un enregistrement est en cours |
//! | `open_releases_page` | — | — (navigateur par défaut, page fixe des versions) |
//! | `get_update_auto_check` | — | `boolean` (vrai par défaut) |
//! | `set_update_auto_check` | `{ enabled: boolean }` | — |
//!
//! Chaque changement d'état des mises à jour est aussi envoyé par l'événement
//! `update-status` (charge utile : le même état), que l'UI écoute avec
//! `window.__TAURI__.event.listen`.
//!
//! `boot_status` ne sert qu'à la page locale de démarrage (`apps/desktop/dist`).
//! Les droits sont dans `capabilities/*.json` ; la liste des commandes dans `build.rs`.
//! Les erreurs sont renvoyées sous forme de texte (promesse rejetée côté JavaScript).

use crate::shell::{self, Boot, Shell};
use crate::updater::{self, Origin, Updates};
use crate::updates::Snapshot;
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

/// État des mises à jour (version installée, dernière vérification, mise à jour disponible,
/// téléchargement, erreur).
#[tauri::command]
pub fn update_status(app: AppHandle) -> Snapshot {
    app.state::<Updates>().snapshot()
}

/// « Rechercher une mise à jour ». Un échec de la vérification (hors ligne, dépôt
/// inaccessible) n'est pas un rejet : il est décrit dans l'état renvoyé.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> Result<Snapshot, String> {
    Ok(updater::check(&app, Origin::Manual).await)
}

/// « Installer et redémarrer » : refusé si le jeu est détecté ou qu'un enregistrement est
/// en cours. En cas de succès Corde s'arrête (l'installeur la relance) : la promesse ne se
/// résout jamais.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> Result<(), String> {
    updater::install(&app).await
}

/// Ouvre la page des versions dans le navigateur par défaut (adresse fixe,
/// `updates::RELEASES_PAGE`).
#[tauri::command]
pub fn open_releases_page() -> Result<(), String> {
    updater::open_releases_page()
}

/// Réglage « rechercher automatiquement les mises à jour » (`desktop.json`, vrai par défaut).
#[tauri::command]
pub fn get_update_auto_check(app: AppHandle) -> bool {
    app.state::<Updates>().auto_check()
}

#[tauri::command]
pub fn set_update_auto_check(app: AppHandle, enabled: bool) -> Result<(), String> {
    match app.state::<Updates>().set_auto_check(enabled) {
        Ok(()) => {
            tracing::info!(
                "mises à jour : vérification automatique {}",
                if enabled { "activée" } else { "désactivée" }
            );
            Ok(())
        }
        Err(e) => {
            tracing::error!("mises à jour : réglage non enregistré ({e})");
            Err(format!("Réglage non enregistré : {e}"))
        }
    }
}
