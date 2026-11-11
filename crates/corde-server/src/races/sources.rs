//! The two public web pages of the online race schedule, read on demand.
//!
//! A browser cannot read these sites (no CORS headers), so the server does it
//! for the « Courses » page — and does nothing else with them: the HTML is
//! handed to the UI as text, which parses it (`packages/ui/src/races/parse.ts`).
//!
//! What may be fetched is a **closed list in the code** ([`SOURCES`]): no
//! address ever comes from a request. Each source is a `GET` without cookies
//! and without any data about the driver, with:
//!
//! * a cache: 10 min for racecontrol.gg, 60 min for lmu.tools; a forced reload
//!   (`?force=1`) at most every 60 s; after a failure, no new attempt for 60 s;
//! * a 20 s deadline and a 3 MiB limit on the answer;
//! * `User-Agent: Corde/<version> (+https://github.com/dultra123/corde)`,
//!   `Accept: text/html`, `Accept-Language: en-GB,en;q=0.8`;
//! * at most [`MAX_REDIRECTS`] redirections, followed only to `https://` on the
//!   same site (`racecontrol.gg` ↔ `www.racecontrol.gg`).
//!
//! Nothing is fetched while the `online_schedule` setting is off. When the UI
//! finds no race in a page, it says so ([`RaceSources::keep_unreadable`]) and
//! the copy the server holds is kept in `<data-dir>/diagnostics/races/<id>.html`
//! (one file per source) to adapt the parser if a site changed.

use crate::state::SERVER_VERSION;
use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// One page of the schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDef {
    /// `racecontrol` | `lmutools` (also the name of the diagnostic file).
    pub id: &'static str,
    pub url: &'static str,
    /// The site; redirections may only stay on it (or its `www.` form).
    pub host: &'static str,
    /// How long a page read successfully is served from the cache.
    pub ttl: Duration,
}

/// The only addresses Corde ever reads on the web.
pub const SOURCES: [SourceDef; 2] = [
    SourceDef {
        id: "racecontrol",
        url: "https://racecontrol.gg/",
        host: "racecontrol.gg",
        ttl: Duration::from_secs(10 * 60),
    },
    SourceDef {
        id: "lmutools",
        url: "https://lmu.tools/schedule",
        host: "lmu.tools",
        ttl: Duration::from_secs(60 * 60),
    },
];

/// A forced reload, or a new attempt after a failure, waits at least this long.
pub const MIN_RETRY: Duration = Duration::from_secs(60);
/// Deadline of one fetch (connection, redirections and body included).
pub const TIMEOUT: Duration = Duration::from_secs(20);
/// Largest page accepted.
pub const MAX_BYTES: u64 = 3 * 1024 * 1024;
/// Redirections followed (on the same site only).
pub const MAX_REDIRECTS: usize = 3;
/// Folder of the copies of unreadable pages, inside the data directory.
pub const DIAGNOSTICS_DIR: &str = "diagnostics/races";

pub fn source(id: &str) -> Option<&'static SourceDef> {
    SOURCES.iter().find(|s| s.id == id)
}

/// `Corde/0.4.0 (+https://github.com/dultra123/corde)`.
pub fn user_agent() -> String {
    format!("Corde/{SERVER_VERSION} (+https://github.com/dultra123/corde)")
}

/// How a page is fetched: the real HTTP client, or a stand-in in the tests.
pub type FetchFn = Arc<dyn Fn(&SourceDef) -> Result<String, String> + Send + Sync>;

/// What `GET /api/races/sources` says about one source.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SourceSnapshot {
    pub id: &'static str,
    pub url: &'static str,
    /// `disabled` (setting off: nothing fetched) · `ok` · `error` (last attempt
    /// failed; `html` is then the last good copy, if any).
    pub state: &'static str,
    /// When the page in `html` was read (ms since the epoch).
    pub fetched_at_ms: Option<i64>,
    /// When the last attempt was made.
    pub attempted_at_ms: Option<i64>,
    pub error: Option<String>,
    /// The page, as text. Never put in a DOM by the UI: it is parsed.
    #[serde(serialize_with = "ser_text")]
    pub html: Option<Arc<str>>,
}

