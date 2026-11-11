//! Reprise des données de l'ancien nom de l'application, avant toute ouverture.
//!
//! - Dossier de données : [`corde_store::prepare_default_data_dir`] (règles et tests dans
//!   `corde_store::migrate`, partagés avec `corde-server`).
//! - Dossiers de l'identifiant Tauri : `%LOCALAPPDATA%\<identifiant>` (profil WebView2 : réglages
//!   de l'UI gardés par le navigateur) et `%APPDATA%\<identifiant>` (taille et position de la
//!   fenêtre). Même règle, sans repli : si le renommage échoue, l'application repart avec un
//!   profil neuf (les dispositions du tableau de bord sont aussi gardées par le serveur, dans
//!   `layouts/`, et reviennent seules).
//!
//! Sans Tauri : testé sous Linux par le banc des modules de la coquille.

use corde_store::migrate::{migrate_data_dir, DataDirMigration, DataDirOutcome};
use std::path::{Path, PathBuf};

/// Identifiant Tauri (`tauri.conf.json`) : nom des dossiers du WebView et de l'état de la fenêtre.
pub const IDENTIFIER: &str = "app.corde.desktop";
/// legacy name (Muret, before 0.3.0): identifiant Tauri de l'ancienne application.
pub const LEGACY_IDENTIFIER: &str = "app.muret.desktop";

/// Renomme `<base>\<ancien identifiant>` en `<base>\<identifiant>` pour chaque dossier de base
/// donné, et journalise ce qui a été fait. « Les deux existent » est l'état normal tant que
/// l'ancienne application est installée : ce cas-là n'est pas répété à chaque lancement.
pub fn migrate_identifier_dirs(bases: &[PathBuf]) -> Vec<DataDirMigration> {
    bases
        .iter()
        .map(|base| {
            let m = migrate_identifier_dir(base);
            if m.outcome != DataDirOutcome::BothExist {
                m.log();
            }
            m
        })
        .collect()
}

fn migrate_identifier_dir(base: &Path) -> DataDirMigration {
    migrate_data_dir(&base.join(IDENTIFIER), &base.join(LEGACY_IDENTIFIER))
}

/// `%LOCALAPPDATA%` et `%APPDATA%` (Windows ; ailleurs, ce qui est défini).
pub fn identifier_bases() -> Vec<PathBuf> {
    ["LOCALAPPDATA", "APPDATA"]
        .iter()
        .filter_map(|var| std::env::var_os(var).map(PathBuf::from))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_dirs_are_renamed_once_and_never_merged() {
        let root = std::env::temp_dir().join(format!("corde-desktop-ident-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (local, roaming) = (root.join("Local"), root.join("Roaming"));
        let profile = local.join(LEGACY_IDENTIFIER).join("EBWebView");
        std::fs::create_dir_all(&profile).unwrap();
        std::fs::write(profile.join("Local State"), b"{}").unwrap();
        std::fs::create_dir_all(roaming.join(LEGACY_IDENTIFIER)).unwrap();
        std::fs::create_dir_all(roaming.join(IDENTIFIER)).unwrap();

        let out = migrate_identifier_dirs(&[local.clone(), roaming.clone()]);
        assert_eq!(out[0].outcome, DataDirOutcome::Moved);
        assert!(local
            .join(IDENTIFIER)
            .join("EBWebView/Local State")
            .is_file());
        assert!(!local.join(LEGACY_IDENTIFIER).exists());
        // Both present: nothing touched.
        assert_eq!(out[1].outcome, DataDirOutcome::BothExist);
        assert!(roaming.join(LEGACY_IDENTIFIER).is_dir());

        let again = migrate_identifier_dirs(std::slice::from_ref(&local));
        assert_eq!(again[0].outcome, DataDirOutcome::Nothing);
        std::fs::remove_dir_all(&root).ok();
    }
}
