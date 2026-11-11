//! Icône de la zone de notification (près de l'horloge).
//!
//! - clic gauche (ou double-clic) : ouvrir Corde ;
//! - clic droit : menu « Ouvrir Corde », « Dashboard », « Quitter » ;
//! - l'info-bulle dit où en est Corde (en attente du jeu, jeu détecté, en piste, erreur).

use crate::shell::{self, Reveal};
use tauri::menu::MenuBuilder;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

/// Identifiant de l'icône (pour la retrouver avec `AppHandle::tray_by_id`).
pub const TRAY_ID: &str = "corde";

const MENU_OPEN: &str = "open";
const MENU_DASHBOARD: &str = "dashboard";
const MENU_QUIT: &str = "quit";

pub const TOOLTIP_STARTING: &str = "Corde — démarrage…";
pub const TOOLTIP_WAITING: &str = "Corde — en attente du jeu";
pub const TOOLTIP_GAME: &str = "Corde — jeu détecté";
pub const TOOLTIP_ON_TRACK: &str = "Corde — en piste";
pub const TOOLTIP_FAILED: &str = "Corde — erreur au démarrage";

/// Crée l'icône et son menu. À appeler une fois, dans `setup`.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = MenuBuilder::new(app)
        .text(MENU_OPEN, "Ouvrir Corde")
        .text(MENU_DASHBOARD, "Dashboard")
        .separator()
        .text(MENU_QUIT, "Quitter")
        .build()?;

    let mut tray = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip(TOOLTIP_STARTING)
        .menu(&menu)
        // Le menu est réservé au clic droit ; le clic gauche ouvre la fenêtre.
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            MENU_OPEN => shell::reveal(app, Reveal::Open),
            MENU_DASHBOARD => shell::reveal(app, Reveal::Dashboard),
            MENU_QUIT => {
                tracing::info!("« Quitter » demandé depuis la zone de notification");
                shell::quit(app);
            }
            other => tracing::debug!("entrée de menu inconnue : {other}"),
        })
        .on_tray_icon_event(|tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
            | TrayIconEvent::DoubleClick {
                button: MouseButton::Left,
                ..
            } => shell::reveal(tray.app_handle(), Reveal::Open),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// Change l'info-bulle de l'icône. Appelable de n'importe quel thread.
pub fn set_tooltip(app: &AppHandle, text: &'static str) {
    shell::on_main(app, move |app| {
        if let Some(tray) = app.tray_by_id(TRAY_ID) {
            if let Err(e) = tray.set_tooltip(Some(text)) {
                tracing::debug!("info-bulle : {e}");
            }
        }
    });
}
