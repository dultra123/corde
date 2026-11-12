//! Mises à jour de l'application : toute la logique qui ne dépend pas de Tauri.
//!
//! La coquille ([`crate::updater`]) s'appuie sur `tauri-plugin-updater` pour télécharger et
//! vérifier la nouvelle version ; ce module décide **quand** et **si** on le fait :
//!
//! - [`Machine`] : l'état montré à l'interface (version installée, dernière vérification,
//!   mise à jour disponible, téléchargement en cours, erreur) et ses transitions ;
//! - [`install_blocker`] : jamais d'installation pendant que le jeu tourne ou qu'un
//!   enregistrement est en cours ;
//! - [`is_newer`] : comparaison de versions (`0.10.0` > `0.9.3`, `0.6.0` > `0.6.0-rc.1`) ;
//! - [`auto_check_due`] : 30 s après le démarrage, puis toutes les 6 h ;
//! - [`should_notify`] : une seule notification Windows par version, jamais en jeu ;
//! - [`redact`] : pas de chemin personnel dans le journal.
//!
//! ```text
//!            begin_check            finish_check (ok / erreur)
//!   Idle ───────────────▶ Checking ───────────────────────────▶ Idle
//!    │ begin_install (mise à jour connue, jeu fermé, pas d'enregistrement)
//!    ▼
//!   Downloading ──progress──▶ Downloading ──begin_installing──▶ Installing ──▶ (fin du processus)
//!    │ download_failed / blocked                                   │ install_failed
//!    └──────────────────────────────▶ Idle ◀───────────────────────┘
//! ```
//!
//! Testé sous Linux par le banc des modules de la coquille (hors dépôt).

use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Page des versions publiées (ouverte dans le navigateur par défaut).
pub const RELEASES_PAGE: &str = "https://github.com/dultra123/corde/releases";
/// Événement envoyé à l'interface à chaque changement d'état (charge utile : [`Snapshot`]).
pub const EVENT: &str = "update-status";
/// Première vérification automatique après le démarrage.
pub const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
/// Intervalle entre deux vérifications automatiques.
pub const CHECK_PERIOD: Duration = Duration::from_secs(6 * 60 * 60);

/// Ce que fait la coquille en ce moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Checking,
    Downloading,
    Installing,
}

/// Une version plus récente, annoncée par `latest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Available {
    pub version: String,
    /// Date de publication telle qu'annoncée (RFC 3339), si présente.
    pub date: Option<String>,
    /// Notes de version (Markdown), si présentes.
    pub notes: Option<String>,
}

/// Avancement du téléchargement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub downloaded: u64,
    /// Taille annoncée par le serveur (`Content-Length`), si connue.
    pub total: Option<u64>,
}

/// Nature d'une erreur ; l'interface choisit son texte d'après elle (FR / EN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    /// La vérification a échoué (hors ligne, dépôt privé → 404, réponse illisible).
    Check,
    /// Le téléchargement ou la vérification de la signature a échoué.
    Download,
    /// L'installeur n'a pas pu être lancé.
    Install,
    /// Refus : le jeu est détecté.
    GameRunning,
    /// Refus : un enregistrement est en cours.
    Recording,
    /// Refus : la coquille ne sait pas si le jeu tourne (serveur embarqué muet).
    ActivityUnknown,
    /// Refus : une vérification ou une installation est déjà en cours.
    Busy,
    /// Refus : aucune mise à jour connue (vérifier d'abord).
    NothingToInstall,
}

/// Dernière erreur, montrée jusqu'à la prochaine action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UpdateError {
    pub kind: ErrorKind,
    /// Texte français : détail technique (déjà nettoyé par [`redact`]) ou message de refus.
    pub message: String,
}

/// État complet envoyé à l'interface (`update_status`, événement [`EVENT`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Snapshot {
    pub current_version: String,
    pub auto_check: bool,
    pub phase: Phase,
    /// Fin de la dernière vérification (réussie ou non), en millisecondes depuis 1970.
    pub last_check_ms: Option<u64>,
    /// La dernière vérification a réussi.
    pub last_check_ok: Option<bool>,
    pub available: Option<Available>,
    pub progress: Option<Progress>,
    pub error: Option<UpdateError>,
    /// Version d'avant si Corde vient d'être mise à jour (au premier lancement qui suit).
    pub updated_from: Option<String>,
}

