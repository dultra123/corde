//! Shared state between the source thread, the WebSocket clients and the HTTP API.

use crate::config::{LlmKind, Role};
use crate::layouts::LayoutStore;
use crate::results_import::ResultsImporter;
use crate::settings::SettingsStore;
use corde_engineer::feedback::{AnthropicExtractor, DictionaryExtractor, SymptomExtractor};
use corde_engineer::Knowledge;
use corde_protocol::{GameEvent, LapRecord, ServerMessage, SourceStatus, TelemetryFrame};
use corde_store::{RecorderHandle, Store};
use serde::Serialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::Instant;
use tokio::sync::{broadcast, watch};

/// Server version advertised in `Hello`.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// `{"type":"frame", ...TelemetryFrame}` without cloning the frame.
#[derive(Serialize)]
struct FrameMessage<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    #[serde(flatten)]
    frame: &'a TelemetryFrame,
}

/// The latest frame plus its lazily-built JSON wire form (built at most once,
/// only if a client needs it: at 100 Hz input and 20 Hz output most frames are
/// never serialised).
#[derive(Debug)]
pub struct FrameSlot {
    pub frame: TelemetryFrame,
    json: OnceLock<Arc<str>>,
}

impl FrameSlot {
    pub fn new(frame: TelemetryFrame) -> Self {
        Self {
            frame,
            json: OnceLock::new(),
        }
    }