fn ser_text<S: serde::Serializer>(v: &Option<Arc<str>>, s: S) -> Result<S::Ok, S::Error> {
    match v {
        Some(t) => s.serialize_some(&**t),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Default)]
struct Slot {
    html: Option<Arc<str>>,
    fetched_at_ms: Option<i64>,
    attempted: Option<Instant>,
    attempted_at_ms: Option<i64>,
    last_ok: bool,
    error: Option<String>,
    /// `fetched_at_ms` of the copy already kept as a diagnostic.
    kept: Option<i64>,
}

/// `true` when a source must be fetched now. `age` = time since the last
/// attempt (`None`: never tried).
pub fn needs_fetch(age: Option<Duration>, last_ok: bool, ttl: Duration, force: bool) -> bool {
    match age {
        None => true,
        Some(age) if !last_ok => age >= MIN_RETRY,
        Some(age) => age >= ttl || (force && age >= MIN_RETRY),
    }
}

/// The two sources, their cache and the way to fetch them.
pub struct RaceSources {
    fetch: Mutex<FetchFn>,
    slots: [tokio::sync::Mutex<Slot>; 2],
}

impl Default for RaceSources {
    fn default() -> Self {
        Self::new(Arc::new(fetch_page))
    }
}

impl RaceSources {
    pub fn new(fetch: FetchFn) -> Self {
        Self {
            fetch: Mutex::new(fetch),
            slots: Default::default(),
        }
    }

    /// Replace the way pages are fetched (tests: no network).
    pub fn set_fetcher(&self, fetch: FetchFn) {
        if let Ok(mut f) = self.fetch.lock() {
            *f = fetch;
        }
    }

    fn fetcher(&self) -> FetchFn {
        match self.fetch.lock() {
            Ok(f) => Arc::clone(&f),
            Err(p) => Arc::clone(&p.into_inner()),
        }
    }

    /// Every source, fetched first when its cache is stale. With `enabled`
    /// false nothing is fetched and no page is returned.
    pub async fn snapshot(&self, enabled: bool, force: bool, now_ms: i64) -> Vec<SourceSnapshot> {
        // The two sites are independent: fetch them side by side.
        let [a, b] = &SOURCES;
        let [sa, sb] = &self.slots;
        let (ra, rb) = tokio::join!(
            self.one(a, sa, enabled, force, now_ms),
            self.one(b, sb, enabled, force, now_ms)
        );
        vec![ra, rb]
    }

    async fn one(
        &self,
        def: &'static SourceDef,
        slot: &tokio::sync::Mutex<Slot>,
        enabled: bool,
        force: bool,
        now_ms: i64,
    ) -> SourceSnapshot {
        if !enabled {
            return SourceSnapshot {
                id: def.id,
                url: def.url,
                state: "disabled",
                fetched_at_ms: None,
                attempted_at_ms: None,
                error: None,
                html: None,
            };
        }
        // Held during the fetch: two pages opened at once wait for one fetch.
        let mut s = slot.lock().await;
        let age = s.attempted.map(|t| t.elapsed());
        if needs_fetch(age, s.last_ok, def.ttl, force) {
            let fetch = self.fetcher();
            let started = Instant::now();
            let result = tokio::task::spawn_blocking(move || fetch(def))
                .await
                .unwrap_or_else(|e| Err(format!("tâche interrompue : {e}")));
            s.attempted = Some(Instant::now());
            s.attempted_at_ms = Some(now_ms);
            match result {
                Ok(html) => {
                    tracing::info!(
                        "planning des courses : {} lu ({} octets, {} ms)",
                        def.url,
                        html.len(),
                        started.elapsed().as_millis()
                    );
                    s.html = Some(html.into());
                    s.fetched_at_ms = Some(now_ms);
                    s.last_ok = true;
                    s.error = None;
                }
                Err(e) => {
                    tracing::warn!("planning des courses : {} illisible — {e}", def.url);
                    s.last_ok = false;
                    s.error = Some(e);
                }
            }
        }
        SourceSnapshot {
            id: def.id,
            url: def.url,
            state: if s.last_ok { "ok" } else { "error" },
            fetched_at_ms: s.fetched_at_ms,
            attempted_at_ms: s.attempted_at_ms,
            error: s.error.clone(),
            html: s.html.clone(),
        }
    }

