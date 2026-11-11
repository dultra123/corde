//! # corde-server
//!
//! The local Corde server: polls one [`corde_core::TelemetrySource`] on a
//! dedicated thread, tracks laps and events, and serves the dashboards over
//! HTTP (static UI + `/api/*`) and WebSocket (`/ws`).
//!
//! ```text
//! source thread ──frames──▶ watch<FrameSlot> ──▶ one timer per WS client (its own rate)
//!               ──laps/events/status──▶ broadcast ──▶ every WS client
//!               ──frames/laps/events──▶ corde_store::Recorder ──▶ corde.db + Parquet
//! results importer (every 60 s) ── UserData/Log/Results/*.xml ──▶ corde.db (results) ──▶ /api/profile
//! GET /api/sessions/{id}/analysis ── corde_engineer::analyse_session ──▶ corde.db (analyses) ──▶ AnalysisReport
//!                                      └── resolved against the driver's .svm (setups) ──▶ plans with real indices
//! garage capture (game at the garage, once per car and session) ── 8 GET ──▶ diagnostics/lmu-api/garage/
//! POST /api/sessions/{id}/setup-files ── corde_engineer::setup::build_setup ──▶ UserData/player/Settings/<Track>/Corde_*.svm
//! ```
//!
//! The binary is a thin wrapper around [`Server`], which the integration tests
//! also drive in-process on a free port.

pub mod api;
pub mod config;
pub mod events;
pub mod layouts;
pub mod lmu_api;
pub mod lmu_garage;
pub mod net;
pub mod results_import;
pub mod runner;
pub mod settings;
pub mod setups;
pub mod state;
pub mod ws;

pub use config::{resolve_bind, BindOrigin, LlmKind, ServerConfig, SourceKind};
pub use settings::Settings;
pub use state::{AppState, SERVER_VERSION};

use corde_core::TelemetrySource;
use corde_store::{Recorder, Store};
use results_import::ResultsImporter;
use runner::{RunnerHandle, SourcePlan, SourceSpec};
use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

/// A bound, not yet serving, server.
pub struct Server {
    listener: TcpListener,
    local_addr: SocketAddr,
    bind_origin: BindOrigin,
    state: Arc<AppState>,
    source: Box<dyn TelemetrySource>,
    plan: SourcePlan,
    recorder: Option<Recorder>,
    ui_dir: Option<PathBuf>,
    data_dir: PathBuf,
    /// Keep a copy of the game's API description (see [`lmu_api`]).
    game_api: bool,
}

impl Server {
    /// Open the data directory, bind the TCP listener and build the source.
    /// Fails early on a bad configuration; nothing runs until [`Server::run`].
    ///
    /// The listening address follows [`resolve_bind`]: `cfg.bind`, else every
    /// interface with `cfg.lan` or the saved `lan_access` setting, else
    /// `127.0.0.1` (this PC only).
    pub async fn bind(cfg: ServerConfig) -> anyhow::Result<Self> {
        let data_dir = cfg.resolve_data_dir();
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| anyhow::anyhow!("cannot create data dir {}: {e}", data_dir.display()))?;