    /// `ServerMessage::Frame` as a JSON text.
    pub fn json(&self) -> Arc<str> {
        self.json
            .get_or_init(|| {
                serde_json::to_string(&FrameMessage {
                    kind: "frame",
                    frame: &self.frame,
                })
                .unwrap_or_else(|e| format!(r#"{{"type":"frame","error":"{e}"}}"#))
                .into()
            })
            .clone()
    }
}

/// What the source thread pushes to every WebSocket client besides frames.
#[derive(Debug, Clone)]
pub enum BusMessage {
    /// A ready-to-send JSON `ServerMessage` (`Lap`, `Event`).
    Json(Arc<str>),
    /// The source status changed (clients render it with their own output rate).
    StatusChanged,
}

/// The race engineer's knowledge and feedback extractor, shared by the API.
pub struct Engineer {
    pub knowledge: Arc<Knowledge>,
    pub extractor: Arc<dyn SymptomExtractor>,
    /// `dictionary` | `anthropic` (what `/api/health` reports).
    pub llm: &'static str,
}

impl Engineer {
    pub fn new(kind: LlmKind) -> anyhow::Result<Self> {
        let knowledge = Knowledge::embedded()
            .map_err(|e| anyhow::anyhow!("base de connaissances illisible : {e}"))?;
        let symptoms = knowledge.rule_symptoms();
        let (extractor, llm): (Arc<dyn SymptomExtractor>, &'static str) = match kind {
            LlmKind::Anthropic => match AnthropicExtractor::from_env(&symptoms) {
                Ok(Some(e)) => {
                    tracing::info!(
                        "ressenti : modèle de langage {} (repli dictionnaire)",
                        e.model()
                    );
                    (Arc::new(e), "anthropic")
                }
                Ok(None) => {
                    tracing::warn!(
                        "--llm anthropic sans {} : interprétation par le dictionnaire",
                        corde_engineer::feedback::ANTHROPIC_KEY_ENV
                    );
                    (
                        Arc::new(DictionaryExtractor::new(&knowledge.feedback)?),
                        "dictionary",
                    )
                }
                Err(e) => {
                    tracing::warn!("extracteur LLM indisponible ({e}) : dictionnaire");
                    (
                        Arc::new(DictionaryExtractor::new(&knowledge.feedback)?),
                        "dictionary",
                    )
                }
            },
            LlmKind::None => (
                Arc::new(DictionaryExtractor::new(&knowledge.feedback)?),
                "dictionary",
            ),
        };
        Ok(Self {
            knowledge: Arc::new(knowledge),
            extractor,
            llm,
        })
    }
}

pub struct AppState {
    pub frame_tx: watch::Sender<Option<Arc<FrameSlot>>>,
    pub frame_rx: watch::Receiver<Option<Arc<FrameSlot>>>,
    pub bus: broadcast::Sender<BusMessage>,
    status: RwLock<SourceStatus>,
    /// Measured input rate (f32 bits) — updated often, so kept outside `status`
    /// to avoid broadcasting a status change every second.
    input_rate_bits: AtomicU32,
    pub laps: Mutex<Vec<LapRecord>>,
    pub clients: AtomicU32,
    pub layouts: LayoutStore,
    /// Session store (`None` when the database could not be opened: the
    /// server still streams, `/api/sessions*` answer 503).
    pub store: Option<Arc<Store>>,
    /// Recorder status / commands (`None` without a store).
    pub recorder: Option<Arc<RecorderHandle>>,
    /// Results files importer (`None` without a store).
    pub importer: Option<Arc<ResultsImporter>>,
    /// `UserData` folder announced by the running game (LMU source), used to
    /// find the results files and the setups.
    pub game_user_data: Mutex<Option<PathBuf>>,
    /// `--game-user-data`: a folder given on the command line wins over the
    /// one the game announces.
    game_user_data_pinned: std::sync::atomic::AtomicBool,
    /// `UserData` folder found through the Steam libraries (looked up once,
    /// the first time it is needed while the game is closed).
    located_user_data: OnceLock<Option<PathBuf>>,
    /// `<data-dir>` (garage captures live under `diagnostics/`).
    pub data_dir: PathBuf,
    /// `<data-dir>/settings.json` (shared with the desktop shell).
    pub settings: SettingsStore,
    /// The address this process really listens on (`GET /api/network`).
    pub listen: SocketAddr,
    pub default_rate: u8,
    pub started: Instant,
    pub shutdown: watch::Sender<bool>,
    /// Knowledge base + feedback extractor.
    pub engineer: Engineer,
    /// Online race schedule, picks and reminders (page « Courses »).
    pub races: crate::races::Races,
    /// `desktop` (the PC) or `hub` (the home server).
    pub role: Role,
    /// Home server only: paired devices, last PC seen.
    pub hub: Option<Arc<crate::hub::Hub>>,
    /// PC only: the link to the home server and what it still lacks.
    pub link: Option<Arc<crate::hub_link::HubLink>>,
}

/// What differs between the PC and the home server, for [`AppState::new`].
pub struct RoleState {
    pub role: Role,
    pub hub: Option<Arc<crate::hub::Hub>>,
    pub link: Option<Arc<crate::hub_link::HubLink>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        layouts: LayoutStore,
        default_rate: u8,
        store: Option<Arc<Store>>,
        recorder: Option<Arc<RecorderHandle>>,
        importer: Option<Arc<ResultsImporter>>,
        settings: SettingsStore,
        listen: SocketAddr,
        engineer: Engineer,
        data_dir: PathBuf,
        role: RoleState,
    ) -> Arc<Self> {
        let (frame_tx, frame_rx) = watch::channel(None);
        let (bus, _) = broadcast::channel(256);
        let (shutdown, _) = watch::channel(false);
        let races = crate::races::Races::open(&data_dir);
        Arc::new(Self {
            frame_tx,
            frame_rx,
            bus,
            status: RwLock::new(SourceStatus::default()),
            input_rate_bits: AtomicU32::new(0),
            laps: Mutex::new(Vec::new()),
            clients: AtomicU32::new(0),
            layouts,
            store,
            recorder,
            importer,
            game_user_data: Mutex::new(None),
            game_user_data_pinned: std::sync::atomic::AtomicBool::new(false),
            located_user_data: OnceLock::new(),
            data_dir,
            settings,
            listen,
            default_rate,
            started: Instant::now(),
            shutdown,
            engineer,
            races,
            role: role.role,
            hub: role.hub,
            link: role.link,
        })
    }

    /// `true` on the home server.
    pub fn is_hub(&self) -> bool {
        self.role == Role::Hub
    }

    /// Use this `UserData` folder whatever the game announces (`--game-user-data`).
    pub fn pin_game_user_data(&self, dir: PathBuf) {
        tracing::info!("dossier UserData imposé : {}", dir.display());
        if let Ok(mut g) = self.game_user_data.lock() {
            *g = Some(dir);
        }
        self.game_user_data_pinned.store(true, Ordering::Relaxed);
    }

    /// Remember the game's `UserData` folder (first time only, or when it changes).
    pub fn set_game_user_data(&self, dir: Option<PathBuf>) {
        if self.game_user_data_pinned.load(Ordering::Relaxed) {
            return;
        }
        if let Ok(mut g) = self.game_user_data.lock() {
            if *g != dir {
                if let Some(d) = &dir {
                    tracing::info!("dossier UserData du jeu : {}", d.display());
                }
                *g = dir;
            }
        }
    }

    pub fn game_user_data(&self) -> Option<PathBuf> {
        self.game_user_data.lock().ok().and_then(|g| g.clone())
    }

    /// [`Self::game_user_data`], or — game closed, nothing imposed — the
    /// installation found through the Steam libraries. The setups are read
    /// after the race, most often with the game closed.
    pub fn game_user_data_or_located(&self) -> Option<PathBuf> {
        // The home server has no game: never look for one there.
        if self.is_hub() {
            return None;
        }
        self.game_user_data().or_else(|| {
            self.located_user_data
                .get_or_init(|| {
                    let found = corde_lmu::settings::locate_game().map(|p| p.user_data);
                    if let Some(d) = &found {
                        tracing::info!("dossier UserData trouvé via Steam : {}", d.display());
                    }
                    found
                })
                .clone()
        })
    }

    /// Driver name of the live session (the player), when the game is connected.
    pub fn live_player_name(&self) -> Option<String> {
        let slot = self.latest_frame()?;
        let name = slot.frame.session.player_name.trim();
        (!name.is_empty()).then(|| name.to_string())
    }

    /// Snapshot of the status for one client / HTTP caller.
    pub fn status(&self, output_rate_hz: f32) -> SourceStatus {
        let mut s = self.status.read().map(|g| g.clone()).unwrap_or_default();
        s.clients = self.clients.load(Ordering::Relaxed);
        s.input_rate_hz = f32::from_bits(self.input_rate_bits.load(Ordering::Relaxed));
        s.output_rate_hz = output_rate_hz;
        s
    }

    /// Mutate the status; broadcasts `StatusChanged` when something changed.
    pub fn update_status(&self, f: impl FnOnce(&mut SourceStatus)) {
        let changed = {
            let mut g = match self.status.write() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            let before = g.clone();
            f(&mut g);
            *g != before
        };
        if changed {
            self.notify_status();
        }
    }

    pub fn set_input_rate(&self, hz: f32) {
        self.input_rate_bits.store(hz.to_bits(), Ordering::Relaxed);
    }

    pub fn notify_status(&self) {
        let _ = self.bus.send(BusMessage::StatusChanged);
    }

    pub fn publish_lap(&self, lap: LapRecord) {
        if let Ok(mut laps) = self.laps.lock() {
            laps.push(lap.clone());
        }
        self.send_json(&ServerMessage::Lap(lap));
    }

    pub fn publish_event(&self, event: GameEvent) {
        self.send_json(&ServerMessage::Event(event));
    }

    pub fn reset_laps(&self) {
        if let Ok(mut laps) = self.laps.lock() {
            laps.clear();
        }
    }

    pub fn laps(&self) -> Vec<LapRecord> {
        self.laps.lock().map(|l| l.clone()).unwrap_or_default()
    }

    pub fn latest_frame(&self) -> Option<Arc<FrameSlot>> {
        self.frame_rx.borrow().clone()
    }

    pub fn uptime_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Send a message to every connected dashboard.
    pub fn broadcast(&self, msg: &ServerMessage) {
        self.send_json(msg);
    }

    fn send_json(&self, msg: &ServerMessage) {
        match serde_json::to_string(msg) {
            Ok(s) => {
                let _ = self.bus.send(BusMessage::Json(s.into()));
            }
            Err(e) => tracing::warn!("cannot serialise server message: {e}"),
        }
    }

    /// Registers a dashboard; the returned guard unregisters it on drop.
    pub fn client_connected(self: &Arc<Self>) -> ClientGuard {
        self.clients.fetch_add(1, Ordering::Relaxed);
        self.notify_status();
        ClientGuard {
            state: Arc::clone(self),
        }
    }
}

pub struct ClientGuard {
    state: Arc<AppState>,
}

impl Drop for ClientGuard {
    fn drop(&mut self) {
        self.state.clients.fetch_sub(1, Ordering::Relaxed);
        self.state.notify_status();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::{PlayerTelemetry, PROTOCOL_VERSION};

    #[test]
    fn engineer_falls_back_to_the_dictionary_without_an_api_key() {
        let e = Engineer::new(LlmKind::None).unwrap();
        assert_eq!(e.llm, "dictionary");
        assert!(!e.knowledge.rule_symptoms().is_empty());
        if corde_store::migrate::env_var(corde_engineer::feedback::ANTHROPIC_KEY_ENV).is_err() {
            let e = Engineer::new(LlmKind::Anthropic).unwrap();
            assert_eq!(e.llm, "dictionary", "--llm anthropic without a key");
            let cfg = crate::ServerConfig::default();
            assert_eq!(cfg.llm_kind(), LlmKind::None);
        }
        let cfg = crate::ServerConfig {
            llm: Some(LlmKind::Anthropic),
            ..Default::default()
        };
        assert_eq!(cfg.llm_kind(), LlmKind::Anthropic);
        let i = e.extractor.extract("ça survire en sortie", Some("fr"));
        assert_eq!(i.symptoms[0].symptom_id, "OS_EXIT");
    }

    #[test]
    fn frame_slot_json_matches_server_message_encoding() {
        let frame = TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: 7,
            wall_ms: 1234,
            source: "sim".into(),
            player: Some(PlayerTelemetry::default()),
            ..Default::default()
        };
        let expected = serde_json::to_string(&ServerMessage::Frame(frame.clone())).unwrap();
        let slot = FrameSlot::new(frame);
        assert_eq!(&*slot.json(), expected);
        assert!(slot.json().starts_with(r#"{"type":"frame","v":1,"seq":7"#));
    }
}