/// Activité du pilote lue dans `GET /api/status` du serveur embarqué.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Activity {
    /// Le jeu est détecté (ou le pilote est en piste).
    pub game_running: bool,
    /// Un enregistrement de session est en cours.
    pub recording: bool,
}

/// Pourquoi on ne peut pas installer maintenant. `None` = rien ne s'y oppose.
///
/// `activity = None` (le serveur embarqué ne répond pas) est un refus : dans le doute, on ne
/// ferme pas Corde sous les roues du pilote.
pub fn install_blocker(activity: Option<Activity>) -> Option<ErrorKind> {
    match activity {
        None => Some(ErrorKind::ActivityUnknown),
        Some(a) if a.game_running => Some(ErrorKind::GameRunning),
        Some(a) if a.recording => Some(ErrorKind::Recording),
        Some(_) => None,
    }
}

/// Message de refus en français (journal, promesse rejetée côté interface).
pub fn refusal_message(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::GameRunning => {
            "Le jeu est ouvert : ferme Le Mans Ultimate, puis relance l'installation. Corde ne \
             s'arrête jamais pendant que tu roules."
        }
        ErrorKind::Recording => {
            "Un enregistrement est en cours : attends la fin de la session (ou arrête \
             l'enregistrement), puis relance l'installation."
        }
        ErrorKind::ActivityUnknown => {
            "Corde ne sait pas si le jeu tourne (le serveur ne répond pas) : réessaie dans un \
             instant."
        }
        ErrorKind::Busy => "Une vérification ou une installation est déjà en cours.",
        ErrorKind::NothingToInstall => {
            "Aucune mise à jour connue : clique d'abord sur « Rechercher une mise à jour »."
        }
        ErrorKind::Check => "Impossible de vérifier les mises à jour pour l'instant.",
        ErrorKind::Download => "Le téléchargement de la mise à jour a échoué.",
        ErrorKind::Install => "L'installeur de la mise à jour n'a pas pu être lancé.",
    }
}

/// La machine d'états des mises à jour. Toutes les méthodes sont des transitions explicites :
/// un appel hors de propos est refusé (vérification pendant un téléchargement…) plutôt que
/// d'emmêler l'état.
#[derive(Debug, Clone)]
pub struct Machine {
    snap: Snapshot,
    /// Octets reçus au dernier événement de progression envoyé (voir [`Machine::progress`]).
    last_emitted: u64,
}

/// Envoyer un événement de progression tous les 512 Kio au plus quand la taille est inconnue.
const PROGRESS_STEP_UNKNOWN: u64 = 512 * 1024;

