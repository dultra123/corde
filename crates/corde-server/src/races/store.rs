//! « Mes courses » and the settings of the « Courses » page, kept by the server
//! so that the PC and the phone share them:
//!
//! * `<data-dir>/races/picks.json` — the starts the driver picked (≤ [`MAX_PICKS`]);
//! * `<data-dir>/races/settings.json` — reminder delays, categories and levels
//!   shown, manual hour correction, driver ratings, access requirements;
//! * `<data-dir>/races/reminders.json` — the reminders already sent (no
//!   duplicate after a restart).
//!
//! Every input is validated and bounded: these routes are open to the local
//! network (see `docs/SECURITY-MODEL.md`). Reading is tolerant: a missing or
//! broken file means the defaults.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const RACES_DIR: &str = "races";
pub const PICKS_FILE: &str = "picks.json";
pub const SETTINGS_FILE: &str = "settings.json";
pub const FIRED_FILE: &str = "reminders.json";

/// Most picks kept at once.
pub const MAX_PICKS: usize = 50;
/// A pick is forgotten this long after its start.
pub const KEEP_AFTER_START_MS: i64 = 3 * 3_600_000;
/// Reminder delays the driver may choose, in minutes before the start.
pub const LEAD_CHOICES: [u32; 7] = [60, 30, 15, 10, 5, 2, 0];
pub const LEVELS: [&str; 3] = ["beginner", "intermediate", "advanced"];
pub const CATS: [&str; 4] = ["daily", "weekly", "special", "champ"];
/// Safety / driver ratings as the game shows them (`""` = none).
pub const RATINGS: [&str; 17] = [
    "", "B0", "B1", "B2", "B3", "S0", "S1", "S2", "S3", "G0", "G1", "G2", "G3", "P0", "P1", "P2",
    "P3",
];
/// Keys of the access requirements: the daily levels, then the other categories.
pub const REQ_KEYS: [&str; 6] = [
    "beginner",
    "intermediate",
    "advanced",
    "weekly",
    "special",
    "champ",
];

const MAX_NAME: usize = 120;
const MAX_ID: usize = 120;
const MAX_DURATION: usize = 40;
const MAX_IMAGE: usize = 500;
/// Plausible start times: 2020-01-01 … 2100-01-01.
const MIN_START_MS: i64 = 1_577_836_800_000;
const MAX_START_MS: i64 = 4_102_444_800_000;

/// One start the driver picked. `id` = `<race_id>@<start_ms>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pick {
    pub id: String,
    pub race_id: String,
    /// `daily` | `weekly` | `special` | `champ`.
    pub cat: String,
    /// `rc` (racecontrol.gg) | `lt` (lmu.tools): the hour correction applies to `rc` only.
    pub src: String,
    pub name: String,
    pub level: Option<String>,
    pub track: String,
    #[serde(default)]
    pub duration: String,
    #[serde(default)]
    pub image: Option<String>,
    /// The start (for a special event: the reminder of the day, 10:00 local).
    pub start_ms: i64,
    /// Special events: the reminder of the evening before (18:00 local),
    /// computed by the device that picked it.
    #[serde(default)]
    pub eve_ms: Option<i64>,
}

pub fn pick_id(race_id: &str, start_ms: i64) -> String {
    format!("{race_id}@{start_ms}")
}

