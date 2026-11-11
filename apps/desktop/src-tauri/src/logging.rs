//! Journal de l'application de bureau.
//!
//! L'application n'a pas de console (`windows_subsystem = "windows"`) : tout ce que la
//! coquille **et le serveur embarqué** écrivent avec `tracing` (et ce que Tauri écrit avec
//! `log`) va dans `<données>/logs/corde-desktop.log`. C'est le fichier à demander au
//! pilote quand quelque chose ne va pas.
//!
//! - ajout en fin de fichier (un second lancement, qui ne fait que réveiller la première
//!   instance, n'écrase rien) ;
//! - rotation simple au démarrage : au-delà de 2 Mo le fichier devient
//!   `corde-desktop.old.log` (un seul ancien fichier conservé) ;
//! - les paniques de tous les threads sont journalisées ;
//! - filtre : `info` par défaut, variable d'environnement `CORDE_LOG` pour le changer
//!   (syntaxe tracing : `debug`, `corde_server=debug,tower_http=info`…).

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tracing_subscriber::fmt::time::ChronoLocal;
use tracing_subscriber::EnvFilter;

pub const LOG_DIR: &str = "logs";
pub const LOG_FILE: &str = "corde-desktop.log";
pub const OLD_LOG_FILE: &str = "corde-desktop.old.log";
/// Variable d'environnement du filtre de journalisation.
pub const LOG_ENV: &str = "CORDE_LOG";
const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

/// Chemin du journal pour un dossier de données.
pub fn log_path(data_dir: &Path) -> PathBuf {
    data_dir.join(LOG_DIR).join(LOG_FILE)
}

/// Écrit dans le fichier, et aussi sur stderr dans un build de développement.
struct LogWriter {
    file: Option<File>,
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        #[cfg(debug_assertions)]
        {
            let _ = io::stderr().write_all(buf);
        }
        if let Some(file) = self.file.as_mut() {
            // Une erreur d'écriture (disque plein…) ne doit pas faire tomber l'application.
            let _ = file.write_all(buf);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.file.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}

/// Renomme le journal en `.old.log` s'il a trop grossi.
fn rotate(path: &Path) {
    let too_big = std::fs::metadata(path)
        .map(|m| m.len() > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_big {
        let old = path.with_file_name(OLD_LOG_FILE);
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(path, &old);
    }
}

fn open(path: &Path) -> io::Result<File> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    rotate(path);
    OpenOptions::new().create(true).append(true).open(path)
}

/// Installe le journal global et le crochet de panique. Retourne le chemin du journal
/// (même si le fichier n'a pas pu être ouvert : c'est celui qu'on indique à l'utilisateur).
pub fn init(data_dir: &Path) -> PathBuf {
    let path = log_path(data_dir);
    let (file, open_error) = match open(&path) {
        Ok(file) => (Some(file), None),
        Err(e) => (None, Some(e)),
    };

    let filter = corde_store::migrate::env_var(LOG_ENV)
        .ok()
        .and_then(|spec| EnvFilter::try_new(spec).ok())
        .unwrap_or_else(|| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(Mutex::new(LogWriter { file }))
        .with_ansi(false)
        .with_target(false)
        .with_timer(ChronoLocal::new("%Y-%m-%d %H:%M:%S%.3f".to_string()))
        .try_init();

    if let Some(e) = open_error {
        tracing::error!("journal {} inaccessible : {e}", path.display());
    }

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("sans nom");
        tracing::error!("PANIQUE dans le thread « {name} » : {info}");
        previous(info);
    }));

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_keeps_one_old_file() {
        let dir = std::env::temp_dir().join(format!(
            "corde-desktop-log-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = log_path(&dir);
        assert!(
            path.ends_with("logs/corde-desktop.log") || path.ends_with("logs\\corde-desktop.log")
        );

        // Petit fichier : on ajoute à la suite.
        let mut f = open(&path).unwrap();
        f.write_all(b"premier\n").unwrap();
        drop(f);
        let mut f = open(&path).unwrap();
        f.write_all(b"second\n").unwrap();
        drop(f);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "premier\nsecond\n");
        assert!(!path.with_file_name(OLD_LOG_FILE).exists());

        // Gros fichier : il devient l'ancien, le nouveau repart de zéro.
        std::fs::write(&path, vec![b'x'; (MAX_LOG_BYTES + 1) as usize]).unwrap();
        let mut f = open(&path).unwrap();
        f.write_all(b"neuf\n").unwrap();
        drop(f);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "neuf\n");
        let old = path.with_file_name(OLD_LOG_FILE);
        assert_eq!(std::fs::metadata(&old).unwrap().len(), MAX_LOG_BYTES + 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
