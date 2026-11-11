//! The source thread: owns one [`TelemetrySource`], polls it at its native
//! rate, runs the [`LapTracker`], the [`EventDetector`] and the
//! [`Recorder`], and publishes frames / laps / events / status into the
//! shared [`AppState`].
//!
//! Sources are synchronous (a memcpy and a conversion), so they live on a
//! plain OS thread rather than inside the Tokio runtime.

use crate::config::{ServerConfig, SourceKind};
use crate::events::EventDetector;
use crate::state::AppState;
use corde_core::{LapTracker, SourceError, TelemetrySource};
use corde_lmu::{LmuSource, LmuSourceConfig};
use corde_protocol::{GameEvent, GameId, TelemetryFrame};
use corde_sim::{SimConfig, SimSource};
use corde_store::Recorder;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How to build a source.
#[derive(Debug, Clone)]
pub enum SourceSpec {
    Sim(SimConfig),
    /// Le Mans Ultimate shared memory; `shm_path` overrides the Linux mirror path.
    Lmu {
        shm_path: Option<PathBuf>,
    },
}

impl SourceSpec {
    pub fn name(&self) -> &'static str {
        match self {
            SourceSpec::Sim(_) => "sim",
            SourceSpec::Lmu { .. } => "lmu",
        }
    }
}

/// Whether the LMU adapter is compiled into this binary.
pub const LMU_AVAILABLE: bool = true;

/// Instantiate a source from its spec (nothing is opened before `connect`).
pub fn make_source(spec: &SourceSpec) -> anyhow::Result<Box<dyn TelemetrySource>> {
    match spec {
        SourceSpec::Sim(cfg) => Ok(Box::new(SimSource::new(cfg.clone()))),
        SourceSpec::Lmu { shm_path } => Ok(Box::new(LmuSource::with_config(LmuSourceConfig {
            shm_path: shm_path.clone(),
            ..LmuSourceConfig::default()
        }))),
    }
}

/// Primary and optional fallback specs resolved from the CLI.
#[derive(Debug, Clone)]
pub struct SourcePlan {
    pub primary: SourceSpec,
    pub fallback: Option<SourceSpec>,
    /// Switch to the fallback when the primary stays unavailable this long.
    pub fallback_after: Duration,
    /// Longest pause between two `connect` attempts (the back-off starts at 1 s).
    pub retry_max: Duration,
}

impl SourcePlan {
    pub fn from_config(cfg: &ServerConfig) -> Self {
        let sim = SourceSpec::Sim(cfg.sim_config());
        let lmu = SourceSpec::Lmu {
            shm_path: cfg.lmu_shm.clone(),
        };
        let (primary, fallback, retry_max) = match cfg.source {
            SourceKind::Sim => (sim, None, Duration::from_secs(5)),
            SourceKind::Lmu => (
                lmu,
                cfg.fallback_sim.then(|| sim.clone()),
                Duration::from_secs(5),
            ),
            // auto: LMU, retried every 2 s while the game is not running.
            SourceKind::Auto if LMU_AVAILABLE => (
                lmu,
                cfg.fallback_sim.then(|| sim.clone()),
                Duration::from_secs(2),
            ),
            SourceKind::Auto => {
                tracing::warn!("--source auto : adaptateur LMU non intégré → simulateur");
                (sim, None, Duration::from_secs(5))
            }
        };
        Self {
            primary,
            fallback,
            fallback_after: Duration::from_secs(10),
            retry_max,
        }
    }
}

/// Handle on the running source thread.
pub struct RunnerHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl RunnerHandle {
    /// Ask the thread to stop and wait for it (it sleeps at most ~50 ms at a time).
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for RunnerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Frames-per-second meter over a sliding window of 2 s.
struct RateMeter {
    stamps: VecDeque<Instant>,
    window: Duration,
}

impl RateMeter {
    fn new() -> Self {
        Self {
            stamps: VecDeque::with_capacity(512),
            window: Duration::from_secs(2),
        }
    }