    /// The UI found no race in the page of `id`: keep the copy the server
    /// holds (the one it served) in `<data-dir>/diagnostics/races/<id>.html`,
    /// replacing the previous one. Returns the path written, `None` when there
    /// is no copy or this copy was already kept.
    pub async fn keep_unreadable(&self, id: &str, data_dir: &Path) -> io::Result<Option<PathBuf>> {
        let Some(idx) = SOURCES.iter().position(|s| s.id == id) else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("source inconnue : {id}"),
            ));
        };
        let (html, fetched) = {
            let mut s = self.slots[idx].lock().await;
            let (Some(html), Some(fetched)) = (s.html.clone(), s.fetched_at_ms) else {
                return Ok(None);
            };
            if s.kept == Some(fetched) {
                return Ok(None);
            }
            s.kept = Some(fetched);
            (html, fetched)
        };
        let id = SOURCES[idx].id;
        let dir = data_dir.join(DIAGNOSTICS_DIR);
        let path = dir.join(format!("{id}.html"));
        let path2 = path.clone();
        tokio::task::spawn_blocking(move || -> io::Result<()> {
            std::fs::create_dir_all(&dir)?;
            let tmp = dir.join(format!("{id}.html.tmp"));
            std::fs::write(&tmp, html.as_bytes())?;
            std::fs::rename(&tmp, &path2)
        })
        .await
        .map_err(io::Error::other)??;
        tracing::warn!(
            "planning des courses : aucune course reconnue dans la page {} lue à {fetched} ; copie gardée dans {}",
            SOURCES[idx].url,
            path.display()
        );
        Ok(Some(path))
    }
}

/// `true` when `url` is an `https://` address on `host` (or `www.<host>`, or
/// `host` without `www.`). Used for the redirections.
pub fn same_site(url: &str, host: &str) -> bool {
    let Some(rest) = url.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return false;
    }
    let name = authority
        .rsplit_once(':')
        .map_or(
            authority,
            |(n, port)| if port == "443" { n } else { "\u{0}" },
        )
        .to_ascii_lowercase();
    let bare = host.trim_start_matches("www.");
    name == bare || name == format!("www.{bare}")
}

/// Resolves a `Location` header against the page that sent it (absolute,
/// `//host/path` or `/path`). `None` for anything else.
fn resolve_location(base: &str, location: &str) -> Option<String> {
    let location = location.trim();
    if location.starts_with("https://") || location.starts_with("http://") {
        return Some(location.to_string());
    }
    if let Some(rest) = location.strip_prefix("//") {
        return Some(format!("https://{rest}"));
    }
    if location.starts_with('/') {
        let rest = base.strip_prefix("https://")?;
        let authority = rest.split('/').next()?;
        return Some(format!("https://{authority}{location}"));
    }
    None
}

/// The real fetch: `GET` the page with the headers of the module comment.
pub fn fetch_page(def: &SourceDef) -> Result<String, String> {
    http_get(def.url, def.host, None)
}

