//! Command line / programmatic configuration of the server.

use crate::settings::Settings;
use clap::{Parser, ValueEnum};
use corde_sim::{PlayerClass, SimConfig};
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

/// Default HTTP + WebSocket port (`packages/protocol/src/index.ts: DEFAULT_PORT`).
pub const DEFAULT_PORT: u16 = 8471;
/// Default broadcast rate to a dashboard, Hz.
pub const DEFAULT_RATE_HZ: u8 = 20;
/// Highest rate a client may ask for (`ClientMessage::SetRate`).
pub const MAX_RATE_HZ: u8 = 60;
/// Default sample rate of the recorder, Hz.
pub const DEFAULT_RECORD_RATE_HZ: f32 = 50.0;
/// Seconds between two rounds of sending to the home server.
pub const DEFAULT_SYNC_INTERVAL_S: u64 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
#[clap(rename_all = "lower")]
pub enum SourceKind {
    /// Le Mans Ultimate, retrying every 2 s while the game is not running
    /// (add `--fallback-sim` to get the simulator after 10 s).
    #[default]
    Auto,
    /// Built-in simulator (no game needed).
    Sim,
    /// Le Mans Ultimate shared memory.
    Lmu,
}

/// What this process is (`--role`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
#[clap(rename_all = "lower")]
pub enum Role {
    /// The PC that runs the game (default): reads the game, records, serves the live view.
    #[default]
    Desktop,
    /// The always-on home server (ADR 0010): no game, no recorder; keeps what a paired PC
    /// sends and serves it to paired devices only. Listens on every interface.
    Hub,
}

/// Which extractor reads the driver's feedback (`--llm`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
#[clap(rename_all = "lower")]
pub enum LlmKind {
    /// The offline dictionary (`knowledge/feedback-dictionary.yaml` + regexes).
    #[default]
    None,
    /// Anthropic Messages API (`CORDE_ANTHROPIC_API_KEY`), dictionary as fallback.
    Anthropic,
}

