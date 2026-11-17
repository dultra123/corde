//! One row of the game and its session text → `corde_protocol::TelemetryFrame`.
//!
//! What the game does not publish is left empty rather than guessed, and what it
//! publishes late is waited for:
//!
//! * **Lap count.** `Lap` (laps started) moves at the line: it is the boundary
//!   (`lap_number`). `LapCompleted` moves with it, but the time of the lap that just
//!   ended (`LapLastLapTime`) lands up to a second later. `laps_completed` follows once
//!   that time changed, or after [`LAP_TIME_WAIT_S`] — the lap then has no official time
//!   and `last_lap_s` stays empty until the game publishes another one (the time still
//!   shown is the one of an older lap). A lap the game refuses is published as −1: counted,
//!   without a time.
//! * **Leaving the car.** The counters read 0 while the car is out of the world (garage,
//!   tow): they are held, not taken for a restart.
//! * **Sector times, live tyre pressures and brake temperatures, world coordinates of the
//!   other cars**: not published. The position of the player comes from its GPS
//!   coordinates, in metres from where the session was first seen.
//! * **Invalid lap.** The game has no such flag; a lap during which the incident count of
//!   the player rose is marked invalidated (Corde's own rule).
//!
//! Everything marked `TODO(verify-in-game)` is an assumption taken from the SDK that
//! still has to be confirmed against the running game.

use crate::layout::{
    engine_warnings, flags, session_state, track_loc, MAX_CARS, UNLIMITED_LAPS, UNLIMITED_TIME_S,
};
use crate::session::{compound_type, SessionDesc};
use crate::vars::Row;
use corde_protocol::{
    Control, Electronics, Energy, Engine, GameId, GamePhase, Gaps, Inputs, LapTiming, Motion,
    PitState, PlayerTelemetry, SessionInfo, SessionType, Standing, TelemetryFrame, TyreSet,
    VehicleClass, Weather, Wheel, YellowFlagState, PROTOCOL_VERSION,
};

/// How long (session clock) the time of a lap may take to appear after the counter moved
/// (observed: 0.03 to 1.00 s).
pub const LAP_TIME_WAIT_S: f64 = 2.0;
/// Two laps in a row with the very same time: the time that did not change is taken for
/// the new lap when it is this close to what the lap lasted on the session clock (which
/// is only read 60 times a second: a sixtieth at each end).
const SAME_TIME_TOLERANCE_S: f64 = 0.05;
/// The two lap counters of the game moving within this long are one line crossing.
const SAME_CROSSING_S: f64 = 1.0;
const G: f32 = 9.806_65;
/// Metres per degree of latitude.
const M_PER_DEG: f64 = 111_320.0;
/// The four wheels as the game names them, in Corde's order (FL, FR, RL, RR).
const WHEELS: [&str; 4] = ["LF", "RF", "LR", "RR"];

fn wall_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A time of the game in seconds, `None` when it has none (0, −1).
fn time_s(v: f32) -> Option<f32> {
    (v > 0.0).then_some(v)
}