fn bad(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

fn check_text(field: &str, v: &str, max: usize, required: bool) -> io::Result<()> {
    if required && v.trim().is_empty() {
        return Err(bad(format!("`{field}` est vide")));
    }
    if v.chars().count() > max {
        return Err(bad(format!("`{field}` dépasse {max} caractères")));
    }
    if v.chars().any(char::is_control) {
        return Err(bad(format!("`{field}` contient un caractère de contrôle")));
    }
    Ok(())
}

/// A race identifier as the parser builds them (`one-stint-sprint-advanced`, `weekly-gt3-sprint`).
pub fn valid_race_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// A pick identifier: `<race_id>@<start_ms>`.
pub fn valid_pick_id(id: &str) -> bool {
    match id.rsplit_once('@') {
        Some((race, start)) => {
            valid_race_id(race)
                && !start.is_empty()
                && start.len() <= 14
                && start.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

/// Image addresses found in the pages: `https://` only, no space nor quote.
pub fn valid_image(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() <= MAX_IMAGE
        && !url.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\')
        })
}

impl Pick {
    /// Parse and check a pick sent by a client for `path_id`.
    pub fn from_request(path_id: &str, body: &[u8]) -> io::Result<Self> {
        let p: Pick =
            serde_json::from_slice(body).map_err(|e| bad(format!("choix invalide : {e}")))?;
        p.validate()?;
        if p.id != path_id {
            return Err(bad("l'identifiant du choix ne correspond pas à l'adresse"));
        }
        Ok(p)
    }

    pub fn validate(&self) -> io::Result<()> {
        if !valid_race_id(&self.race_id) {
            return Err(bad("`race_id` invalide"));
        }
        if !(MIN_START_MS..=MAX_START_MS).contains(&self.start_ms) {
            return Err(bad("`start_ms` invalide"));
        }
        if self.id != pick_id(&self.race_id, self.start_ms) {
            return Err(bad("`id` doit valoir <race_id>@<start_ms>"));
        }
        if !CATS.contains(&self.cat.as_str()) {
            return Err(bad("`cat` invalide"));
        }
        if self.src != "rc" && self.src != "lt" {
            return Err(bad("`src` invalide"));
        }
        if let Some(l) = &self.level {
            if !LEVELS.contains(&l.as_str()) {
                return Err(bad("`level` invalide"));
            }
        }
        check_text("name", &self.name, MAX_NAME, true)?;
        check_text("track", &self.track, MAX_NAME, true)?;
        check_text("duration", &self.duration, MAX_DURATION, false)?;
        if let Some(img) = &self.image {
            if !valid_image(img) {
                return Err(bad("`image` invalide (adresse https:// attendue)"));
            }
        }
        match (self.cat.as_str(), self.eve_ms) {
            (_, None) => {}
            ("special", Some(eve)) => {
                if eve >= self.start_ms || self.start_ms - eve > 2 * 86_400_000 {
                    return Err(bad("`eve_ms` invalide"));
                }
            }
            _ => return Err(bad("`eve_ms` est réservé aux événements spéciaux")),
        }
        Ok(())
    }
}

/// Rating pair (`""` = none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Ratings {
    #[serde(default)]
    pub sr: String,
    #[serde(default)]
    pub dr: String,
}

/// Settings of the « Courses » page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RaceSettings {
    /// Reminder delays (minutes before the start), largest first.
    pub leads: Vec<u32>,
    /// Daily levels shown.
    pub levels: Vec<String>,
    /// Categories shown.
    pub cats: Vec<String>,
    /// Manual correction of the racecontrol.gg times, hours (−2 … +2).
    pub hour_fix: i32,
    /// The driver's ratings (what the game shows in its multiplayer menu).
    pub profile: Ratings,
    /// Access requirements shown as badges, by [`REQ_KEYS`].
    pub req: BTreeMap<String, Ratings>,
}

impl Default for RaceSettings {
    fn default() -> Self {
        let r = |sr: &str| Ratings {
            sr: sr.into(),
            dr: String::new(),
        };
        Self {
            leads: vec![15, 5],
            levels: LEVELS.iter().map(|s| s.to_string()).collect(),
            cats: CATS.iter().map(|s| s.to_string()).collect(),
            hour_fix: 0,
            profile: Ratings::default(),
            // Published rules (no DR requirement is published).
            req: BTreeMap::from([
                ("beginner".into(), r("B0")),
                ("intermediate".into(), r("S0")),
                ("advanced".into(), r("G0")),
                ("weekly".into(), r("S2")),
                ("special".into(), r("B2")),
                ("champ".into(), r("B2")),
            ]),
        }
    }
}

fn rating(field: &str, v: &Value) -> io::Result<String> {
    match v.as_str() {
        Some(s) if RATINGS.contains(&s) => Ok(s.to_string()),
        _ => Err(bad(format!("`{field}` : note invalide (B0 … P3 ou vide)"))),
    }
}

fn ratings_patch(field: &str, v: &Value, into: &mut Ratings) -> io::Result<()> {
    let Value::Object(m) = v else {
        return Err(bad(format!("`{field}` : objet {{sr, dr}} attendu")));
    };
    for (k, v) in m {
        match k.as_str() {
            "sr" => into.sr = rating(&format!("{field}.sr"), v)?,
            "dr" => into.dr = rating(&format!("{field}.dr"), v)?,
            other => return Err(bad(format!("`{field}.{other}` inconnu"))),
        }
    }
    Ok(())
}