        // The port first: a second Corde on the same port must stop here,
        // before it touches the database of the one already running.
        let settings = settings::SettingsStore::open(&data_dir);
        let (bind_ip, bind_origin) = config::resolve_bind_origin(&cfg, &settings.get());
        let addr = SocketAddr::new(bind_ip, cfg.port);
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                anyhow::anyhow!(
                    "cannot bind {addr}: {e} — Corde tourne peut-être déjà (ou choisis un autre port avec --port)"
                )
            } else {
                anyhow::anyhow!("cannot bind {addr}: {e}")
            }
        })?;
        let local_addr = listener.local_addr()?;

        let layouts = layouts::LayoutStore::open(&data_dir)?;
        // The store is optional: a broken database must not take the live
        // telemetry down with it.
        let (store, recorder) = match Store::open(&data_dir) {
            Ok(store) => match Recorder::new(Arc::clone(&store), cfg.recorder_config()) {
                Ok((recorder, handle)) => (Some(store), Some((recorder, handle))),
                Err(e) => {
                    tracing::error!("enregistreur désactivé : {e}");
                    (Some(store), None)
                }
            },
            Err(e) => {
                tracing::error!(
                    "base de sessions inaccessible ({}) : enregistrement désactivé — {e}",
                    data_dir.join(corde_store::db::DB_FILE).display()
                );
                (None, None)
            }
        };
        let (recorder, recorder_handle) = match recorder {
            Some((r, h)) => (Some(r), Some(h)),
            None => (None, None),
        };
        let importer = store
            .as_ref()
            .map(|s| ResultsImporter::new(Arc::clone(s), cfg.import_config()));
        let engineer = state::Engineer::new(cfg.llm_kind())?;
        let state = AppState::new(
            layouts,
            cfg.default_rate(),
            store,
            recorder_handle,
            importer,
            settings,
            local_addr,
            engineer,
            data_dir.clone(),
        );
        if let Some(dir) = cfg.game_user_data.clone() {
            state.pin_game_user_data(dir);
        }

        let mut plan = SourcePlan::from_config(&cfg);
        let source = match runner::make_source(&plan.primary) {
            Ok(s) => s,
            Err(e) => match plan.fallback.take() {
                Some(fb) => {
                    tracing::warn!("{e:#} → {}", fb.name());
                    plan.primary = fb;
                    runner::make_source(&plan.primary)?
                }
                None => return Err(e),
            },
        };

        // Status is meaningful before the source thread starts (Hello on an
        // early connection must already name the source).
        let info = source.info();
        state.update_status(|s| {
            s.game = info.game;
            s.source = info.id.to_string();
            s.connected = false;
        });

        let ui_dir = cfg.resolve_ui_dir();
        let game_api = !cfg.no_game_api;
        Ok(Self {
            listener,
            local_addr,
            bind_origin,
            state,
            source,
            plan,
            recorder,
            ui_dir,
            data_dir,
            game_api,
        })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.local_addr
    }

    /// `true` when other devices of the local network can connect (the server
    /// does not listen on the loopback only).
    pub fn listens_on_lan(&self) -> bool {
        net::listens_on_lan(self.local_addr.ip())
    }

    pub fn state(&self) -> &Arc<AppState> {
        &self.state
    }

    pub fn ui_dir(&self) -> Option<&PathBuf> {
        self.ui_dir.as_ref()
    }

    pub fn data_dir(&self) -> &PathBuf {
        &self.data_dir
    }

    /// Name of the source that will be started (`sim`, `lmu`).
    pub fn source_name(&self) -> &'static str {
        self.plan.primary.name()
    }

    pub fn banner(&self) -> Vec<String> {
        net::banner_lines(
            self.local_addr,
            self.bind_origin,
            self.source_name(),
            self.ui_dir.as_deref(),
            &self.data_dir,
        )
    }

    /// Start the source thread and serve until `shutdown` resolves, then stop
    /// the source thread.
    pub async fn run(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> anyhow::Result<()> {
        let Server {
            listener,
            state,
            source,
            plan,
            recorder,
            ui_dir,
            data_dir,
            game_api,
            ..
        } = self;
        let mut runner: RunnerHandle = runner::spawn(Arc::clone(&state), source, plan, recorder);
        let import_task = state.importer.clone().map(|imp| {
            tokio::spawn(results_import::run_periodic(
                imp,
                Arc::clone(&state),
                state.shutdown.subscribe(),
            ))
        });
        let game_api_task = game_api.then(|| {
            tokio::spawn(lmu_api::run_periodic(
                Arc::clone(&state),
                data_dir.clone(),
                state.shutdown.subscribe(),
            ))
        });
        let garage_task = game_api.then(|| {
            tokio::spawn(lmu_garage::run_periodic(
                Arc::clone(&state),
                data_dir,
                state.shutdown.subscribe(),
            ))
        });
        let app = api::router(Arc::clone(&state), ui_dir);
        let graceful = {
            let state = Arc::clone(&state);
            async move {
                shutdown.await;
                tracing::info!("arrêt demandé");
                let _ = state.shutdown.send(true);
            }
        };
        let result = axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(graceful)
        .await;
        // Joins the source thread, which closes the recording (flush guaranteed).
        runner.stop();
        if let Some(t) = import_task {
            t.abort();
        }
        if let Some(t) = game_api_task {
            t.abort();
        }
        if let Some(t) = garage_task {
            t.abort();
        }
        result.map_err(Into::into)
    }
}

/// Resolves on Ctrl-C (SIGINT) — and SIGTERM on Unix.
pub async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(e) = tokio::signal::ctrl_c().await {
            tracing::warn!("cannot listen for Ctrl-C: {e}");
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut s) => {
                s.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}

/// Convenience for tests and embedding: the simulator as the only source.
pub fn sim_spec(cfg: &ServerConfig) -> SourceSpec {
    SourceSpec::Sim(cfg.sim_config())
}