#[derive(Debug, Clone, Parser)]
#[command(
    name = "corde-server",
    version,
    about = "Serveur local Corde : télémétrie → WebSocket/HTTP pour les tableaux de bord"
)]
pub struct ServerConfig {
    /// `desktop` (default: the PC that runs the game) or `hub` (the home server: no game,
    /// pairing required, see docs/SERVEUR.md).
    #[arg(long, value_enum, default_value_t = Role::Desktop)]
    pub role: Role,
    /// Another host name this server answers to (`Host` header), besides IP addresses and
    /// this machine's own name: e.g. a name your VPN or your router gives the server. Repeat
    /// the option for several names.
    #[arg(long = "allow-host", value_name = "NAME")]
    pub allow_host: Vec<String>,
    /// PC paired with a home server: seconds between two rounds of sending (0 = only after
    /// a recording stops and when asked). Start-up and the end of every recording send too.
    #[arg(long, default_value_t = DEFAULT_SYNC_INTERVAL_S, hide = true)]
    pub sync_interval_s: u64,
    /// TCP port to listen on (0 = pick a free port).
    #[arg(long, default_value_t = DEFAULT_PORT)]
    pub port: u16,
    /// Address to bind, overriding everything else (0.0.0.0 = every interface).
    /// Default: 127.0.0.1 — this PC only, no Windows firewall prompt — unless
    /// `--lan` is given or the setting "access from my phone" is on
    /// (`lan_access` in <data-dir>/settings.json).
    #[arg(long)]
    pub bind: Option<IpAddr>,
    /// Listen on every interface (0.0.0.0) so that a phone or another PC of the
    /// local network can connect, whatever the saved setting says.
    #[arg(long)]
    pub lan: bool,
    /// Telemetry source.
    #[arg(long, value_enum, default_value_t = SourceKind::Auto)]
    pub source: SourceKind,
    /// With `--source auto|lmu`: switch to the simulator when the game is still
    /// not available after ~10 s.
    #[arg(long)]
    pub fallback_sim: bool,
    /// Directory of the built dashboard (index.html…). Default: `ui/` next to the
    /// executable, else `packages/ui/dist` under the current directory, else API only.
    #[arg(long)]
    pub ui_dir: Option<PathBuf>,
    /// Where layouts and sessions are stored. Default: %LOCALAPPDATA%\Corde (Windows)
    /// or ~/.local/share/corde (Linux/macOS).
    #[arg(long)]
    pub data_dir: Option<PathBuf>,
    /// Default broadcast rate per WebSocket client, Hz (1..=60; clients can change it).
    #[arg(long, default_value_t = DEFAULT_RATE_HZ)]
    pub rate: u8,
    /// Log filter (tracing syntax): info, debug, corde_server=debug,tower_http=info…
    #[arg(long, default_value = "info")]
    pub log: String,
    /// Simulator seed.
    #[arg(long)]
    pub sim_seed: Option<u64>,
    /// Simulator player class: gt3 | hypercar.
    #[arg(long)]
    pub sim_class: Option<PlayerClass>,
    /// Simulator rain intensity 0..1.
    #[arg(long)]
    pub sim_rain: Option<f32>,
    /// Simulator tick rate, Hz.
    #[arg(long)]
    pub sim_rate: Option<f32>,
    /// Simulator: number of AI cars.
    #[arg(long)]
    pub sim_ai: Option<usize>,
    /// Simulator: nominal lap time of the player, seconds (≥ 30; bounded below by
    /// the speed envelope — shorten --sim-track-length for very short laps).
    #[arg(long)]
    pub sim_lap_time: Option<f32>,
    /// Simulator: circuit length, metres (500..=30000; default 5698.6).
    #[arg(long)]
    pub sim_track_length: Option<f32>,
    /// Simulator: speed factor vs real time (10 = laps go by ten times faster).
    #[arg(long)]
    pub sim_speed: Option<f64>,
    /// LMU on Linux: path of the mirrored block (default /dev/shm/LMU_Data). Ignored on Windows.
    #[arg(long)]
    pub lmu_shm: Option<PathBuf>,
    /// Record sessions to <data-dir>/corde.db + Parquet (default). See --no-record.
    #[arg(long, conflicts_with = "no_record")]
    pub record: bool,
    /// Do not record automatically (POST /api/recording/start still works).
    #[arg(long)]
    pub no_record: bool,
    /// Sample rate written to disk, Hz (1..=200, capped by the source rate).
    #[arg(long, default_value_t = DEFAULT_RECORD_RATE_HZ)]
    pub record_rate: f32,
    /// Folder of the game's results files (<LMU>\UserData\Log\Results). Default: announced
    /// by the running game, else found through the Steam libraries.
    #[arg(long)]
    pub lmu_results_dir: Option<PathBuf>,
    /// Your in-game driver name, to tell you apart in multiplayer results files (default:
    /// taken from the game while it runs, else inferred as the name present in every file).
    #[arg(long)]
    pub player_name: Option<String>,
    /// Do not import results files (GET /api/profile stays empty).
    #[arg(long)]
    pub no_import: bool,
    /// Do not read the game's local API at all: neither the copy of its description
    /// (<data-dir>/diagnostics/lmu-api, one read-only request when the game is detected) nor the
    /// garage capture (<data-dir>/diagnostics/lmu-api/garage, a few read-only requests per car
    /// and session while at the garage).
    #[arg(long)]
    pub no_game_api: bool,
    /// The game's `UserData` folder (setups in `player/Settings/<Track>/*.svm`), instead of the
    /// one the running game announces. Useful for tests, or when the game is not running.
    #[arg(long)]
    pub game_user_data: Option<PathBuf>,
    /// Feedback interpreter: `none` = offline dictionary (default), `anthropic` = language
    /// model through the Anthropic API (needs CORDE_ANTHROPIC_API_KEY; the dictionary stays
    /// the fallback). Setting the variable alone also enables it.
    #[arg(long, value_enum)]
    pub llm: Option<LlmKind>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            role: Role::Desktop,
            allow_host: Vec::new(),
            sync_interval_s: DEFAULT_SYNC_INTERVAL_S,
            port: DEFAULT_PORT,
            bind: None,
            lan: false,
            source: SourceKind::Auto,
            fallback_sim: false,
            ui_dir: None,
            data_dir: None,
            rate: DEFAULT_RATE_HZ,
            log: "info".into(),
            sim_seed: None,
            sim_class: None,
            sim_rain: None,
            sim_rate: None,
            sim_ai: None,
            sim_lap_time: None,
            sim_track_length: None,
            sim_speed: None,
            lmu_shm: None,
            record: false,
            no_record: false,
            record_rate: DEFAULT_RECORD_RATE_HZ,
            lmu_results_dir: None,
            player_name: None,
            no_import: false,
            no_game_api: false,
            game_user_data: None,
            llm: None,
        }
    }
}

impl ServerConfig {
    /// The extractor to use: `--llm` when given, else `anthropic` when the
    /// API key variable is set, else the dictionary.
    pub fn llm_kind(&self) -> LlmKind {
        match self.llm {
            Some(k) => k,
            None => {
                match corde_store::migrate::env_var(corde_engineer::feedback::ANTHROPIC_KEY_ENV) {
                    Ok(k) if !k.trim().is_empty() => LlmKind::Anthropic,
                    _ => LlmKind::None,
                }
            }
        }
    }

    /// Default rate clamped into the supported range.
    pub fn default_rate(&self) -> u8 {
        self.rate.clamp(1, MAX_RATE_HZ)
    }