impl Machine {
    pub fn new(current_version: &str, auto_check: bool, updated_from: Option<String>) -> Self {
        Self {
            snap: Snapshot {
                current_version: current_version.to_string(),
                auto_check,
                phase: Phase::Idle,
                last_check_ms: None,
                last_check_ok: None,
                available: None,
                progress: None,
                error: None,
                updated_from,
            },
            last_emitted: 0,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.clone()
    }

    pub fn phase(&self) -> Phase {
        self.snap.phase
    }

    pub fn available(&self) -> Option<&Available> {
        self.snap.available.as_ref()
    }

    pub fn set_auto_check(&mut self, enabled: bool) {
        self.snap.auto_check = enabled;
    }

    fn refuse(&mut self, kind: ErrorKind) -> ErrorKind {
        self.snap.error = Some(UpdateError {
            kind,
            message: refusal_message(kind).to_string(),
        });
        kind
    }

    /// Début d'une vérification. Refusée si une autre action est en cours (l'état n'est alors
    /// pas modifié : l'action en cours garde son affichage).
    pub fn begin_check(&mut self) -> Result<(), ErrorKind> {
        if self.snap.phase != Phase::Idle {
            return Err(ErrorKind::Busy);
        }
        self.snap.phase = Phase::Checking;
        self.snap.error = None;
        Ok(())
    }

    /// Fin de la vérification : `Ok(Some)` = mise à jour disponible, `Ok(None)` = à jour,
    /// `Err(détail)` = échec. Un échec n'efface pas une mise à jour déjà connue.
    pub fn finish_check(&mut self, now: SystemTime, result: Result<Option<Available>, String>) {
        if self.snap.phase != Phase::Checking {
            return;
        }
        self.snap.phase = Phase::Idle;
        self.snap.last_check_ms = Some(unix_ms(now));
        match result {
            Ok(available) => {
                self.snap.last_check_ok = Some(true);
                self.snap.available = available;
                self.snap.error = None;
            }
            Err(detail) => {
                self.snap.last_check_ok = Some(false);
                self.snap.error = Some(UpdateError {
                    kind: ErrorKind::Check,
                    message: detail,
                });
            }
        }
    }

    /// Demande d'installation : il faut une mise à jour connue, rien en cours, et ni jeu ni
    /// enregistrement ([`install_blocker`]). En cas de refus, l'erreur est mémorisée (et
    /// renvoyée) ; sinon la phase passe à `Downloading`.
    pub fn begin_install(&mut self, activity: Option<Activity>) -> Result<(), ErrorKind> {
        if self.snap.phase != Phase::Idle {
            return Err(ErrorKind::Busy);
        }
        if self.snap.available.is_none() {
            return Err(self.refuse(ErrorKind::NothingToInstall));
        }
        if let Some(kind) = install_blocker(activity) {
            return Err(self.refuse(kind));
        }
        self.snap.phase = Phase::Downloading;
        self.snap.progress = Some(Progress {
            downloaded: 0,
            total: None,
        });
        self.snap.error = None;
        self.last_emitted = 0;
        Ok(())
    }

    /// Un morceau du téléchargement est arrivé. Retourne `true` quand l'interface mérite un
    /// événement (le pourcentage entier a changé, ou 512 Kio de plus si la taille est
    /// inconnue) : pas un événement par paquet réseau.
    pub fn progress(&mut self, chunk: u64, total: Option<u64>) -> bool {
        if self.snap.phase != Phase::Downloading {
            return false;
        }
        let p = self.snap.progress.get_or_insert(Progress {
            downloaded: 0,
            total: None,
        });
        p.downloaded = p.downloaded.saturating_add(chunk);
        p.total = total.filter(|t| *t > 0);
        let (now, before) = (p.downloaded, self.last_emitted);
        let worth = match p.total {
            Some(t) => percent(now, t) != percent(before, t),
            None => now.saturating_sub(before) >= PROGRESS_STEP_UNKNOWN,
        };
        if worth {
            self.last_emitted = now;
        }
        worth
    }

    /// Le téléchargement (ou la vérification de sa signature) a échoué.
    pub fn download_failed(&mut self, detail: String) {
        if self.snap.phase != Phase::Downloading {
            return;
        }
        self.snap.phase = Phase::Idle;
        self.snap.progress = None;
        self.snap.error = Some(UpdateError {
            kind: ErrorKind::Download,
            message: detail,
        });
    }

    /// Téléchargement terminé, mais le jeu (ou un enregistrement) a démarré entre-temps :
    /// retour à `Idle` avec le refus. L'installeur reste en mémoire chez l'appelant.
    pub fn blocked_after_download(&mut self, kind: ErrorKind) {
        if self.snap.phase != Phase::Downloading {
            return;
        }
        self.snap.phase = Phase::Idle;
        self.snap.progress = None;
        self.refuse(kind);
    }

    /// L'installeur va être lancé ; Corde va s'arrêter.
    pub fn begin_installing(&mut self) {
        if self.snap.phase == Phase::Downloading {
            self.snap.phase = Phase::Installing;
        }
    }

    /// L'installeur n'a pas pu être lancé.
    pub fn install_failed(&mut self, detail: String) {
        if !matches!(self.snap.phase, Phase::Installing | Phase::Downloading) {
            return;
        }
        self.snap.phase = Phase::Idle;
        self.snap.progress = None;
        self.snap.error = Some(UpdateError {
            kind: ErrorKind::Install,
            message: detail,
        });
    }
}

fn percent(done: u64, total: u64) -> u64 {
    done.min(total).saturating_mul(100) / total.max(1)
}

/// Millisecondes depuis le 1er janvier 1970 (0 pour une horloge antérieure).
pub fn unix_ms(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Une version `x.y.z[-pré][+build]` découpée pour la comparaison.
#[derive(Debug, PartialEq, Eq)]
struct Version<'a> {
    core: [u64; 3],
    pre: Option<&'a str>,
}

fn parse_version(text: &str) -> Option<Version<'_>> {
    let text = text.trim();
    let text = text.strip_prefix(['v', 'V']).unwrap_or(text);
    // Les métadonnées de build (`+…`) ne comptent pas dans l'ordre (SemVer §10).
    let text = text.split_once('+').map_or(text, |(v, _)| v);
    let (core, pre) = match text.split_once('-') {
        Some((core, pre)) if !pre.is_empty() => (core, Some(pre)),
        Some(_) => return None,
        None => (text, None),
    };
    let mut parts = core.split('.');
    let mut out = [0u64; 3];
    for slot in &mut out {
        let part = parts.next()?;
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse().ok()?;
    }
    if parts.next().is_some() {
        return None;
    }
    Some(Version { core: out, pre })
}