fn subset(field: &str, v: &Value, allowed: &[&str]) -> io::Result<Vec<String>> {
    let Value::Array(items) = v else {
        return Err(bad(format!("`{field}` : liste attendue")));
    };
    let mut out: Vec<String> = Vec::new();
    for it in items {
        match it.as_str() {
            Some(s) if allowed.contains(&s) => {
                if !out.iter().any(|x| x == s) {
                    out.push(s.to_string());
                }
            }
            _ => return Err(bad(format!("`{field}` : valeur inconnue {it}"))),
        }
    }
    if out.is_empty() {
        return Err(bad(format!("`{field}` : garde au moins une valeur")));
    }
    // Canonical order.
    out.sort_by_key(|s| allowed.iter().position(|a| a == s));
    Ok(out)
}

impl RaceSettings {
    /// Apply a `PUT /api/races/settings` body (an object of known keys only).
    pub fn apply_patch(&mut self, body: &[u8]) -> io::Result<()> {
        let value: Value =
            serde_json::from_slice(body).map_err(|e| bad(format!("réglages invalides : {e}")))?;
        let Value::Object(map) = value else {
            return Err(bad("réglages invalides : un objet JSON est attendu"));
        };
        let mut next = self.clone();
        for (key, v) in &map {
            match key.as_str() {
                "leads" => {
                    let Value::Array(items) = v else {
                        return Err(bad("`leads` : liste attendue"));
                    };
                    let mut leads = BTreeSet::new();
                    for it in items {
                        match it.as_u64() {
                            Some(n) if LEAD_CHOICES.contains(&(n as u32)) && n <= 60 => {
                                leads.insert(n as u32);
                            }
                            _ => return Err(bad(format!("`leads` : délai non proposé {it}"))),
                        }
                    }
                    next.leads = leads.into_iter().rev().collect();
                }
                "levels" => next.levels = subset("levels", v, &LEVELS)?,
                "cats" => next.cats = subset("cats", v, &CATS)?,
                "hour_fix" => match v.as_i64() {
                    Some(h) if (-2..=2).contains(&h) => next.hour_fix = h as i32,
                    _ => return Err(bad("`hour_fix` : entier de -2 à 2 attendu")),
                },
                "profile" => ratings_patch("profile", v, &mut next.profile)?,
                "req" => {
                    let Value::Object(m) = v else {
                        return Err(bad("`req` : objet attendu"));
                    };
                    for (k, r) in m {
                        if !REQ_KEYS.contains(&k.as_str()) {
                            return Err(bad(format!("`req.{k}` inconnu")));
                        }
                        let entry = next.req.entry(k.clone()).or_default();
                        ratings_patch(&format!("req.{k}"), r, entry)?;
                    }
                }
                other => return Err(bad(format!("réglage inconnu : `{other}`"))),
            }
        }
        *self = next;
        Ok(())
    }

    /// Settings read from disk: anything unknown or invalid falls back to the default.
    fn from_disk(value: Value) -> Self {
        let mut s = Self::default();
        if let Value::Object(map) = value {
            for (k, v) in map {
                let mut one = Map::new();
                one.insert(k.clone(), v);
                let body = serde_json::to_vec(&Value::Object(one)).unwrap_or_default();
                if let Err(e) = s.apply_patch(&body) {
                    tracing::warn!("réglage des courses `{k}` ignoré : {e}");
                }
            }
        }
        s
    }
}

#[derive(Debug, Default)]
struct Inner {
    picks: Vec<Pick>,
    settings: RaceSettings,
    /// Reminders already sent: `<pick_id>#<lead|eve|day>`.
    fired: BTreeSet<String>,
}

/// The files of `<data-dir>/races/`, behind one lock.
#[derive(Debug)]
pub struct RaceStore {
    dir: PathBuf,
    inner: Mutex<Inner>,
}

fn read_json(path: &Path) -> Option<Value> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
        Err(e) => {
            tracing::warn!("{} illisible : {e}", path.display());
            return None;
        }
    };
    let json = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    match serde_json::from_slice(json) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("{} n'est pas du JSON valide ({e}) : ignoré", path.display());
            None
        }
    }
}

fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    let mut json = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    json.push(b'\n');
    let tmp = dir.join(format!("{name}.tmp"));
    fs::write(&tmp, json)?;
    fs::rename(&tmp, dir.join(name))
}

