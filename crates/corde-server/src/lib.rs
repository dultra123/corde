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
//! GET /api/races/sources (setting `online_schedule`) ── GET racecontrol.gg, lmu.tools (cache) ──▶ HTML parsed by the UI
//! reminder task (every 5 s) ── races/picks.json ──▶ broadcast race_reminder
//! hub link (paired PC only) ── closed sessions, results, feedback ──▶ home server (/api/sync/*)
//! ```
//!
//! With `--role hub` the same binary is the **home server** (ADR 0010, [`hub`]): no source,
//! no recorder, no importer, no game API; it receives what a paired PC sends and serves it
//! to paired devices only.
//!
//! The binary is a thin wrapper around [`Server`], which the integration tests
//! also drive in-process on a free port.

pub mod api;
pub mod config;
pub mod crypto;
pub mod events;
pub mod guard;
pub mod hub;
pub mod hub_link;
pub mod layouts;
pub mod lmu_api;
pub mod lmu_garage;
pub mod net;
pub mod races;
pub mod results_import;
pub mod runner;
pub mod settings;
pub mod setups;
pub mod state;
pub mod sync_wire;
pub mod ws;

pub use config::{resolve_bind, BindOrigin, LlmKind, Role, ServerConfig, SourceKind};
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
use std::time::Duration;
use tokio::net::TcpListener;

/// A bound, not yet serving, server.
pub struct Server {
    listener: TcpListener,
    local_addr: SocketAddr,
    bind_origin: BindOrigin,
    state: Arc<AppState>,
    /// `None` on the home server.
    source: Option<Box<dyn TelemetrySource>>,
    plan: SourcePlan,
    recorder: Option<Recorder>,
    ui_dir: Option<PathBuf>,
    data_dir: PathBuf,
    /// Keep a copy of the game's API description (see [`lmu_api`]).
    game_api: bool,
    allow_host: Vec<String>,
    sync_interval: Duration,
}

impl Server {
    /// Open the data directory, bind the TCP listener and build the source.
    /// Fails early on a bad configuration; nothing runs until [`Server::run`].
    ///
    /// The listening address follows [`resolve_bind`]: `cfg.bind`, else every
    /// interface with `--role hub`, `cfg.lan` or the saved `lan_access` setting, else
    /// `127.0.0.1` (this PC only).
    pub async fn bind(cfg: ServerConfig) -> anyhow::Result<Self> {
        let data_dir = cfg.resolve_data_dir();
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| anyhow::anyhow!("cannot create data dir {}: {e}", data_dir.display()))?;
        let hub_role = cfg.role == Role::Hub;

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
        // The store is optional on the PC: a broken database must not take the live
        // telemetry down with it. The home server has nothing else to do: it stops.
        let (store, recorder) = match Store::open(&data_dir) {
            Ok(store) if hub_role => (Some(store), None),
            Ok(store) => match Recorder::new(Arc::clone(&store), cfg.recorder_config()) {
                Ok((recorder, handle)) => (Some(store), Some((recorder, handle))),
                Err(e) => {
                    tracing::error!("enregistreur désactivé : {e}");
                    (Some(store), None)
                }
            },
            Err(e) if hub_role => {
                return Err(anyhow::anyhow!(
                    "base de sessions inaccessible ({}) : {e}",
                    data_dir.join(corde_store::db::DB_FILE).display()
                ))
            }
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
        let importer = match hub_role {
            true => None,
            false => store
                .as_ref()
                .map(|s| ResultsImporter::new(Arc::clone(s), cfg.import_config())),
        };
        let engineer = state::Engineer::new(cfg.llm_kind())?;
        let role = if hub_role {
            let hub = hub::Hub::open(&data_dir)
                .map_err(|e| anyhow::anyhow!("dossier du serveur illisible : {e}"))?;
            hub.auth
                .write_control_file(local_addr.port())
                .map_err(|e| anyhow::anyhow!("cannot write the control file: {e}"))?;
            state::RoleState {
                role: Role::Hub,
                hub: Some(Arc::new(hub)),
                link: None,
            }
        } else {
            state::RoleState {
                role: Role::Desktop,
                hub: None,
                link: Some(Arc::new(hub_link::HubLink::open(&data_dir))),
            }
        };
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
            role,
        );
        if let Some(dir) = cfg.game_user_data.clone().filter(|_| !hub_role) {
            state.pin_game_user_data(dir);
        }

        let mut plan = SourcePlan::from_config(&cfg);
        let source = if hub_role {
            state.update_status(|s| {
                s.source = "hub".to_string();
                s.connected = false;
            });
            None
        } else {
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
            Some(source)
        };

        let ui_dir = cfg.resolve_ui_dir();
        let game_api = !cfg.no_game_api && !hub_role;
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
            allow_host: cfg.allow_host.clone(),
            sync_interval: Duration::from_secs(cfg.sync_interval_s),
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

    /// Name of the source that will be started (`sim`, `lmu`; `hub` on the home server).
    pub fn source_name(&self) -> &'static str {
        if self.source.is_none() {
            "hub"
        } else {
            self.plan.primary.name()
        }
    }

    pub fn banner(&self) -> Vec<String> {
        if self.state.is_hub() {
            return net::hub_banner_lines(self.local_addr, self.ui_dir.as_deref(), &self.data_dir);
        }
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
            allow_host,
            sync_interval,
            ..
        } = self;
        let mut runner: Option<RunnerHandle> =
            source.map(|source| runner::spawn(Arc::clone(&state), source, plan, recorder));
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
        let reminder_task = tokio::spawn(races::run_periodic(
            Arc::clone(&state),
            state.shutdown.subscribe(),
        ));
        let hub_task = tokio::spawn(hub::run_periodic(
            Arc::clone(&state),
            state.shutdown.subscribe(),
        ));
        let link_task = tokio::spawn(hub_link::run_periodic(
            Arc::clone(&state),
            sync_interval,
            state.shutdown.subscribe(),
        ));
        let app = api::router_with(Arc::clone(&state), ui_dir, &allow_host);
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
        if let Some(r) = runner.as_mut() {
            r.stop();
        }
        if let Some(t) = import_task {
            t.abort();
        }
        if let Some(t) = game_api_task {
            t.abort();
        }
        if let Some(t) = garage_task {
            t.abort();
        }
        reminder_task.abort();
        // Lets the hub write the devices' last use once more.
        let _ = tokio::time::timeout(Duration::from_secs(2), hub_task).await;
        link_task.abort();
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