    /// Recording starts automatically unless `--no-record`.
    pub fn record_enabled(&self) -> bool {
        !self.no_record
    }

    /// Recorder sample rate, sanitised.
    pub fn record_rate_hz(&self) -> f32 {
        if self.record_rate.is_finite() {
            self.record_rate.clamp(1.0, 200.0)
        } else {
            DEFAULT_RECORD_RATE_HZ
        }
    }

    /// Recorder configuration derived from the CLI.
    pub fn recorder_config(&self) -> corde_store::RecorderConfig {
        corde_store::RecorderConfig {
            enabled: self.record_enabled(),
            record_rate_hz: self.record_rate_hz(),
            ..Default::default()
        }
    }

    /// Results importer configuration derived from the CLI.
    pub fn import_config(&self) -> crate::results_import::ImportConfig {
        crate::results_import::ImportConfig {
            enabled: !self.no_import,
            results_dir: self.lmu_results_dir.clone(),
            player_name: self
                .player_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string),
            ..Default::default()
        }
    }

    /// Simulator configuration derived from the `--sim-*` flags.
    pub fn sim_config(&self) -> SimConfig {
        let mut c = SimConfig::default();
        if let Some(seed) = self.sim_seed {
            c.seed = seed;
        }
        if let Some(class) = self.sim_class {
            c.player_class = class;
        }
        if let Some(rain) = self.sim_rain {
            c.rain = rain;
        }
        if let Some(rate) = self.sim_rate {
            c.rate_hz = rate;
        }
        if let Some(ai) = self.sim_ai {
            c.num_ai = ai;
        }
        if let Some(t) = self.sim_lap_time {
            c.lap_time_s = t;
        }
        if let Some(l) = self.sim_track_length {
            c.track_length_m = l;
        }
        if let Some(s) = self.sim_speed {
            c.time_scale = s;
        }
        c.sanitized()
    }

    /// Resolve the data directory (`--data-dir`, else the per-user local data
    /// folder: `%LOCALAPPDATA%\Corde` on Windows, `~/.local/share/corde` elsewhere).
    ///
    /// Without `--data-dir`, the folder of the application's former name is
    /// taken over first ([`corde_store::migrate`]): renamed when possible, else
    /// used as is for this run. Logged; idempotent.
    pub fn resolve_data_dir(&self) -> PathBuf {
        if let Some(d) = &self.data_dir {
            return d.clone();
        }
        let migration = corde_store::prepare_default_data_dir();
        migration.log();
        migration.dir
    }

    /// Resolve the UI directory: `--ui-dir`, else `ui/` next to the executable,
    /// else `packages/ui/dist` relative to the current directory. `None` = API only.
    pub fn resolve_ui_dir(&self) -> Option<PathBuf> {
        if let Some(d) = &self.ui_dir {
            return Some(d.clone());
        }
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                candidates.push(dir.join("ui"));
            }
        }
        candidates.push(PathBuf::from("packages").join("ui").join("dist"));
        candidates.into_iter().find(|p| is_ui_dir(p))
    }
}

/// Why the server listens where it does (worded in the start-up banner).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindOrigin {
    /// `--bind <ip>`.
    Explicit,
    /// `--lan`.
    LanFlag,
    /// `lan_access: true` in `settings.json`.
    Setting,
    /// Nothing asked for the network: this PC only.
    Default,
    /// `--role hub`: the home server listens on every interface (pairing required).
    Hub,
}

/// The address the server listens on, in order of priority:
///
/// 1. `--bind <ip>` (`cfg.bind`), as given;
/// 2. `--lan` (`cfg.lan`): `0.0.0.0`;
/// 3. the saved setting: `0.0.0.0` when `lan_access` is on, else `127.0.0.1`.
///
/// So without any option nor setting the server is reachable from this PC only
/// and Windows never shows its firewall prompt. [`crate::Server::bind`] calls
/// this with the settings of the data directory; the desktop shell can call it
/// too (`resolve_bind(&cfg, &Settings::load(&cfg.resolve_data_dir()))`).
pub fn resolve_bind(cfg: &ServerConfig, settings: &Settings) -> IpAddr {
    resolve_bind_origin(cfg, settings).0
}

/// [`resolve_bind`] plus the rule that decided.
pub fn resolve_bind_origin(cfg: &ServerConfig, settings: &Settings) -> (IpAddr, BindOrigin) {
    if let Some(ip) = cfg.bind {
        return (ip, BindOrigin::Explicit);
    }
    if cfg.role == Role::Hub {
        return (IpAddr::V4(Ipv4Addr::UNSPECIFIED), BindOrigin::Hub);
    }
    if cfg.lan {
        return (IpAddr::V4(Ipv4Addr::UNSPECIFIED), BindOrigin::LanFlag);
    }
    if settings.lan_access {
        return (IpAddr::V4(Ipv4Addr::UNSPECIFIED), BindOrigin::Setting);
    }
    (IpAddr::V4(Ipv4Addr::LOCALHOST), BindOrigin::Default)
}