impl RaceStore {
    /// Opens `<data-dir>/races/` (created on the first write only).
    pub fn open(data_dir: &Path) -> Self {
        let dir = data_dir.join(RACES_DIR);
        let picks = read_json(&dir.join(PICKS_FILE))
            .and_then(|v| match v {
                Value::Array(items) => Some(items),
                _ => None,
            })
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| {
                let p: Pick = serde_json::from_value(v).ok()?;
                p.validate().ok()?;
                Some(p)
            })
            .take(MAX_PICKS)
            .collect();
        let settings = read_json(&dir.join(SETTINGS_FILE))
            .map(RaceSettings::from_disk)
            .unwrap_or_default();
        let fired = read_json(&dir.join(FIRED_FILE))
            .and_then(|v| serde_json::from_value::<BTreeSet<String>>(v).ok())
            .unwrap_or_default();
        Self {
            dir,
            inner: Mutex::new(Inner {
                picks,
                settings,
                fired,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }

    fn prune(inner: &mut Inner, now_ms: i64) -> bool {
        let before = inner.picks.len();
        inner
            .picks
            .retain(|p| p.start_ms > now_ms - KEEP_AFTER_START_MS);
        inner
            .picks
            .sort_by(|a, b| a.start_ms.cmp(&b.start_ms).then(a.id.cmp(&b.id)));
        let ids: BTreeSet<&str> = inner.picks.iter().map(|p| p.id.as_str()).collect();
        let fired_before = inner.fired.len();
        inner
            .fired
            .retain(|k| k.rsplit_once('#').is_some_and(|(id, _)| ids.contains(id)));
        before != inner.picks.len() || fired_before != inner.fired.len()
    }

    fn save_picks(&self, inner: &Inner) -> io::Result<()> {
        write_json(&self.dir, PICKS_FILE, &inner.picks)?;
        write_json(&self.dir, FIRED_FILE, &inner.fired)
    }

    /// The picks still useful (started less than 3 h ago), by start.
    pub fn picks(&self, now_ms: i64) -> Vec<Pick> {
        let mut g = self.lock();
        if Self::prune(&mut g, now_ms) {
            if let Err(e) = self.save_picks(&g) {
                tracing::warn!("courses : enregistrement impossible — {e}");
            }
        }
        g.picks.clone()
    }

    /// Add or replace a pick. `Ok(true)` when it is new.
    pub fn put_pick(&self, pick: Pick, now_ms: i64) -> io::Result<bool> {
        pick.validate()?;
        let mut g = self.lock();
        Self::prune(&mut g, now_ms);
        let new = match g.picks.iter_mut().find(|p| p.id == pick.id) {
            Some(p) => {
                *p = pick;
                false
            }
            None => {
                if g.picks.len() >= MAX_PICKS {
                    return Err(io::Error::new(
                        io::ErrorKind::StorageFull,
                        format!("au plus {MAX_PICKS} courses choisies : retires-en d'abord"),
                    ));
                }
                g.picks.push(pick);
                true
            }
        };
        g.picks
            .sort_by(|a, b| a.start_ms.cmp(&b.start_ms).then(a.id.cmp(&b.id)));
        self.save_picks(&g)?;
        Ok(new)
    }

    /// Remove a pick. `Ok(false)` when there was none.
    pub fn delete_pick(&self, id: &str, now_ms: i64) -> io::Result<bool> {
        let mut g = self.lock();
        let before = g.picks.len();
        g.picks.retain(|p| p.id != id);
        let removed = g.picks.len() != before;
        Self::prune(&mut g, now_ms);
        if removed {
            self.save_picks(&g)?;
        }
        Ok(removed)
    }

    pub fn settings(&self) -> RaceSettings {
        self.lock().settings.clone()
    }

    /// Apply a settings patch. A change of the hour correction moves the
    /// racecontrol.gg picks with it (they were picked on the old time).
    pub fn patch_settings(&self, body: &[u8], now_ms: i64) -> io::Result<RaceSettings> {
        let mut g = self.lock();
        let mut next = g.settings.clone();
        next.apply_patch(body)?;
        let delta = i64::from(next.hour_fix - g.settings.hour_fix);
        write_json(&self.dir, SETTINGS_FILE, &next)?;
        g.settings = next.clone();
        if delta != 0 {
            for p in g.picks.iter_mut() {
                if p.src == "rc" && p.cat == "daily" {
                    p.start_ms += delta * 3_600_000;
                    p.id = pick_id(&p.race_id, p.start_ms);
                }
            }
            Self::prune(&mut g, now_ms);
            self.save_picks(&g)?;
        }
        Ok(next)
    }

    /// Picks, reminder delays and reminders already sent, for the reminder task.
    pub fn reminder_input(&self) -> (Vec<Pick>, Vec<u32>, BTreeSet<String>) {
        let g = self.lock();
        (g.picks.clone(), g.settings.leads.clone(), g.fired.clone())
    }

    /// Remember that these reminders were sent (persisted).
    pub fn mark_fired(&self, keys: &[String]) {
        if keys.is_empty() {
            return;
        }
        let mut g = self.lock();
        for k in keys {
            g.fired.insert(k.clone());
        }
        if let Err(e) = write_json(&self.dir, FIRED_FILE, &g.fired) {
            tracing::warn!("courses : rappels envoyés non enregistrés — {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn tmp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "corde-races-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    const NOW: i64 = 1_762_891_800_000; // 2026-11-11 20:10 UTC

    pub(crate) fn pick(race: &str, start: i64) -> Pick {
        Pick {
            id: pick_id(race, start),
            race_id: race.into(),
            cat: "daily".into(),
            src: "rc".into(),
            name: "One Stint Sprint".into(),
            level: Some("advanced".into()),
            track: "Monza".into(),
            duration: "55 min".into(),
            image: Some("https://cdn.example/monza.webp".into()),
            start_ms: start,
            eve_ms: None,
        }
    }

    #[test]
    fn picks_are_validated() {
        let p = pick("one-stint-sprint-advanced", NOW + 900_000);
        assert!(p.validate().is_ok());
        let body = serde_json::to_vec(&p).unwrap();
        assert!(Pick::from_request(&p.id, &body).is_ok());
        assert!(Pick::from_request("other@1", &body).is_err());
        type Breaker = Box<dyn Fn(&mut Pick)>;
        let bad_ones: Vec<Breaker> = vec![
            Box::new(|p| p.race_id = "../x".into()),
            Box::new(|p| p.race_id = "A".into()),
            Box::new(|p| p.id = "x@1".into()),
            Box::new(|p| p.cat = "hourly".into()),
            Box::new(|p| p.src = "web".into()),
            Box::new(|p| p.level = Some("pro".into())),
            Box::new(|p| p.name = String::new()),
            Box::new(|p| p.name = "x".repeat(121)),
            Box::new(|p| p.track = "a\u{0}b".into()),
            Box::new(|p| p.image = Some("http://cdn.example/a.png".into())),
            Box::new(|p| p.image = Some("javascript:alert(1)".into())),
            Box::new(|p| p.image = Some("https://a.example/\"onerror=x".into())),
            Box::new(|p| p.eve_ms = Some(p.start_ms - 1000)),
        ];
        for f in bad_ones {
            let mut q = pick("one-stint-sprint-advanced", NOW + 900_000);
            f(&mut q);
            assert!(q.validate().is_err(), "{q:?}");
        }
        let mut sp = pick("special-sebring-1000-miles-2026-11-21", NOW + 86_400_000);
        sp.cat = "special".into();
        sp.src = "lt".into();
        sp.eve_ms = Some(sp.start_ms - 16 * 3_600_000);
        assert!(sp.validate().is_ok());
        sp.eve_ms = Some(sp.start_ms + 1);
        assert!(sp.validate().is_err());
        let extra = br#"{"id":"a@1700000000000","race_id":"a","cat":"daily","src":"rc","name":"n","level":null,"track":"t","start_ms":1700000000000,"token":"x"}"#;
        assert!(
            Pick::from_request("a@1700000000000", extra).is_err(),
            "unknown field"
        );
        assert!(valid_pick_id("one-stint-sprint-advanced@1762892700000"));
        assert!(!valid_pick_id("one@x"));
        assert!(!valid_pick_id("one"));
    }

    #[test]
    fn picks_are_bounded_pruned_and_persisted() {
        let dir = tmp_dir("picks");
        let store = RaceStore::open(&dir);
        assert!(store.picks(NOW).is_empty());
        assert!(!dir.exists(), "reading creates nothing");
        for i in 0..MAX_PICKS as i64 {
            assert!(store
                .put_pick(pick("r", NOW + (i + 1) * 60_000), NOW)
                .unwrap());
        }
        assert!(
            !store.put_pick(pick("r", NOW + 60_000), NOW).unwrap(),
            "replaced"
        );
        let err = store
            .put_pick(pick("r", NOW + 999 * 60_000), NOW)
            .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::StorageFull);
        // Reopened from disk.
        let again = RaceStore::open(&dir);
        assert_eq!(again.picks(NOW).len(), MAX_PICKS);
        // Three hours after a start, the pick is gone.
        let later = NOW + 60_000 + KEEP_AFTER_START_MS + 1;
        let left = again.picks(later);
        assert_eq!(left.len(), MAX_PICKS - 1);
        assert!(left.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
        assert!(again.delete_pick(&left[0].id, later).unwrap());
        assert!(!again.delete_pick(&left[0].id, later).unwrap());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn settings_patch_is_strict() {
        let mut s = RaceSettings::default();
        assert_eq!(s.leads, vec![15, 5]);
        assert_eq!(s.req["weekly"].sr, "S2");
        s.apply_patch(br#"{"leads":[0,60,5,5],"hour_fix":-1,"profile":{"sr":"S1"},"req":{"champ":{"dr":"S0"}},"cats":["champ","daily"]}"#)
            .unwrap();
        assert_eq!(s.leads, vec![60, 5, 0]);
        assert_eq!(s.hour_fix, -1);
        assert_eq!(s.profile.sr, "S1");
        assert_eq!(
            s.req["champ"],
            Ratings {
                sr: "B2".into(),
                dr: "S0".into()
            }
        );
        assert_eq!(s.cats, vec!["daily", "champ"]);
        let before = s.clone();
        for bad_body in [
            &br#"[]"#[..],
            br#"{"leads":[7]}"#,
            br#"{"leads":"15"}"#,
            br#"{"levels":[]}"#,
            br#"{"levels":["pro"]}"#,
            br#"{"hour_fix":3}"#,
            br#"{"hour_fix":0.5}"#,
            br#"{"profile":{"sr":"Z9"}}"#,
            br#"{"profile":{"name":"me"}}"#,
            br#"{"req":{"hourly":{"sr":"B0"}}}"#,
            br#"{"sound":true}"#,
        ] {
            assert!(
                s.apply_patch(bad_body).is_err(),
                "{}",
                String::from_utf8_lossy(bad_body)
            );
            assert_eq!(s, before, "a rejected patch changes nothing");
        }
    }

    #[test]
    fn hour_fix_moves_the_racecontrol_picks() {
        let dir = tmp_dir("hourfix");
        let store = RaceStore::open(&dir);
        let start = NOW + 3_600_000;
        store
            .put_pick(pick("one-stint-sprint-advanced", start), NOW)
            .unwrap();
        let mut weekly = pick("weekly-gt3", start);
        weekly.src = "lt".into();
        weekly.cat = "weekly".into();
        weekly.level = None;
        store.put_pick(weekly.clone(), NOW).unwrap();
        store.patch_settings(br#"{"hour_fix":1}"#, NOW).unwrap();
        let picks = store.picks(NOW);
        let rc = picks.iter().find(|p| p.src == "rc").unwrap();
        assert_eq!(rc.start_ms, start + 3_600_000);
        assert_eq!(
            rc.id,
            pick_id("one-stint-sprint-advanced", start + 3_600_000)
        );
        assert!(picks.contains(&weekly), "lmu.tools times are not corrected");
        let on_disk: Value =
            serde_json::from_slice(&fs::read(dir.join("races/settings.json")).unwrap()).unwrap();
        assert_eq!(on_disk["hour_fix"], json!(1));
        assert_eq!(RaceStore::open(&dir).settings().hour_fix, 1);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn broken_files_mean_defaults() {
        let dir = tmp_dir("broken");
        fs::create_dir_all(dir.join(RACES_DIR)).unwrap();
        fs::write(dir.join("races/picks.json"), "not json").unwrap();
        fs::write(
            dir.join("races/settings.json"),
            r#"{"leads":[7],"hour_fix":2,"theme":"x"}"#,
        )
        .unwrap();
        let store = RaceStore::open(&dir);
        assert!(store.picks(NOW).is_empty());
        let s = store.settings();
        assert_eq!(s.leads, vec![15, 5], "invalid key → default");
        assert_eq!(s.hour_fix, 2, "valid key kept");
        let _ = fs::remove_dir_all(&dir);
    }
}