/// Ordre des pré-versions (SemVer §11) : identifiants numériques comparés comme des nombres et
/// avant les alphanumériques ; une liste plus longue l'emporte à préfixe égal.
fn cmp_pre(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let mut ia = a.split('.');
    let mut ib = b.split('.');
    loop {
        match (ia.next(), ib.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let ord = match (x.parse::<u64>(), y.parse::<u64>()) {
                    (Ok(nx), Ok(ny)) => nx.cmp(&ny),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => x.cmp(y),
                };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
        }
    }
}

/// `candidate` est-elle strictement plus récente que `current` ? Une version illisible n'est
/// jamais « plus récente » (on n'installe pas ce qu'on ne comprend pas).
pub fn is_newer(candidate: &str, current: &str) -> bool {
    use std::cmp::Ordering;
    let (Some(c), Some(i)) = (parse_version(candidate), parse_version(current)) else {
        return false;
    };
    match c.core.cmp(&i.core) {
        Ordering::Greater => true,
        Ordering::Less => false,
        Ordering::Equal => match (c.pre, i.pre) {
            (None, None) => false,
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(a), Some(b)) => cmp_pre(a, b) == Ordering::Greater,
        },
    }
}

/// Faut-il afficher la notification Windows « Corde X est disponible » ?
///
/// Une seule fois par version (`last_notified` est mémorisé dans `desktop.json`), et jamais
/// pendant que le jeu tourne : la notification attend la fermeture du jeu.
pub fn should_notify(available: &str, last_notified: Option<&str>, game_running: bool) -> bool {
    !game_running && last_notified.map(str::trim) != Some(available.trim())
}

/// La vérification automatique est-elle due ?
///
/// - jamais si le réglage « vérifier automatiquement » est coupé ;
/// - pas avant [`FIRST_CHECK_DELAY`] après le démarrage ;
/// - puis toutes les [`CHECK_PERIOD`] depuis la dernière vérification, **manuelle ou
///   automatique, réussie ou non** (hors ligne : on réessaie six heures plus tard).
pub fn auto_check_due(
    auto_check: bool,
    since_start: Duration,
    since_last_check: Option<Duration>,
) -> bool {
    auto_check
        && since_start >= FIRST_CHECK_DELAY
        && since_last_check.is_none_or(|d| d >= CHECK_PERIOD)
}

/// Remplace dans `text` chaque valeur sensible par son nom générique (`%USERPROFILE%`…), la
/// plus longue d'abord. Sert à journaliser les erreurs du plugin (chemins de fichiers
/// temporaires) sans le nom du compte Windows. Comparaison insensible à la casse ASCII
/// (chemins Windows).
pub fn redact(text: &str, secrets: &[(String, &str)]) -> String {
    let mut pairs: Vec<&(String, &str)> = secrets
        .iter()
        .filter(|(value, _)| value.trim().len() >= 3)
        .collect();
    pairs.sort_by_key(|(value, _)| std::cmp::Reverse(value.len()));
    let mut out = text.to_string();
    for (value, name) in pairs {
        out = replace_ignore_ascii_case(&out, value, name);
    }
    out
}