/// Per-user local data folder for Corde (shared with the `corde-store` CLI).
pub fn default_data_dir() -> PathBuf {
    corde_store::default_data_dir()
}

fn is_ui_dir(p: &Path) -> bool {
    p.join("index.html").is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_defaults_match_protocol_constants() {
        let c = ServerConfig::parse_from(["corde-server"]);
        assert_eq!(c.port, 8471);
        assert_eq!(c.rate, 20);
        assert_eq!(c.source, SourceKind::Auto);
        assert_eq!(c.bind, None);
        assert!(!c.lan);
        let c = ServerConfig::parse_from([
            "corde-server",
            "--source",
            "sim",
            "--port",
            "0",
            "--rate",
            "90",
            "--sim-class",
            "hypercar",
        ]);
        assert_eq!(c.source, SourceKind::Sim);
        assert_eq!(c.default_rate(), 60);
        assert_eq!(c.sim_config().player_class, PlayerClass::Hypercar);
        assert!(c.record_enabled());
        assert_eq!(c.record_rate_hz(), 50.0);
        let c = ServerConfig::parse_from(["corde-server", "--no-record", "--record-rate", "1000"]);
        assert!(!c.record_enabled());
        assert_eq!(c.record_rate_hz(), 200.0);
        assert!(ServerConfig::try_parse_from(["corde-server", "--record", "--no-record"]).is_err());
        let ic = c.import_config();
        assert!(ic.enabled && ic.results_dir.is_none() && ic.player_name.is_none());
        let c = ServerConfig::parse_from([
            "corde-server",
            "--lmu-results-dir",
            "reference/samples",
            "--player-name",
            " alex martin ",
            "--no-import",
        ]);
        let ic = c.import_config();
        assert!(!ic.enabled);
        assert_eq!(ic.results_dir, Some(PathBuf::from("reference/samples")));
        assert_eq!(ic.player_name.as_deref(), Some("alex martin"));
    }

    #[test]
    fn bind_address_priority_is_explicit_then_lan_flag_then_setting() {
        let off = Settings::default();
        let on = Settings {
            lan_access: true,
            ..Default::default()
        };
        let local = IpAddr::from([127, 0, 0, 1]);
        let any = IpAddr::from([0, 0, 0, 0]);

        // Nothing asked: this PC only (no firewall prompt on first launch).
        let c = ServerConfig::parse_from(["corde-server"]);
        assert_eq!(resolve_bind(&c, &off), local);
        assert_eq!(resolve_bind_origin(&c, &off).1, BindOrigin::Default);
        assert_eq!(resolve_bind(&ServerConfig::default(), &off), local);

        // The saved setting opens the LAN.
        assert_eq!(resolve_bind(&c, &on), any);
        assert_eq!(resolve_bind_origin(&c, &on).1, BindOrigin::Setting);

        // --lan forces it whatever the setting says.
        let c = ServerConfig::parse_from(["corde-server", "--lan"]);
        assert!(c.lan && c.bind.is_none());
        assert_eq!(resolve_bind(&c, &off), any);
        assert_eq!(resolve_bind_origin(&c, &off).1, BindOrigin::LanFlag);

        // --bind wins over both.
        let c = ServerConfig::parse_from(["corde-server", "--bind", "127.0.0.1", "--lan"]);
        assert_eq!(resolve_bind(&c, &on), local);
        assert_eq!(resolve_bind_origin(&c, &on).1, BindOrigin::Explicit);
        let c = ServerConfig::parse_from(["corde-server", "--bind", "192.168.1.20"]);
        assert_eq!(resolve_bind(&c, &off), IpAddr::from([192, 168, 1, 20]));

        // The home server listens on every interface, whatever the setting says.
        let c = ServerConfig::parse_from(["corde-server", "--role", "hub"]);
        assert_eq!(c.role, Role::Hub);
        assert_eq!(resolve_bind_origin(&c, &off), (any, BindOrigin::Hub));
        let c = ServerConfig::parse_from(["corde-server", "--role", "hub", "--bind", "127.0.0.1"]);
        assert_eq!(resolve_bind(&c, &off), local);
        assert_eq!(
            ServerConfig::parse_from(["corde-server"]).role,
            Role::Desktop
        );
        assert!(ServerConfig::try_parse_from(["corde-server", "--bind", "pc-jeu"]).is_err());
    }

    #[test]
    fn data_dir_ends_with_corde() {
        let d = default_data_dir();
        let name = d
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_ascii_lowercase();
        assert_eq!(name, "corde");
    }
}