    fn tick(&mut self, now: Instant) {
        self.stamps.push_back(now);
        while self
            .stamps
            .front()
            .is_some_and(|t| now.duration_since(*t) > self.window)
        {
            self.stamps.pop_front();
        }
    }

    fn hz(&self, now: Instant) -> f32 {
        match (self.stamps.front(), self.stamps.len()) {
            (Some(first), n) if n >= 2 => {
                let span = now.duration_since(*first).as_secs_f32().max(0.05);
                (n as f32 - 1.0) / span
            }
            _ => 0.0,
        }
    }

    fn reset(&mut self) {
        self.stamps.clear();
    }
}

/// Start the source thread. `source` is the already-built primary source
/// (built in advance so that a configuration error surfaces before binding).
pub fn spawn(
    state: Arc<AppState>,
    source: Box<dyn TelemetrySource>,
    plan: SourcePlan,
    recorder: Option<Recorder>,
) -> RunnerHandle {
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = Arc::clone(&stop);
    let thread = std::thread::Builder::new()
        .name("corde-source".into())
        .spawn(move || run_loop(state, source, plan, recorder, stop2))
        .expect("spawn source thread");
    RunnerHandle {
        stop,
        thread: Some(thread),
    }
}

/// Sleep in small slices so that `stop` is honoured quickly.
fn sleep_interruptible(d: Duration, stop: &AtomicBool) {
    let deadline = Instant::now() + d;
    while !stop.load(Ordering::Relaxed) {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        std::thread::sleep((deadline - now).min(Duration::from_millis(50)));
    }
}

fn run_loop(
    state: Arc<AppState>,
    mut source: Box<dyn TelemetrySource>,
    plan: SourcePlan,
    mut recorder: Option<Recorder>,
    stop: Arc<AtomicBool>,
) {
    let mut fallback = plan.fallback.clone();
    let mut tracker = LapTracker::new();
    let mut events = EventDetector::new();
    let mut status_cache = FrameStatusCache::default();
    let mut meter = RateMeter::new();
    let mut connected = false;
    let mut backoff = Duration::from_secs(1);
    let mut unavailable_since: Option<Instant> = None;
    let mut last_rate_update = Instant::now();
    let mut last_invalid_log = Instant::now() - Duration::from_secs(60);

    let mut info = source.info();
    state.update_status(|s| {
        s.connected = false;
        s.game = info.game;
        s.source = info.id.to_string();
        s.last_error = None;
    });
    tracing::info!(
        "source: {} ({}), {:.0} Hz natif",
        info.id,
        info.label,
        info.native_rate_hz
    );

    while !stop.load(Ordering::Relaxed) {
        if !connected {
            match source.connect() {
                Ok(()) => {
                    connected = true;
                    backoff = Duration::from_secs(1);
                    unavailable_since = None;
                    meter.reset();
                    // The id may be refined by `connect` (e.g. which reader was opened).
                    info = source.info();
                    status_cache = FrameStatusCache::default();
                    tracing::info!("source {} connectée ({})", info.id, info.label);
                    state.update_status(|s| {
                        s.connected = true;
                        s.game = info.game;
                        s.source = info.id.to_string();
                        s.last_error = None;
                    });
                }
                Err(e) => {
                    let since = *unavailable_since.get_or_insert_with(Instant::now);
                    let msg = e.to_string();
                    state.update_status(|s| {
                        s.connected = false;
                        s.last_error = Some(msg.clone());
                    });
                    if let Some(fb) = fallback
                        .as_ref()
                        .filter(|_| since.elapsed() >= plan.fallback_after)
                    {
                        match make_source(fb) {
                            Ok(next) => {
                                tracing::warn!(
                                    "{} indisponible depuis {:?} ({msg}) → bascule sur {}",
                                    info.id,
                                    since.elapsed(),
                                    fb.name()
                                );
                                source = next;
                                info = source.info();
                                fallback = None;
                                tracker = LapTracker::new();
                                events = EventDetector::new();
                                state.reset_laps();
                                state.update_status(|s| {
                                    s.game = info.game;
                                    s.source = info.id.to_string();
                                    s.last_error = None;
                                });
                                continue;
                            }
                            Err(e2) => tracing::error!("source de repli impossible: {e2:#}"),
                        }
                    }
                    tracing::debug!(
                        "source {} indisponible: {msg} — nouvel essai dans {:?}",
                        info.id,
                        backoff
                    );
                    sleep_interruptible(backoff, &stop);
                    backoff = (backoff * 2).min(plan.retry_max);
                    continue;
                }
            }
        }

        match source.poll() {
            Ok(Some(frame)) => {
                let now = Instant::now();
                meter.tick(now);
                handle_frame(
                    &state,
                    &mut tracker,
                    &mut events,
                    recorder.as_mut(),
                    &mut status_cache,
                    frame,
                );
                if now.duration_since(last_rate_update) >= Duration::from_secs(1) {
                    last_rate_update = now;
                    state.set_input_rate(meter.hz(now));
                    // The game announces its UserData folder in the block;
                    // the results importer looks for its files there.
                    if let Some(dir) = source.user_data_dir() {
                        state.set_game_user_data(Some(dir));
                    }
                }
            }
            Ok(None) => {
                let now = Instant::now();
                if now.duration_since(last_rate_update) >= Duration::from_secs(1) {
                    last_rate_update = now;
                    state.set_input_rate(meter.hz(now));
                }
                sleep_interruptible(source.poll_interval(), &stop);
            }
            Err(SourceError::NotAvailable(msg)) => {
                tracing::warn!("source {} perdue: {msg}", info.id);
                connected = false;
                source.disconnect();
                meter.reset();
                state.set_input_rate(0.0);
                if let Some(rec) = recorder.as_mut() {
                    for ev in rec.on_disconnect() {
                        state.publish_event(ev);
                    }
                }
                state.update_status(|s| {
                    s.connected = false;
                    s.in_realtime = false;
                    s.recording = false;
                    s.last_error = Some(msg);
                });
                backoff = Duration::from_secs(1);
            }
            Err(e) => {
                // Invalid snapshot / IO hiccup: keep going, log at most every 5 s.
                if last_invalid_log.elapsed() >= Duration::from_secs(5) {
                    last_invalid_log = Instant::now();
                    tracing::warn!("source {}: {e}", info.id);
                }
                let msg = e.to_string();
                status_cache.had_error = true;
                state.update_status(|s| s.last_error = Some(msg));
                sleep_interruptible(source.poll_interval(), &stop);
            }
        }
    }

    source.disconnect();
    // Ctrl-C / shutdown: close the recording before the thread ends.
    if let Some(rec) = recorder.as_mut() {
        for ev in rec.shutdown() {
            state.publish_event(ev);
        }
    }
    state.update_status(|s| {
        s.connected = false;
        s.in_realtime = false;
        s.recording = false;
    });
    tracing::debug!("source thread stopped");
}

/// Last status-relevant values seen in a frame, to touch the shared status
/// only when they change (not at 100 Hz).
#[derive(Debug, Default)]
struct FrameStatusCache {
    in_realtime: Option<bool>,
    game: Option<GameId>,
    recording: Option<bool>,
    had_error: bool,
}

/// Order matters for the recorder: the laps made official by this frame (they
/// belong to the session still open), then the events (a `SessionStarted` or
/// `SessionEnded` closes the recording), then the frame itself.
fn handle_frame(
    state: &AppState,
    tracker: &mut LapTracker,
    events: &mut EventDetector,
    mut recorder: Option<&mut Recorder>,
    cache: &mut FrameStatusCache,
    frame: TelemetryFrame,
) {
    // Laps first: a lap made official on this frame belongs to the session
    // still open, whatever the events below decide.
    for lap in tracker.feed(&frame) {
        tracing::info!(
            "tour {} : {:.3} s (chronométré par le jeu : {}, valide : {}{})",
            lap.lap_number + 1,
            lap.lap_time_s,
            lap.timed_by_game,
            lap.valid,
            lap.note.map(|n| format!(", {n:?}")).unwrap_or_default()
        );
        if let Some(rec) = recorder.as_deref_mut() {
            rec.on_lap(&lap);
        }
        state.publish_lap(lap);
    }
    for ev in events.feed(&frame) {
        if matches!(ev, GameEvent::SessionStarted { .. }) {
            state.reset_laps();
            tracing::info!(
                "session: {} — {:?} ({:?}), {} voitures",
                frame.session.track_name,
                frame.session.session_type,
                frame.session.game,
                frame.session.num_vehicles
            );
        }
        tracing::debug!("event: {ev:?}");
        if let Some(rec) = recorder.as_deref_mut() {
            for r in rec.on_event(&frame, &ev) {
                state.publish_event(r);
            }
        }
        state.publish_event(ev);
    }
    let recording = match recorder {
        Some(rec) => {
            for r in rec.on_frame(&frame) {
                state.publish_event(r);
            }
            rec.is_recording()
        }
        None => false,
    };
    let in_realtime = frame.session.in_realtime;
    let game = frame.session.game;
    if cache.in_realtime != Some(in_realtime)
        || cache.game != Some(game)
        || cache.recording != Some(recording)
        || cache.had_error
    {
        cache.in_realtime = Some(in_realtime);
        cache.game = Some(game);
        cache.recording = Some(recording);
        cache.had_error = false;
        state.update_status(|s| {
            s.in_realtime = in_realtime;
            s.recording = recording;
            if game != GameId::Unknown {
                s.game = game;
            }
            s.last_error = None;
        });
    }
    state
        .frame_tx
        .send_replace(Some(Arc::new(crate::state::FrameSlot::new(frame))));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_meter_measures_hz() {
        let mut m = RateMeter::new();
        let t0 = Instant::now();
        for i in 0..101 {
            m.tick(t0 + Duration::from_millis(10 * i));
        }
        let hz = m.hz(t0 + Duration::from_millis(1000));
        assert!((hz - 100.0).abs() < 1.0, "hz {hz}");
        // Window drops old samples.
        m.tick(t0 + Duration::from_secs(5));
        assert_eq!(m.stamps.len(), 1);
        assert_eq!(m.hz(t0 + Duration::from_secs(5)), 0.0);
    }

    #[test]
    fn plan_resolution() {
        let mut cfg = ServerConfig {
            source: SourceKind::Sim,
            ..Default::default()
        };
        let plan = SourcePlan::from_config(&cfg);
        assert!(matches!(plan.primary, SourceSpec::Sim(_)) && plan.fallback.is_none());
        cfg.source = SourceKind::Lmu;
        cfg.fallback_sim = true;
        let plan = SourcePlan::from_config(&cfg);
        assert!(
            matches!(plan.primary, SourceSpec::Lmu { .. })
                && matches!(plan.fallback, Some(SourceSpec::Sim(_)))
        );
        assert_eq!(plan.retry_max, Duration::from_secs(5));
        cfg.source = SourceKind::Auto;
        let plan = SourcePlan::from_config(&cfg);
        assert!(matches!(plan.primary, SourceSpec::Lmu { .. }));
        assert_eq!(plan.retry_max, Duration::from_secs(2));
        // Building the LMU source opens nothing: it succeeds without the game.
        let src = make_source(&SourceSpec::Lmu { shm_path: None }).unwrap();
        assert_eq!(src.info().game, corde_protocol::GameId::Lmu);
    }
}