fn small(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn phase(state: i32, session_flags: u32) -> GamePhase {
    if session_flags & flags::RED != 0 {
        GamePhase::SessionStopped
    } else if session_flags & flags::CHECKERED != 0
        || matches!(state, session_state::CHECKERED | session_state::COOL_DOWN)
    {
        GamePhase::SessionOver
    } else if session_flags & (flags::CAUTION | flags::CAUTION_WAVING) != 0 {
        GamePhase::FullCourseYellow
    } else {
        match state {
            session_state::GET_IN_CAR => GamePhase::BeforeSession,
            session_state::WARMUP => GamePhase::GridWalk,
            session_state::PARADE_LAPS => GamePhase::Formation,
            session_state::RACING => GamePhase::Green,
            _ => GamePhase::Unknown,
        }
    }
}

/// A lap counted by the game whose time has not been published yet.
#[derive(Debug, Clone, Copy)]
struct Waiting {
    /// `LapLastLapTime` before the counter moved.
    before: f32,
    /// When the counter moved (session clock).
    since: f64,
    /// What the lap lasted between the two line crossings, when both were seen.
    measured: Option<f64>,
}

/// Keeps what has to be remembered from one row to the next (see the module
/// documentation). One per connection to the game.
#[derive(Debug, Default)]
pub struct Converter {
    seq: u64,
    /// Circuit and session number: the counters restart when one changes.
    key: Option<(String, i32)>,
    last_clock: f64,
    /// Laps started (the boundary), official count.
    lap: i32,
    done: i32,
    confirmed: i32,
    waiting: Option<Waiting>,
    last_time_seen: f32,
    /// The published last lap time belongs to an older lap.
    stale: Option<f32>,
    /// Session clock of the last line crossing.
    boundary_s: Option<f64>,
    /// What the lap that ended there lasted, when its start was seen too.
    last_len_s: Option<f64>,
    incidents_at_lap_start: i32,
    /// Where the session was first seen (latitude, longitude, degrees).
    origin: Option<(f64, f64)>,
}

impl Converter {
    pub fn new() -> Self {
        Self::default()
    }

    /// One frame from one row. `desc` is what the session text says.
    pub fn frame(&mut self, desc: &SessionDesc, row: Row, source_id: &str) -> TelemetryFrame {
        let now = row.f64("SessionTime");
        let session_num = row.i32("SessionNum").unwrap_or(0);
        let player_idx = row
            .i32("PlayerCarIdx")
            .or(Some(desc.player_idx))
            .and_then(|i| usize::try_from(i).ok())
            .filter(|i| *i < MAX_CARS);
        let surface = row.i32("PlayerTrackSurface");
        let in_world = surface != Some(track_loc::NOT_IN_WORLD);
        let lap_raw = row.i32("LapCompleted").unwrap_or(0).max(0);
        let started_raw = row.i32("Lap").unwrap_or(lap_raw).max(0);
        let last = row.num("LapLastLapTime").unwrap_or(-1.0) as f32;
        let incidents = row.i32("PlayerCarMyIncidentCount").unwrap_or(0);

        let key = (desc.track_id.clone(), session_num);
        if self.key.as_ref() != Some(&key) || now < self.last_clock - 1.0 {
            // Another session, or the clock went back (restart).
            *self = Self {
                seq: self.seq,
                key: Some(key),
                lap: started_raw,
                done: lap_raw,
                confirmed: lap_raw,
                last_time_seen: last,
                incidents_at_lap_start: incidents,
                origin: self
                    .origin
                    .filter(|_| self.key.as_ref().is_some_and(|k| k.0 == desc.track_id)),
                ..Self::default()
            };
        }
        self.last_clock = now;

        // --- the lap count: the boundary at once, the official count with its time.
        if in_world {
            if started_raw < self.lap || lap_raw < self.done {
                // The game counts from zero again.
                self.lap = started_raw;
                self.done = lap_raw;
                self.confirmed = lap_raw;
                self.waiting = None;
                self.stale = None;
                self.boundary_s = None;
                self.last_len_s = None;
                self.incidents_at_lap_start = incidents;
            } else if started_raw > self.lap || lap_raw > self.done {
                // A line crossing — one, even when the two counters move a tick apart.
                if self.boundary_s.is_none_or(|b| now - b > SAME_CROSSING_S) {
                    self.last_len_s = self.boundary_s.map(|b| now - b);
                    self.boundary_s = Some(now);
                    self.incidents_at_lap_start = incidents;
                }
                if lap_raw > self.done && self.waiting.is_none() {
                    self.waiting = Some(Waiting {
                        before: self.last_time_seen,
                        since: now,
                        measured: self.last_len_s,
                    });
                }
                self.lap = started_raw;
                self.done = lap_raw;
            }
        }
        if let Some(w) = self.waiting {
            if last != w.before {
                // The time of the lap landed (or −1: the game refused the lap).
                self.confirmed = self.done;
                self.waiting = None;
                self.stale = None;
            } else if now - w.since >= LAP_TIME_WAIT_S {
                self.confirmed = self.done;
                self.waiting = None;
                // Nothing new was published: the lap has no official time — unless it
                // lasted exactly what the previous one did.
                let same = last > 0.0
                    && w.measured
                        .is_some_and(|m| (m - last as f64).abs() <= SAME_TIME_TOLERANCE_S);
                self.stale = (!same).then_some(last);
            }
        }
        if self.stale.is_some_and(|s| s != last) {
            self.stale = None;
        }
        self.last_time_seen = last;

        let fraction = row.f32("LapDistPct").clamp(0.0, 1.0);
        let pos = self.position(row);
        let standings = self.standings(desc, row, player_idx, fraction, pos);
        let session = self.session_info(desc, row, now, session_num, standings.len());
        let player = player_idx.map(|idx| {
            let last_lap_s = if self.stale.is_some() {
                None
            } else {
                time_s(last)
            };
            self.player(
                desc, row, idx, now, fraction, last_lap_s, incidents, &session, pos,
            )
        });
        self.seq += 1;
        TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: self.seq,
            wall_ms: wall_ms_now(),
            source: source_id.to_string(),
            session,
            player,
            standings,
        }
    }

    fn session_info(
        &self,
        desc: &SessionDesc,
        row: Row,
        now: f64,
        session_num: i32,
        cars: usize,
    ) -> SessionInfo {
        let entry = desc.session(session_num);
        let session_flags = row.bits("SessionFlags");
        let remain = row.num("SessionTimeRemain").unwrap_or(-1.0);
        let timed = (0.0..UNLIMITED_TIME_S).contains(&remain);
        let max_laps = match row.i32("SessionLapsTotal") {
            Some(n) if n > 0 && n < UNLIMITED_LAPS => n,
            _ => entry.and_then(|e| e.laps).unwrap_or(0),
        };
        let wind = row.f32("WindVel");
        let dir = row.f32("WindDir");
        // 1 dry … 7 extremely wet (0: not said).
        let wetness = ((row.i32("TrackWetness").unwrap_or(0) - 1).clamp(0, 6) as f32) / 6.0;
        let caution = session_flags & (flags::CAUTION | flags::CAUTION_WAVING) != 0;
        SessionInfo {
            game: GameId::Iracing,
            track_name: desc.track_name.clone(),
            track_length_m: desc.track_length_m,
            session_type: entry.map(|e| e.kind).unwrap_or(SessionType::Unknown),
            session_index: session_num,
            phase: phase(row.i32("SessionState").unwrap_or(0), session_flags),
            yellow_flag: if caution {
                YellowFlagState::Pending
            } else {
                YellowFlagState::None
            },
            sector_flags: [0; 3],
            elapsed_s: now,
            end_s: if timed { now + remain } else { 0.0 },
            time_remaining_s: if timed { remain as f32 } else { 0.0 },
            max_laps,
            num_vehicles: cars as i32,
            // In the car, on the circuit: not the garage screen, not a replay.
            in_realtime: row.flag("IsOnTrack") && !row.flag("IsReplayPlaying"),
            is_fixed_setup: desc.is_fixed_setup,
            player_name: desc.player().map(|d| d.name.clone()).unwrap_or_default(),
            server_name: String::new(),
            is_multiplayer: desc.multiplayer,
            weather: Weather {
                ambient_c: row.f32("AirTemp"),
                track_c: if row.has("TrackTempCrew") {
                    row.f32("TrackTempCrew")
                } else {
                    row.f32("TrackTemp")
                },
                rain: row.f32("Precipitation").clamp(0.0, 1.0),
                wetness_min: wetness,
                wetness_max: wetness,
                wetness_avg: wetness,
                // 0 clear … 3 overcast.
                cloud_darkness: (row.i32("Skies").unwrap_or(0).clamp(0, 3) as f32) / 3.0,
                // TODO(verify-in-game): the origin and the sense of the wind direction.
                wind_ms: [wind * dir.sin(), 0.0, wind * dir.cos()],
                time_of_day_s: row.f32("SessionTimeOfDay"),
                ..Weather::default()
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn player(
        &self,
        desc: &SessionDesc,
        row: Row,
        idx: usize,
        now: f64,
        fraction: f32,
        last_lap_s: Option<f32>,
        incidents: i32,
        session: &SessionInfo,
        pos: [f64; 3],
    ) -> PlayerTelemetry {
        let me = desc.driver(idx);
        let current_lap_s = row.f32("LapCurrentLapTime").max(0.0);
        let lap = LapTiming {
            lap_number: self.lap,
            laps_completed: self.confirmed,
            lap_start_s: now - current_lap_s as f64,
            current_lap_s,
            current_sector: desc.sector_at(fraction),
            in_pit_lane: row.flag("OnPitRoad"),
            lap_invalidated: incidents > self.incidents_at_lap_start,
            delta_best_s: if row.flag("LapDeltaToBestLap_OK") {
                row.f32("LapDeltaToBestLap")
            } else {
                0.0
            },
            last_lap_s,
            best_lap_s: time_s(row.f32("LapBestLapTime")),
            estimated_lap_s: desc.est_lap_s.max(0.0),
            time_into_lap_s: current_lap_s,
            lap_dist_m: if row.has("LapDist") {
                row.f32("LapDist").max(0.0)
            } else {
                fraction * desc.track_length_m
            },
            lap_fraction: fraction,
            ..LapTiming::default()
        };

        // The game's axes: X forward, Y to the left, Z up. Corde's (those of the game it
        // was written for): x to the left, y up, z to the back.
        // TODO(verify-in-game): the signs of the accelerations and of the yaw rate.
        let speed_ms = row.f32("Speed").max(0.0);
        let (lat, long, vert) = (
            row.f32("LatAccel"),
            row.f32("LongAccel"),
            row.f32("VertAccel"),
        );
        let motion = Motion {
            pos,
            local_vel: [
                row.f32("VelocityY"),
                row.f32("VelocityZ"),
                -row.f32("VelocityX"),
            ],
            local_accel: [lat, vert, -long],
            local_rot: [
                row.f32("PitchRate"),
                row.f32("YawRate"),
                row.f32("RollRate"),
            ],
            speed_ms,
            speed_kmh: speed_ms * 3.6,
            // Positive to the right, and when accelerating.
            g_lat: -lat / G,
            g_long: long / G,
            g_vert: vert / G,
            yaw_rate_dps: row.f32("YawRate").to_degrees(),
            heading_rad: row.f32("Yaw"),
        };

        let warnings = row.bits("EngineWarnings");
        let engine = Engine {
            rpm: row.f32("RPM").max(0.0),
            max_rpm: desc.red_line_rpm.max(0.0),
            // −1 reverse, 0 neutral, 1 first: as Corde counts.
            gear: row.i32("Gear").unwrap_or(0).clamp(-1, 12) as i8,
            max_gears: desc.forward_gears,
            water_c: row.f32("WaterTemp"),
            oil_c: row.f32("OilTemp"),
            turbo_boost: row.f32("ManifoldPress"),
            speed_limiter: warnings & engine_warnings::PIT_SPEED_LIMITER != 0,
            speed_limiter_available: true,
            anti_stall: warnings & engine_warnings::ENGINE_STALLED != 0,
            overheating: warnings & (engine_warnings::WATER_TEMP | engine_warnings::OIL_TEMP) != 0,
            ..Engine::default()
        };

        // The wheel: the game counts the angle positive to the left, in radians.
        // TODO(verify-in-game): `SteeringWheelAngleMax` taken as the angle from the
        // centre to one stop.
        let angle = row.f32("SteeringWheelAngle");
        let lock = row.f32("SteeringWheelAngleMax");
        let steering = if lock > 0.0 {
            (-angle / lock).clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let throttle = row.f32("Throttle").clamp(0.0, 1.0);
        let brake = row.f32("Brake").clamp(0.0, 1.0);
        // The game publishes how much the clutch is engaged (1: pedal released).
        let clutch = if row.has("Clutch") {
            (1.0 - row.f32("Clutch")).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let raw = |name: &str, or: f32| {
            if row.has(name) {
                row.f32(name).clamp(0.0, 1.0)
            } else {
                or
            }
        };
        let inputs = Inputs {
            throttle,
            brake,
            steering,
            clutch,
            throttle_raw: raw("ThrottleRaw", throttle),
            brake_raw: raw("BrakeRaw", brake),
            steering_raw: steering,
            clutch_raw: if row.has("ClutchRaw") {
                (1.0 - row.f32("ClutchRaw")).clamp(0.0, 1.0)
            } else {
                clutch
            },
            steering_torque_nm: row.f32("SteeringWheelTorque"),
            steering_angle_deg: -angle.to_degrees(),
            wheel_range_deg: (2.0 * lock).to_degrees().max(0.0),
            ffb_torque: row.f32("SteeringWheelPctTorque"),
        };

        let fuel_l = row.f32("FuelLevel").max(0.0);
        let capacity = desc.fuel_capacity_l.max(0.0);
        let energy = Energy {
            fuel_l,
            fuel_capacity_l: capacity,
            fuel_fraction: if capacity > 0.0 {
                (fuel_l / capacity).clamp(0.0, 1.0)
            } else {
                row.f32("FuelLevelPct").clamp(0.0, 1.0)
            },
            battery_soc: row
                .num("EnergyERSBatteryPct")
                .map(|v| (v as f32).clamp(0.0, 1.0)),
            ..Energy::default()
        };

        // The settings of the car the driver changes from the wheel: each car has its own.
        let bias_front = row.f32("dcBrakeBias");
        let electronics = Electronics {
            tc: small(row.f32("dcTractionControl")),
            tc_slip: small(row.f32("dcTractionControl2")),
            abs: small(row.f32("dcABS")),
            motor_map: small(row.f32("dcThrottleShape")),
            arb_front: small(row.f32("dcAntiRollFront")),
            arb_rear: small(row.f32("dcAntiRollRear")),
            // The game gives the share of the front, in percent.
            brake_bias_rear: if bias_front > 0.0 {
                (1.0 - bias_front / 100.0).clamp(0.0, 1.0)
            } else {
                0.0
            },
            abs_active: row.flag("BrakeABSactive"),
            ..Electronics::default()
        };

        let compound_index = row.i32("PlayerTireCompound").unwrap_or(-1);
        let compound_name = usize::try_from(compound_index)
            .ok()
            .and_then(|i| desc.compounds.get(i))
            .cloned()
            .unwrap_or_default();
        let compound = compound_type(&compound_name);
        let wheels: [Wheel; 4] = std::array::from_fn(|i| {
            let w = WHEELS[i];
            let three = |a: &str, b: &str, c: &str| {
                [
                    row.f32(&format!("{w}{a}")),
                    row.f32(&format!("{w}{b}")),
                    row.f32(&format!("{w}{c}")),
                ]
            };
            // Left, middle, right of the tyre: Corde's order. The carcass is what the game
            // keeps up to date while driving; the surface is only measured in the pits.
            let carcass = three("tempCL", "tempCM", "tempCR");
            let temp_c = if row.has(&format!("{w}tempCL")) {
                carcass
            } else {
                three("tempL", "tempM", "tempR")
            };
            let wear = three("wearL", "wearM", "wearR");
            let worn = row.has(&format!("{w}wearL")) && wear.iter().any(|v| *v > 0.0);
            let hot = format!("{w}pressure");
            Wheel {
                susp_deflection_m: row.f32(&format!("{w}shockDefl")),
                ride_height_m: row.f32(&format!("{w}rideHeight")),
                // As of the last stop: the game does not publish live pressures.
                pressure_kpa: if row.has(&hot) {
                    row.f32(&hot)
                } else {
                    row.f32(&format!("{w}coldPressure"))
                },
                temp_c,
                temp_inner_c: temp_c,
                carcass_c: temp_c.iter().sum::<f32>() / 3.0,
                // Share of the tread left, as of the last stop too; full when not said.
                wear_remaining: if worn {
                    (wear.iter().sum::<f32>() / 3.0).clamp(0.0, 1.0)
                } else {
                    1.0
                },
                compound_index: compound_index.clamp(0, 255) as u8,
                compound_type: compound,
                ..Wheel::default()
            }
        });

        let place = row
            .i32("PlayerCarPosition")
            .or(row.i32_at("CarIdxPosition", idx))
            .unwrap_or(0)
            .clamp(0, 255) as u8;
        let in_stall = row.flag("PlayerCarInPitStall");
        PlayerTelemetry {
            id: idx as i32,
            vehicle_name: me.map(|d| d.car.clone()).unwrap_or_default(),
            vehicle_model: me.map(|d| d.car_path.clone()).unwrap_or_default(),
            vehicle_class: me.map(|d| d.class).unwrap_or(VehicleClass::Unknown),
            class_name: me.map(|d| d.class_name.clone()).unwrap_or_default(),
            championship: String::new(),
            place,
            lap,
            motion,
            engine,
            inputs,
            energy,
            electronics,
            wheels,
            gaps: gaps(desc, row, idx, session.session_type == SessionType::Race),
            tyres: TyreSet {
                front_compound: compound_name.clone(),
                rear_compound: compound_name,
                front_compound_index: compound_index.clamp(0, 255) as u8,
                rear_compound_index: compound_index.clamp(0, 255) as u8,
            },
            in_pits: row.flag("OnPitRoad"),
            pit_state: if in_stall {
                PitState::Stopped
            } else {
                PitState::None
            },
            in_garage: row.flag("IsInGarage"),
            ..PlayerTelemetry::default()
        }
    }

    /// Metres east (x) and south (z) of where the session was first seen, with the
    /// altitude (y). Zero while the game gives no coordinates.
    fn position(&mut self, row: Row) -> [f64; 3] {
        let (Some(lat), Some(lon)) = (row.num("Lat"), row.num("Lon")) else {
            return [0.0; 3];
        };
        if lat == 0.0 && lon == 0.0 {
            return [0.0; 3];
        }
        let (lat0, lon0) = *self.origin.get_or_insert((lat, lon));
        [
            (lon - lon0) * M_PER_DEG * lat0.to_radians().cos(),
            row.f64("Alt"),
            -(lat - lat0) * M_PER_DEG,
        ]
    }

    fn standings(
        &self,
        desc: &SessionDesc,
        row: Row,
        player_idx: Option<usize>,
        player_fraction: f32,
        player_pos: [f64; 3],
    ) -> Vec<Standing> {
        let race = row
            .i32("SessionNum")
            .and_then(|n| desc.session(n))
            .is_some_and(|e| e.kind == SessionType::Race);
        let mut out: Vec<Standing> = desc
            .drivers
            .iter()
            .filter(|d| !d.spectator)
            .filter_map(|d| {
                let i = d.idx;
                let is_player = player_idx == Some(i);
                let surface = row.i32_at("CarIdxTrackSurface", i);
                let in_world = surface.is_some_and(|s| s != track_loc::NOT_IN_WORLD);
                let place = row.i32_at("CarIdxPosition", i).unwrap_or(0).clamp(0, 255) as u8;
                // The pace car only while it is out; the others once they have a place
                // or are on the circuit.
                let shown = if d.pace_car {
                    in_world
                } else {
                    is_player || in_world || place > 0
                };
                if !shown {
                    return None;
                }
                let fraction = if is_player {
                    player_fraction
                } else {
                    row.f32_at("CarIdxLapDistPct", i).clamp(0.0, 1.0)
                };
                Some(Standing {
                    id: i as i32,
                    driver_name: d.name.clone(),
                    vehicle_name: d.car.clone(),
                    class_name: d.class_name.clone(),
                    vehicle_class: d.class,
                    is_player,
                    control: if is_player {
                        Control::LocalPlayer
                    } else if d.ai {
                        Control::LocalAi
                    } else {
                        Control::Remote
                    },
                    place,
                    class_place: row
                        .i32_at("CarIdxClassPosition", i)
                        .unwrap_or(0)
                        .clamp(0, 255) as u8,
                    laps_completed: if is_player {
                        self.confirmed
                    } else {
                        row.i32_at("CarIdxLapCompleted", i).unwrap_or(0).max(0)
                    },
                    sector: desc.sector_at(fraction),
                    lap_dist_m: fraction * desc.track_length_m,
                    lap_fraction: fraction,
                    in_pits: row.flag_at("CarIdxOnPitRoad", i),
                    pit_state: if surface == Some(track_loc::IN_PIT_STALL) {
                        PitState::Stopped
                    } else {
                        PitState::None
                    },
                    in_garage: !in_world && !is_player,
                    // Race: the time behind the leader. The other sessions rank by best
                    // lap, there is no gap on the road.
                    time_behind_leader_s: if race {
                        row.f32_at("CarIdxF2Time", i).max(0.0)
                    } else {
                        0.0
                    },
                    best_lap_s: time_s(row.f32_at("CarIdxBestLapTime", i)),
                    last_lap_s: time_s(row.f32_at("CarIdxLastLapTime", i)),
                    time_into_lap_s: row.f32_at("CarIdxEstTime", i).max(0.0),
                    estimated_lap_s: d.est_lap_s.max(0.0),
                    // The game gives coordinates for the player's car only: the others
                    // are placed by where they are on the lap.
                    pos: if is_player {
                        player_pos.map(|v| v as f32)
                    } else {
                        [0.0; 3]
                    },
                    speed_kmh: if is_player {
                        row.f32("Speed").max(0.0) * 3.6
                    } else {
                        0.0
                    },
                    flag_blue: if is_player {
                        row.bits("SessionFlags") & flags::BLUE != 0
                    } else {
                        row.bits_at("CarIdxSessionFlags", i) & flags::BLUE != 0
                    },
                    ..Standing::default()
                })
            })
            .collect();
        // By place; the cars without one after the others, in the order of the game.
        out.sort_by_key(|s| (s.place == 0, s.place, s.id));
        if race {
            let leader_laps = out.first().map(|s| s.laps_completed).unwrap_or(0);
            let mut ahead: Option<(f32, i32)> = None;
            for s in out.iter_mut().filter(|s| s.place > 0) {
                s.laps_behind_leader = (leader_laps - s.laps_completed).max(0);
                if let Some((time, laps)) = ahead {
                    s.time_behind_next_s = (s.time_behind_leader_s - time).max(0.0);
                    s.laps_behind_next = (laps - s.laps_completed).max(0);
                }
                ahead = Some((s.time_behind_leader_s, s.laps_completed));
            }
        }
        out
    }
}

/// The gaps of the player: to the cars around on the road (any class), and — in a race —
/// to the places ahead and behind.
fn gaps(desc: &SessionDesc, row: Row, me: usize, race: bool) -> Gaps {
    let mut out = Gaps::default();
    // On the road: the time each car needs to reach where it is from the line, compared
    // with ours, around a lap of the expected duration.
    let lap = desc.est_lap_s;
    let mine = row.f32_at("CarIdxEstTime", me);
    if lap > 0.0 && row.has("CarIdxEstTime") {
        for d in desc.drivers.iter().filter(|d| d.idx != me && !d.spectator) {
            let surface = row.i32_at("CarIdxTrackSurface", d.idx);
            if !matches!(surface, Some(track_loc::ON_TRACK | track_loc::OFF_TRACK)) {
                continue;
            }
            let mut diff = (row.f32_at("CarIdxEstTime", d.idx) - mine) % lap;
            if diff > lap / 2.0 {
                diff -= lap;
            } else if diff < -lap / 2.0 {
                diff += lap;
            }
            if diff > 0.0 {
                out.car_ahead_s = Some(out.car_ahead_s.map_or(diff, |v: f32| v.min(diff)));
            } else if diff < 0.0 {
                out.car_behind_s = Some(out.car_behind_s.map_or(-diff, |v: f32| v.min(-diff)));
            }
        }
    }
    if race {
        let place = row.i32_at("CarIdxPosition", me).unwrap_or(0);
        let behind_leader = row.f32_at("CarIdxF2Time", me);
        if place > 0 {
            for d in desc.drivers.iter().filter(|d| d.idx != me) {
                let other = row.i32_at("CarIdxPosition", d.idx).unwrap_or(0);
                let diff = row.f32_at("CarIdxF2Time", d.idx) - behind_leader;
                if other == place - 1 {
                    out.place_ahead_s = Some((-diff).max(0.0));
                } else if other == place + 1 {
                    out.place_behind_s = Some(diff.max(0.0));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_phase_of_the_session() {
        use session_state as st;
        assert_eq!(phase(st::RACING, 0), GamePhase::Green);
        assert_eq!(phase(st::GET_IN_CAR, 0), GamePhase::BeforeSession);
        assert_eq!(phase(st::WARMUP, 0), GamePhase::GridWalk);
        assert_eq!(phase(st::PARADE_LAPS, 0), GamePhase::Formation);
        assert_eq!(
            phase(st::RACING, flags::CAUTION_WAVING),
            GamePhase::FullCourseYellow
        );
        assert_eq!(phase(st::RACING, flags::RED), GamePhase::SessionStopped);
        assert_eq!(phase(st::RACING, flags::CHECKERED), GamePhase::SessionOver);
        assert_eq!(phase(st::COOL_DOWN, 0), GamePhase::SessionOver);
        // A local yellow is not a neutralisation.
        assert_eq!(phase(st::RACING, flags::YELLOW), GamePhase::Green);
        assert_eq!(phase(st::INVALID, 0), GamePhase::Unknown);
    }
}