fn replace_ignore_ascii_case(haystack: &str, needle: &str, with: &str) -> String {
    let lower_h = haystack.to_ascii_lowercase();
    let lower_n = needle.to_ascii_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut pos = 0;
    // `to_ascii_lowercase` garde les positions d'octets : les indices valent pour `haystack`.
    while let Some(found) = lower_h[pos..].find(&lower_n) {
        let start = pos + found;
        out.push_str(&haystack[pos..start]);
        out.push_str(with);
        pos = start + needle.len();
    }
    out.push_str(&haystack[pos..]);
    out
}

/// Les valeurs à masquer sur cette machine : dossiers du profil et nom du compte.
pub fn personal_values() -> Vec<(String, &'static str)> {
    [
        ("LOCALAPPDATA", "%LOCALAPPDATA%"),
        ("APPDATA", "%APPDATA%"),
        ("TEMP", "%TEMP%"),
        ("TMP", "%TEMP%"),
        ("USERPROFILE", "%USERPROFILE%"),
        ("HOME", "~"),
        ("USERNAME", "%USERNAME%"),
        ("USER", "%USERNAME%"),
    ]
    .iter()
    .filter_map(|(var, name)| {
        std::env::var(var)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(|v| (v, *name))
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn available(v: &str) -> Available {
        Available {
            version: v.into(),
            date: Some("2026-11-12T10:00:00Z".into()),
            notes: Some("- nouveauté".into()),
        }
    }

    const IDLE: Option<Activity> = Some(Activity {
        game_running: false,
        recording: false,
    });

    #[test]
    fn versions_compare_like_semver() {
        assert!(is_newer("0.6.1", "0.6.0"));
        assert!(is_newer("0.10.0", "0.9.9"), "numeric, not lexicographic");
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(is_newer("v0.7.0", "0.6.0"));
        assert!(is_newer("0.6.0", "0.6.0-rc.1"));
        assert!(is_newer("0.6.0-rc.10", "0.6.0-rc.2"));
        assert!(is_newer("0.6.0-rc.1", "0.6.0-beta"));
        assert!(is_newer("0.6.0-alpha.1", "0.6.0-alpha"));
        assert!(!is_newer("0.6.0", "0.6.0"));
        assert!(!is_newer("0.6.0+build.7", "0.6.0"));
        assert!(!is_newer("0.5.1", "0.6.0"));
        assert!(!is_newer("0.6.0-rc.1", "0.6.0"));
        // Illisible : jamais une mise à jour.
        for bad in [
            "", "latest", "0.6", "0.6.0.1", "0.6.x", "0.6.0-", "-1.0.0", "1..0",
        ] {
            assert!(!is_newer(bad, "0.5.0"), "{bad:?}");
            assert!(!is_newer("9.9.9", bad), "{bad:?}");
        }
    }

    #[test]
    fn check_cycle() {
        let mut m = Machine::new("0.6.0", true, None);
        let s = m.snapshot();
        assert_eq!(
            (s.phase, s.last_check_ms, s.available.clone()),
            (Phase::Idle, None, None)
        );

        m.begin_check().unwrap();
        assert_eq!(m.phase(), Phase::Checking);
        assert_eq!(m.begin_check(), Err(ErrorKind::Busy), "one check at a time");
        let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
        m.finish_check(t, Ok(None));
        let s = m.snapshot();
        assert_eq!(s.phase, Phase::Idle);
        assert_eq!(s.last_check_ms, Some(1_700_000_000_123));
        assert_eq!(s.last_check_ok, Some(true));
        assert!(s.available.is_none() && s.error.is_none());

        m.begin_check().unwrap();
        m.finish_check(t, Ok(Some(available("0.6.1"))));
        assert_eq!(m.available().unwrap().version, "0.6.1");

        // Hors ligne : erreur de vérification, la mise à jour connue reste affichée.
        m.begin_check().unwrap();
        m.finish_check(t, Err("error sending request".into()));
        let s = m.snapshot();
        assert_eq!(s.error.as_ref().unwrap().kind, ErrorKind::Check);
        assert_eq!(s.last_check_ok, Some(false));
        assert_eq!(s.available.unwrap().version, "0.6.1");

        // Une fin de vérification hors propos ne change rien.
        let before = m.snapshot();
        m.finish_check(t, Ok(None));
        assert_eq!(m.snapshot(), before);
    }

    #[test]
    fn install_is_refused_while_racing_or_recording() {
        let mut m = Machine::new("0.6.0", true, None);
        assert_eq!(m.begin_install(IDLE), Err(ErrorKind::NothingToInstall));
        m.begin_check().unwrap();
        m.finish_check(SystemTime::now(), Ok(Some(available("0.6.1"))));

        let racing = Some(Activity {
            game_running: true,
            recording: true,
        });
        let recording = Some(Activity {
            game_running: false,
            recording: true,
        });
        for (activity, kind) in [
            (racing, ErrorKind::GameRunning),
            (recording, ErrorKind::Recording),
            (None, ErrorKind::ActivityUnknown),
        ] {
            assert_eq!(m.begin_install(activity), Err(kind));
            let s = m.snapshot();
            assert_eq!(s.phase, Phase::Idle);
            let err = s.error.unwrap();
            assert_eq!(err.kind, kind);
            assert_eq!(err.message, refusal_message(kind));
        }
        assert!(refusal_message(ErrorKind::GameRunning).contains("jeu"));
        assert!(refusal_message(ErrorKind::Recording).contains("enregistrement"));

        m.begin_install(IDLE).unwrap();
        assert_eq!(m.phase(), Phase::Downloading);
        assert!(m.snapshot().error.is_none(), "the refusal is cleared");
        assert_eq!(m.begin_check(), Err(ErrorKind::Busy));
        assert_eq!(m.begin_install(IDLE), Err(ErrorKind::Busy));
    }

    #[test]
    fn download_progress_is_throttled() {
        let mut m = Machine::new("0.6.0", true, None);
        m.begin_check().unwrap();
        m.finish_check(SystemTime::now(), Ok(Some(available("0.6.1"))));
        m.begin_install(IDLE).unwrap();

        // 10 000 octets annoncés, paquets de 10 : un événement par pour cent, pas par paquet.
        let mut events = 0;
        for _ in 0..1000 {
            if m.progress(10, Some(10_000)) {
                events += 1;
            }
        }
        assert_eq!(events, 100);
        let p = m.snapshot().progress.unwrap();
        assert_eq!((p.downloaded, p.total), (10_000, Some(10_000)));

        // Taille inconnue : tous les 512 Kio.
        let mut m2 = Machine::new("0.6.0", true, None);
        m2.begin_check().unwrap();
        m2.finish_check(SystemTime::now(), Ok(Some(available("0.6.1"))));
        m2.begin_install(IDLE).unwrap();
        let events = (0..64).filter(|_| m2.progress(64 * 1024, None)).count();
        assert_eq!(events, 8);
        assert_eq!(m2.snapshot().progress.unwrap().total, None);

        m.download_failed("signature invalide".into());
        let s = m.snapshot();
        assert_eq!(s.phase, Phase::Idle);
        assert!(s.progress.is_none());
        assert_eq!(s.error.unwrap().kind, ErrorKind::Download);
        assert!(!m.progress(10, Some(10)), "no progress outside a download");
    }

    #[test]
    fn game_started_during_the_download() {
        let mut m = Machine::new("0.6.0", true, None);
        m.begin_check().unwrap();
        m.finish_check(SystemTime::now(), Ok(Some(available("0.6.1"))));
        m.begin_install(IDLE).unwrap();
        m.blocked_after_download(ErrorKind::GameRunning);
        let s = m.snapshot();
        assert_eq!(s.phase, Phase::Idle);
        assert_eq!(s.error.unwrap().kind, ErrorKind::GameRunning);
        assert_eq!(
            s.available.unwrap().version,
            "0.6.1",
            "still installable later"
        );
    }

    #[test]
    fn install_then_failure_to_launch() {
        let mut m = Machine::new("0.6.0", false, Some("0.5.1".into()));
        m.begin_check().unwrap();
        m.finish_check(SystemTime::now(), Ok(Some(available("0.6.1"))));
        m.begin_install(IDLE).unwrap();
        m.begin_installing();
        assert_eq!(m.phase(), Phase::Installing);
        m.install_failed("ShellExecuteW: 5".into());
        let s = m.snapshot();
        assert_eq!(s.phase, Phase::Idle);
        assert_eq!(s.error.unwrap().kind, ErrorKind::Install);
        assert_eq!(s.updated_from.as_deref(), Some("0.5.1"));
        assert!(!s.auto_check);
    }

    #[test]
    fn snapshot_json_is_the_ui_contract() {
        let mut m = Machine::new("0.6.0", true, None);
        m.begin_check().unwrap();
        m.finish_check(
            UNIX_EPOCH + Duration::from_millis(5),
            Ok(Some(available("0.6.1"))),
        );
        let v = serde_json::to_value(m.snapshot()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "current_version": "0.6.0",
                "auto_check": true,
                "phase": "idle",
                "last_check_ms": 5,
                "last_check_ok": true,
                "available": {
                    "version": "0.6.1",
                    "date": "2026-11-12T10:00:00Z",
                    "notes": "- nouveauté"
                },
                "progress": null,
                "error": null,
                "updated_from": null
            })
        );
        m.begin_install(None).unwrap_err();
        let v = serde_json::to_value(m.snapshot()).unwrap();
        assert_eq!(v["error"]["kind"], "activity_unknown");
        m.begin_install(IDLE).unwrap();
        m.progress(3, Some(10));
        let v = serde_json::to_value(m.snapshot()).unwrap();
        assert_eq!(v["phase"], "downloading");
        assert_eq!(
            v["progress"],
            serde_json::json!({ "downloaded": 3, "total": 10 })
        );
    }

    #[test]
    fn one_notification_per_version_and_never_in_game() {
        assert!(should_notify("0.6.1", None, false));
        assert!(
            !should_notify("0.6.1", None, true),
            "waits for the game to close"
        );
        assert!(!should_notify("0.6.1", Some("0.6.1"), false));
        assert!(!should_notify("0.6.1", Some(" 0.6.1 "), false));
        assert!(should_notify("0.6.2", Some("0.6.1"), false));
    }

    #[test]
    fn automatic_check_schedule() {
        let s = Duration::from_secs;
        assert!(!auto_check_due(true, s(29), None));
        assert!(auto_check_due(true, s(30), None));
        assert!(!auto_check_due(false, s(3600), None), "setting off");
        // Une vérification (manuelle) récente repousse l'automatique.
        assert!(!auto_check_due(true, s(40), Some(s(25))));
        assert!(!auto_check_due(
            true,
            s(7 * 3600),
            Some(CHECK_PERIOD - s(1))
        ));
        assert!(auto_check_due(true, s(7 * 3600), Some(CHECK_PERIOD)));
    }

    #[test]
    fn personal_paths_are_redacted() {
        let secrets = vec![
            (
                r"C:\Users\Pilote\AppData\Local".to_string(),
                "%LOCALAPPDATA%",
            ),
            (r"C:\Users\Pilote".to_string(), "%USERPROFILE%"),
            ("Pilote".to_string(), "%USERNAME%"),
            ("ab".to_string(), "x"), // trop court : ignoré
        ];
        let text = r"failed to write c:\users\pilote\AppData\Local\Temp\Corde-0.6.1-installer.exe (Pilote)";
        assert_eq!(
            redact(text, &secrets),
            r"failed to write %LOCALAPPDATA%\Temp\Corde-0.6.1-installer.exe (%USERNAME%)"
        );
        assert_eq!(redact("rien à cacher", &secrets), "rien à cacher");
        assert_eq!(redact("é Pilote é", &secrets), "é %USERNAME% é");
    }

    #[test]
    fn releases_page_is_fixed() {
        assert_eq!(RELEASES_PAGE, "https://github.com/dultra123/corde/releases");
        assert_eq!(EVENT, "update-status");
        assert_eq!(FIRST_CHECK_DELAY, Duration::from_secs(30));
        assert_eq!(CHECK_PERIOD, Duration::from_secs(21_600));
    }
}