/// `GET url`, following up to [`MAX_REDIRECTS`] redirections on the same site.
/// `proxy`: `None` = the system's proxy settings (environment), as any program.
pub fn http_get(
    url: &str,
    host: &str,
    proxy: Option<Option<ureq::Proxy>>,
) -> Result<String, String> {
    let mut builder = ureq::config::Config::builder()
        .timeout_global(Some(TIMEOUT))
        .http_status_as_error(false)
        .max_redirects(0)
        .user_agent(user_agent())
        .accept("text/html");
    if let Some(p) = proxy {
        builder = builder.proxy(p);
    }
    let agent: ureq::Agent = builder.build().new_agent();
    let deadline = Instant::now() + TIMEOUT;
    let mut current = url.to_string();
    for _ in 0..=MAX_REDIRECTS {
        if Instant::now() >= deadline {
            return Err("délai dépassé".into());
        }
        let mut resp = agent
            .get(&current)
            .header("Accept-Language", "en-GB,en;q=0.8")
            .call()
            .map_err(|e| format!("requête impossible : {e}"))?;
        let status = resp.status().as_u16();
        if (300..400).contains(&status) {
            let location = resp
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let next = resolve_location(&current, location)
                .filter(|n| same_site(n, host))
                .ok_or_else(|| {
                    format!("redirection {status} refusée (hors du site {host}) : {location}")
                })?;
            current = next;
            continue;
        }
        if status >= 400 {
            return Err(format!("le site a répondu {status}"));
        }
        let bytes = resp
            .body_mut()
            .with_config()
            .limit(MAX_BYTES)
            .read_to_vec()
            .map_err(|e| format!("page illisible : {e}"))?;
        return Ok(String::from_utf8_lossy(&bytes).into_owned());
    }
    Err(format!("trop de redirections (plus de {MAX_REDIRECTS})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn the_list_is_closed_and_https() {
        assert_eq!(SOURCES.len(), 2);
        for s in SOURCES {
            assert!(s.url.starts_with("https://"), "{}", s.url);
            assert!(same_site(s.url, s.host), "{}", s.url);
            assert!(s.ttl >= Duration::from_secs(600));
        }
        assert_eq!(
            source("racecontrol").unwrap().url,
            "https://racecontrol.gg/"
        );
        assert_eq!(
            source("lmutools").unwrap().url,
            "https://lmu.tools/schedule"
        );
        assert!(source("https://evil.example").is_none());
        assert!(user_agent().starts_with("Corde/"));
        assert!(user_agent().ends_with("(+https://github.com/dultra123/corde)"));
    }

    #[test]
    fn cache_rules() {
        let ten = Duration::from_secs(600);
        let s = Duration::from_secs;
        assert!(needs_fetch(None, false, ten, false), "never fetched");
        assert!(!needs_fetch(Some(s(30)), true, ten, false));
        assert!(
            !needs_fetch(Some(s(30)), true, ten, true),
            "force ≤ every 60 s"
        );
        assert!(needs_fetch(Some(s(61)), true, ten, true));
        assert!(!needs_fetch(Some(s(599)), true, ten, false));
        assert!(needs_fetch(Some(s(600)), true, ten, false));
        assert!(
            !needs_fetch(Some(s(59)), false, ten, true),
            "failure: wait 60 s"
        );
        assert!(needs_fetch(Some(s(60)), false, ten, false));
    }

    #[test]
    fn redirections_stay_on_the_site() {
        assert!(same_site("https://racecontrol.gg/", "racecontrol.gg"));
        assert!(same_site("https://www.racecontrol.gg/en", "racecontrol.gg"));
        assert!(same_site("https://RaceControl.gg:443/x", "racecontrol.gg"));
        assert!(!same_site("http://racecontrol.gg/", "racecontrol.gg"));
        assert!(!same_site(
            "https://racecontrol.gg.evil.example/",
            "racecontrol.gg"
        ));
        assert!(!same_site(
            "https://evil.example/racecontrol.gg",
            "racecontrol.gg"
        ));
        assert!(!same_site(
            "https://racecontrol.gg@evil.example/",
            "racecontrol.gg"
        ));
        assert!(!same_site("https://racecontrol.gg:8443/", "racecontrol.gg"));
        assert!(!same_site("https://raceos.gg/", "racecontrol.gg"));
        assert_eq!(
            resolve_location("https://lmu.tools/schedule", "/schedule/"),
            Some("https://lmu.tools/schedule/".into())
        );
        assert_eq!(resolve_location("https://lmu.tools/schedule", "next"), None);
    }

    #[tokio::test]
    async fn disabled_fetches_nothing_and_the_cache_holds() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&calls);
        let sources = RaceSources::new(Arc::new(move |def: &SourceDef| {
            c.fetch_add(1, Ordering::SeqCst);
            Ok(format!("<html>{}</html>", def.id))
        }));
        let snap = sources.snapshot(false, true, 1).await;
        assert!(snap
            .iter()
            .all(|s| s.state == "disabled" && s.html.is_none()));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "setting off: no fetch");

        let snap = sources.snapshot(true, false, 1000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert_eq!(snap[0].id, "racecontrol");
        assert_eq!(snap[0].state, "ok");
        assert_eq!(snap[0].fetched_at_ms, Some(1000));
        assert_eq!(snap[1].html.as_deref(), Some("<html>lmutools</html>"));

        // Cached, forced or not (forced reloads wait 60 s).
        sources.snapshot(true, false, 2000).await;
        sources.snapshot(true, true, 3000).await;
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn a_failure_keeps_the_last_good_copy() {
        let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let f = Arc::clone(&fail);
        let sources = RaceSources::new(Arc::new(move |_: &SourceDef| {
            if f.load(Ordering::SeqCst) {
                Err("le site a répondu 503".into())
            } else {
                Ok("<p>page</p>".into())
            }
        }));
        sources.snapshot(true, false, 10).await;
        fail.store(true, Ordering::SeqCst);
        // Expire the cache by hand.
        for slot in &sources.slots {
            slot.lock().await.attempted = Instant::now().checked_sub(Duration::from_secs(3601));
        }
        let snap = sources.snapshot(true, false, 20).await;
        for s in &snap {
            assert_eq!(s.state, "error");
            assert_eq!(s.error.as_deref(), Some("le site a répondu 503"));
            assert_eq!(s.html.as_deref(), Some("<p>page</p>"), "last good copy");
            assert_eq!(s.fetched_at_ms, Some(10));
            assert_eq!(s.attempted_at_ms, Some(20));
        }
    }

    #[tokio::test]
    async fn an_unreadable_page_is_kept_once() {
        let dir = std::env::temp_dir().join(format!(
            "corde-races-diag-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        let sources = RaceSources::new(Arc::new(|_: &SourceDef| Ok("<p>maintenance</p>".into())));
        assert_eq!(
            sources.keep_unreadable("racecontrol", &dir).await.unwrap(),
            None
        );
        sources.snapshot(true, false, 5).await;
        let path = sources
            .keep_unreadable("racecontrol", &dir)
            .await
            .unwrap()
            .expect("written");
        assert_eq!(path, dir.join("diagnostics/races/racecontrol.html"));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "<p>maintenance</p>"
        );
        assert_eq!(
            sources.keep_unreadable("racecontrol", &dir).await.unwrap(),
            None
        );
        assert!(sources.keep_unreadable("../x", &dir).await.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The real client against a local HTTP server: headers, no cookie, size
    /// limit, redirections.
    #[test]
    fn http_client_sends_the_documented_headers() {
        use std::io::{Read, Write};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut requests = Vec::new();
            for answer in [
                // 1: a page, with a cookie the client must not send back
                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nSet-Cookie: a=b\r\nContent-Length: 11\r\nConnection: close\r\n\r\n<p>page</p>".to_string(),
                // 2: second request (must carry no cookie)
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".to_string(),
                // 3: too large
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    MAX_BYTES + 10
                ),
                // 4: a redirection to another site
                "HTTP/1.1 302 Found\r\nLocation: https://evil.example/\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
            ] {
                let (mut sock, _) = listener.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = sock.read(&mut buf).unwrap();
                requests.push(String::from_utf8_lossy(&buf[..n]).to_string());
                let _ = sock.write_all(answer.as_bytes());
                if answer.contains(&format!("{}", MAX_BYTES + 10)) {
                    let chunk = vec![b'x'; 64 * 1024];
                    for _ in 0..=(MAX_BYTES / chunk.len() as u64) {
                        if sock.write_all(&chunk).is_err() {
                            break;
                        }
                    }
                }
            }
            requests
        });
        let url = format!("http://{addr}/schedule");
        let host = "127.0.0.1";
        assert_eq!(http_get(&url, host, Some(None)).unwrap(), "<p>page</p>");
        assert_eq!(http_get(&url, host, Some(None)).unwrap(), "ok");
        let err = http_get(&url, host, Some(None)).unwrap_err();
        assert!(err.contains("illisible"), "{err}");
        let err = http_get(&url, host, Some(None)).unwrap_err();
        assert!(err.contains("redirection 302 refusée"), "{err}");
        let requests = server.join().unwrap();
        let first = requests[0].to_ascii_lowercase();
        assert!(first.starts_with("get /schedule http/1.1"), "{first}");
        assert!(
            first.contains(&format!(
                "user-agent: {}",
                user_agent().to_ascii_lowercase()
            )),
            "{first}"
        );
        assert!(first.contains("accept: text/html\r\n"), "{first}");
        assert!(first.contains("accept-language: en-gb,en;q=0.8"), "{first}");
        for r in &requests {
            assert!(!r.to_ascii_lowercase().contains("cookie"), "{r}");
        }
    }
}
