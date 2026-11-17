//! The three pages of the game → `corde_protocol::TelemetryFrame`.
//!
//! The game does not publish everything Corde's frame holds. What is missing is either
//! worked out here, or left empty rather than guessed:
//!
//! * **Session clock.** There is none in the pages (only the time left and the time of
//!   day). [`Converter`] keeps one: the seconds spent with the session live, counted on
//!   the caller's monotonic clock. The lap tracker measures the laps on it and checks them
//!   against the official times of the game.
//! * **Lap distance.** The game gives the position along the lap as a fraction. The length
//!   comes from [`crate::tracks`]; for a circuit that is not listed, it is measured on the
//!   first full lap (speed integrated between two line crossings), and until then the
//!   distance covered since the line stands for the lap distance.
//! * **Official lap count.** The counter of the game may move a frame before the time of
//!   the lap that just ended is published. `lap_number` follows the counter at once (the
//!   boundary); `laps_completed` follows once the last lap time changed, or after
//!   [`LAP_TIME_WAIT_S`] — the lap then has no official time, and `last_lap_s` stays empty
//!   until the game publishes another one (the time still shown belongs to an older lap),
//!   unless it is exactly what the lap lasted (two laps in a row with the same time).
//! * **Other cars.** The pages only give their coordinates: no names, places or gaps. The
//!   standings hold the player alone; the gaps to the cars ahead and behind come from the
//!   player's own page.
//! * **Sector times, tyre wear, ride heights, damage detail**: not sent by the game, or not
//!   yet verified — left empty.
//!
//! Everything marked `TODO(verify-in-game)` is an assumption taken from the documentation
//! that still has to be confirmed against the running game.

use crate::layout::{
    flag, session, status, text, Game, Graphics, Physics, Static, MAX_CARS, NO_TIME_MS,
};
use crate::reader::Pages;
use crate::tracks;
use corde_protocol::{
    CompoundType, Control, Electronics, Energy, Engine, GameId, GamePhase, Gaps, Inputs, LapTiming,
    Motion, PlayerTelemetry, SessionInfo, SessionType, Standing, TelemetryFrame, TyreSet,
    VehicleClass, Weather, Wheel, YellowFlagState, PROTOCOL_VERSION,
};
use std::time::Duration;

/// How long (session clock) the official time of a lap may take to appear after the lap
/// counter moved.
pub const LAP_TIME_WAIT_S: f64 = 1.0;
/// Two laps in a row with the very same time: the time that did not change is taken for
/// the new lap when it is this close to what the lap lasted on the session clock.
const SAME_TIME_TOLERANCE_S: f64 = 0.05;
/// The clock never jumps by more than this between two frames (a stalled reader, a
/// machine that slept).
const MAX_STEP_S: f64 = 0.5;
const PSI_TO_KPA: f32 = 6.894_757;
const G: f32 = 9.806_65;

fn wall_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// A time of the game in ms → seconds, `None` when the game has none.
fn time_s(ms: i32) -> Option<f32> {
    (ms > 0 && ms < NO_TIME_MS).then(|| ms as f32 / 1000.0)
}

/// Is this the interface of Competizione? The first Assetto Corsa uses the same mapping
/// names with a shorter layout and stops at version « 1.7 ».
pub fn is_competizione(statics: &Static) -> bool {
    Game::of(statics) == Some(Game::Competizione)
}

impl Game {
    pub fn id(self) -> GameId {
        match self {
            Game::Competizione => GameId::Acc,
            Game::AssettoCorsa => GameId::Ac,
        }
    }
}

/// The compound of the first Assetto Corsa, from its name (« Soft (S) », « Medium (M) »…).
fn compound_of(name: &str) -> CompoundType {
    let n = name.to_ascii_lowercase();
    if n.contains("wet") || n.contains("rain") {
        CompoundType::Wet
    } else if n.contains("soft") {
        CompoundType::Soft
    } else if n.contains("medium") {
        CompoundType::Medium
    } else if n.contains("hard") {
        CompoundType::Hard
    } else {
        CompoundType::Unknown
    }
}

pub fn session_type(raw: i32) -> SessionType {
    match raw {
        session::QUALIFY | session::HOTLAP_SUPERPOLE => SessionType::Qualify,
        session::RACE => SessionType::Race,
        session::PRACTICE
        | session::HOTLAP
        | session::HOTSTINT
        | session::TIME_ATTACK
        | session::DRIFT
        | session::DRAG => SessionType::Practice,
        _ => SessionType::Unknown,
    }
}

fn phase(g: &Graphics) -> GamePhase {
    if g.status == status::PAUSE {
        GamePhase::Paused
    } else if g.global_red != 0 {
        GamePhase::SessionStopped
    } else if g.global_chequered != 0 || g.flag == flag::CHECKERED {
        GamePhase::SessionOver
    } else if g.global_yellow != 0 {
        GamePhase::FullCourseYellow
    } else if g.status == status::LIVE {
        GamePhase::Green
    } else {
        GamePhase::Unknown
    }
}

/// The car class, from the model identifier of the game (« bmw_m4_gt3 », « alpine_a110_gt4 »…).
pub fn car_class(model: &str) -> (VehicleClass, String) {
    let m = model.to_ascii_lowercase();
    let has = |tag: &str| m.split('_').any(|w| w == tag);
    if has("gt3") {
        (VehicleClass::Gt3, "GT3".into())
    } else if has("gt4") {
        (VehicleClass::Unknown, "GT4".into())
    } else if has("gt2") {
        (VehicleClass::Unknown, "GT2".into())
    } else if has("cup") || has("st") || has("st15") || has("challenge") || has("tcx") {
        (VehicleClass::Unknown, "Cup".into())
    } else {
        (VehicleClass::Unknown, String::new())
    }
}

/// What a session is told apart by: the clock and the measures restart when it changes.
#[derive(Debug, Clone, PartialEq)]
struct SessionKey {
    track: String,
    session: i32,
    index: i32,
}

/// Keeps what has to be remembered from one frame to the next (see the module
/// documentation). One per connection to the game.
#[derive(Debug, Default)]
pub struct Converter {
    game: Game,
    seq: u64,
    key: Option<SessionKey>,
    /// Monotonic time of the previous frame.
    last_now: Option<Duration>,
    /// The session clock.
    elapsed_s: f64,
    // --- lap count confirmation
    counter: i32,
    confirmed: i32,
    /// `i_last_time` before the counter moved, when it moved (session clock), and what
    /// the lap lasted on that clock when its start was seen too.
    waiting: Option<(i32, f64, Option<f64>)>,
    /// `i_last_time` of the previous frame.
    last_time_seen: i32,
    /// The published last lap time belongs to an older lap.
    stale: Option<i32>,
    /// Session clock of the last move of the counter.
    boundary_s: Option<f64>,
    // --- lap length
    prev_fraction: Option<f32>,
    /// Metres covered since the last line crossing.
    since_line_m: f64,
    /// `since_line_m` started at a line crossing (not when we began to look).
    from_line: bool,
    measured_length_m: Option<f32>,
}

impl Converter {
    /// For the pages of Competizione.
    pub fn new() -> Self {
        Self::default()
    }

    /// For the pages of `game` (already decoded by [`crate::RawPages::pages_of`]).
    pub fn for_game(game: Game) -> Self {
        Self {
            game,
            ..Self::default()
        }
    }

    /// The session clock, seconds.
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed_s
    }

    /// One frame from one copy of the pages. `now` is a monotonic clock (any origin).
    pub fn frame(&mut self, pages: &Pages, now: Duration, source_id: &str) -> TelemetryFrame {
        let (p, g, s) = (&pages.physics, &pages.graphics, &pages.statics);
        let (track_field, model_field) = (s.track, s.car_model);
        let track_id = text(&track_field);
        let key = SessionKey {
            track: track_id.clone(),
            session: g.session,
            index: g.session_index,
        };
        if self.key.as_ref() != Some(&key) {
            let seq = self.seq;
            *self = Self {
                game: self.game,
                seq,
                key: Some(key),
                counter: g.completed_laps.max(0),
                confirmed: g.completed_laps.max(0),
                last_time_seen: g.i_last_time,
                ..Self::default()
            };
        }
        let live = g.status == status::LIVE;
        let dt = match self.last_now {
            Some(prev) if now > prev => (now - prev).as_secs_f64().min(MAX_STEP_S),
            _ => 0.0,
        };
        self.last_now = Some(now);
        if live {
            self.elapsed_s += dt;
        }

        // --- lap count: the counter at once, the official count with the lap time.
        let laps = g.completed_laps.max(0);
        if laps < self.counter {
            // Restart of the session: the game counts from zero again.
            self.counter = laps;
            self.confirmed = laps;
            self.waiting = None;
            self.stale = None;
            self.boundary_s = None;
        } else if laps > self.counter {
            self.counter = laps;
            let measured = self.boundary_s.map(|b| self.elapsed_s - b);
            self.boundary_s = Some(self.elapsed_s);
            if self.waiting.is_none() {
                self.waiting = Some((self.last_time_seen, self.elapsed_s, measured));
            }
        }
        if let Some((before, since, measured)) = self.waiting {
            if g.i_last_time != before {
                // The time of the lap landed.
                self.confirmed = self.counter;
                self.waiting = None;
                self.stale = None;
            } else if self.elapsed_s - since >= LAP_TIME_WAIT_S {
                self.confirmed = self.counter;
                self.waiting = None;
                // Nothing new was published: the lap has no official time — unless it
                // lasted exactly what the previous one did.
                let same = time_s(before)
                    .zip(measured)
                    .is_some_and(|(t, m)| (m - f64::from(t)).abs() <= SAME_TIME_TOLERANCE_S);
                self.stale = (!same).then_some(before);
            }
        }
        if self.stale.is_some_and(|s| s != g.i_last_time) {
            self.stale = None;
        }
        self.last_time_seen = g.i_last_time;

        // --- lap length and distance.
        let speed_ms = finite(p.speed_kmh).max(0.0) / 3.6;
        let fraction = finite(g.normalized_car_position).clamp(0.0, 1.0);
        if let Some(prev) = self.prev_fraction {
            if prev > 0.9 && fraction < 0.1 {
                // Line crossing.
                if self.from_line && self.since_line_m > 500.0 {
                    self.measured_length_m = Some(self.since_line_m as f32);
                }
                self.since_line_m = 0.0;
                self.from_line = true;
            } else if live {
                self.since_line_m += speed_ms as f64 * dt;
            }
        }
        self.prev_fraction = Some(fraction);
        // The first Assetto Corsa publishes the length; Competizione leaves it at zero.
        let published = finite(s.track_spline_length);
        let known_length = if published > 100.0 {
            Some(published)
        } else {
            tracks::length_m(&track_id).or(self.measured_length_m)
        };
        let track_length_m = known_length.unwrap_or(0.0);
        let lap_dist_m = match known_length {
            Some(len) => fraction * len,
            None => self.since_line_m as f32,
        };

        let model = text(&model_field);
        let session = self.session_info(g, p, s, &track_id, track_length_m);
        let player = self.player(p, g, s, &model, fraction, lap_dist_m);
        let standings = vec![standing(&player, g)];
        self.seq += 1;
        TelemetryFrame {
            v: PROTOCOL_VERSION,
            seq: self.seq,
            wall_ms: wall_ms_now(),
            source: source_id.to_string(),
            session,
            player: Some(player),
            standings,
        }
    }

    fn session_info(
        &self,
        g: &Graphics,
        p: &Physics,
        s: &Static,
        track_id: &str,
        track_length_m: f32,
    ) -> SessionInfo {
        let (name, surname) = (s.player_name, s.player_surname);
        let player_name = format!("{} {}", text(&name), text(&surname))
            .trim()
            .to_string();
        let left_s = finite(g.session_time_left) / 1000.0;
        let rain = (g.rain_intensity.clamp(0, 5) as f32) / 5.0;
        let wind = finite(g.wind_speed);
        let dir = finite(g.wind_direction);
        SessionInfo {
            game: self.game.id(),
            track_name: {
                // The layout, when the circuit has several (the first Assetto Corsa).
                let field = s.track_configuration;
                match text(&field).as_str() {
                    "" => tracks::pretty(track_id),
                    layout => format!("{} — {}", tracks::pretty(track_id), tracks::pretty(layout)),
                }
            },
            track_length_m,
            session_type: session_type(g.session),
            session_index: g.session_index,
            phase: phase(g),
            yellow_flag: if g.global_yellow != 0 {
                YellowFlagState::Pending
            } else {
                YellowFlagState::None
            },
            sector_flags: [
                (g.global_yellow1 != 0) as i8,
                (g.global_yellow2 != 0) as i8,
                (g.global_yellow3 != 0) as i8,
            ],
            elapsed_s: self.elapsed_s,
            end_s: if left_s > 0.0 {
                self.elapsed_s + left_s as f64
            } else {
                0.0
            },
            time_remaining_s: left_s.max(0.0),
            max_laps: 0,
            num_vehicles: g.active_cars.clamp(0, MAX_CARS as i32),
            // Driving, or paused in the car: not in the setup screen, not a replay.
            in_realtime: matches!(g.status, status::LIVE | status::PAUSE)
                && g.is_setup_menu_visible == 0,
            is_fixed_setup: false,
            player_name,
            server_name: String::new(),
            is_multiplayer: s.is_online != 0,
            weather: Weather {
                ambient_c: finite(p.air_temp),
                track_c: finite(p.road_temp),
                rain,
                // 0 green … 6 flooded: wet from « damp » (4) on.
                wetness_avg: ((g.track_grip_status - 3).clamp(0, 3) as f32) / 3.0,
                track_grip_level: Some(g.track_grip_status.clamp(0, 6) as u8),
                // TODO(verify-in-game): the origin and the sense of the wind direction.
                wind_ms: [wind * dir.sin(), 0.0, wind * dir.cos()],
                time_of_day_s: finite(g.clock),
                ..Weather::default()
            },
        }
    }

    fn player(
        &self,
        p: &Physics,
        g: &Graphics,
        s: &Static,
        model: &str,
        fraction: f32,
        lap_dist_m: f32,
    ) -> PlayerTelemetry {
        let (vehicle_class, class_name) = car_class(model);
        let current_lap_s = (g.i_current_time.max(0) as f32) / 1000.0;
        let lap = LapTiming {
            lap_number: self.counter,
            laps_completed: self.confirmed,
            lap_start_s: self.elapsed_s - current_lap_s as f64,
            current_lap_s,
            current_sector: (g.current_sector_index.clamp(0, 2) + 1) as u8,
            in_pit_lane: g.is_in_pit_lane != 0,
            lap_invalidated: g.is_valid_lap == 0,
            delta_best_s: g.i_delta_lap_time as f32 / 1000.0,
            last_lap_s: if self.stale.is_some() {
                None
            } else {
                time_s(g.i_last_time)
            },
            best_lap_s: time_s(g.i_best_time),
            estimated_lap_s: time_s(g.i_estimated_lap_time).unwrap_or(0.0),
            time_into_lap_s: current_lap_s,
            lap_dist_m,
            lap_fraction: fraction,
            ..LapTiming::default()
        };

        let (vel, acc, rot) = (p.local_velocity, p.acc_g, p.local_angular_vel);
        let speed_kmh = finite(p.speed_kmh);
        let motion = Motion {
            pos: player_position(g),
            local_vel: [finite(vel[0]), finite(vel[1]), finite(vel[2])],
            local_accel: [finite(acc[0]) * G, finite(acc[1]) * G, finite(acc[2]) * G],
            local_rot: [finite(rot[0]), finite(rot[1]), finite(rot[2])],
            speed_ms: speed_kmh / 3.6,
            speed_kmh,
            // TODO(verify-in-game): the signs (left / right, braking / accelerating).
            g_lat: finite(acc[0]),
            g_long: finite(acc[2]),
            g_vert: finite(acc[1]),
            yaw_rate_dps: finite(rot[1]).to_degrees(),
            heading_rad: finite(p.heading),
        };

        let engine = Engine {
            rpm: p.rpm.max(0) as f32,
            max_rpm: s.max_rpm.max(0) as f32,
            // The game counts 0 = reverse, 1 = neutral, 2 = first.
            gear: (p.gear - 1).clamp(-1, 12) as i8,
            water_c: finite(p.water_temp),
            turbo_boost: finite(p.turbo_boost),
            ignition_starter: (p.ignition_on != 0) as u8 + (p.starter_engine_on != 0) as u8,
            speed_limiter: p.pit_limiter_on != 0,
            speed_limiter_available: true,
            ..Engine::default()
        };

        let inputs = Inputs {
            throttle: finite(p.gas).clamp(0.0, 1.0),
            brake: finite(p.brake).clamp(0.0, 1.0),
            steering: finite(p.steer_angle).clamp(-1.0, 1.0),
            clutch: finite(p.clutch).clamp(0.0, 1.0),
            throttle_raw: finite(p.gas).clamp(0.0, 1.0),
            brake_raw: finite(p.brake).clamp(0.0, 1.0),
            steering_raw: finite(p.steer_angle).clamp(-1.0, 1.0),
            clutch_raw: finite(p.clutch).clamp(0.0, 1.0),
            ffb_torque: finite(p.final_ff),
            ..Inputs::default()
        };

        // TODO(verify-in-game): the documentation says kg, the game shows litres.
        let fuel_l = finite(p.fuel).max(0.0);
        let capacity = finite(s.max_fuel).max(0.0);
        let energy = Energy {
            fuel_l,
            fuel_capacity_l: capacity,
            fuel_fraction: if capacity > 0.0 {
                (fuel_l / capacity).clamp(0.0, 1.0)
            } else {
                0.0
            },
            ..Energy::default()
        };

        let bias_front = finite(p.brake_bias);
        let electronics = Electronics {
            tc: g.tc.clamp(0, 255) as u8,
            tc_cut: g.tc_cut.clamp(0, 255) as u8,
            abs: g.abs.clamp(0, 255) as u8,
            motor_map: g.engine_map.clamp(0, 255) as u8,
            // Before the per-car offset of the documentation: close to, not equal to, the
            // number shown in the car.
            brake_bias_rear: if bias_front > 0.0 {
                (1.0 - bias_front).clamp(0.0, 1.0)
            } else {
                0.0
            },
            tc_active: finite(p.tc) > 0.0,
            abs_active: finite(p.abs) > 0.0,
            wiper: g.wiper_lv.clamp(0, 255) as u8,
            headlights: g.lights_stage > 0,
            ..Electronics::default()
        };

        let ac = self.game == Game::AssettoCorsa;
        let wet = g.rain_tyres != 0;
        let compound_field = g.tyre_compound;
        let raw_compound = text(&compound_field);
        let (compound_name, compound_type) = if ac {
            // The first Assetto Corsa names its compounds.
            (raw_compound.clone(), compound_of(&raw_compound))
        } else if raw_compound.is_empty() {
            (String::new(), CompoundType::Unknown)
        } else if wet {
            ("Wet".to_string(), CompoundType::Wet)
        } else {
            ("Dry".to_string(), CompoundType::Unknown)
        };
        // What the first Assetto Corsa publishes and Competizione does not.
        let (temp_i, temp_m, temp_o, wear, load, camber, ride) = (
            p.tyre_temp_i,
            p.tyre_temp_m,
            p.tyre_temp_o,
            p.tyre_wear,
            p.wheel_load,
            p.camber_rad,
            p.ride_height,
        );
        let (pressure, core, brake, travel, spin, ratio, angle, radius, press) = (
            p.wheels_pressure,
            p.tyre_core_temperature,
            p.brake_temp,
            p.suspension_travel,
            p.wheel_angular_speed,
            p.slip_ratio,
            p.slip_angle,
            s.tyre_radius,
            p.brake_pressure,
        );
        let wheels: [Wheel; 4] = std::array::from_fn(|i| {
            let t = finite(core[i]);
            Wheel {
                susp_deflection_m: finite(travel[i]),
                brake_temp_c: finite(brake[i]),
                brake_pressure: finite(press[i]),
                rotation_rads: finite(spin[i]),
                pressure_kpa: finite(pressure[i]) * PSI_TO_KPA,
                // Competizione: one temperature per tyre, the core. The first Assetto
                // Corsa: inside / middle / outside, put in the frame's order (left /
                // centre / right of the tyre: the inside of a left wheel is its right).
                temp_c: if !ac {
                    [t; 3]
                } else if i % 2 == 0 {
                    [finite(temp_o[i]), finite(temp_m[i]), finite(temp_i[i])]
                } else {
                    [finite(temp_i[i]), finite(temp_m[i]), finite(temp_o[i])]
                },
                temp_inner_c: [t; 3],
                carcass_c: t,
                // Competizione does not send the wear: full, rather than « worn out ».
                // TODO(verify-in-game): the first Assetto Corsa counts from 100 down.
                wear_remaining: if ac && finite(wear[i]) > 0.0 {
                    (finite(wear[i]) / 100.0).clamp(0.0, 1.0)
                } else {
                    1.0
                },
                load_n: if ac { finite(load[i]) } else { 0.0 },
                camber_deg: if ac {
                    finite(camber[i]).to_degrees()
                } else {
                    0.0
                },
                ride_height_m: if ac { finite(ride[i / 2]) } else { 0.0 },
                compound_type,
                radius_m: finite(radius[i]),
                slip_ratio: finite(ratio[i]),
                // TODO(verify-in-game): radians, by the documentation.
                slip_angle_deg: finite(angle[i]).to_degrees(),
                ..Wheel::default()
            }
        });

        PlayerTelemetry {
            id: g.player_car_id,
            vehicle_name: tracks::pretty(model),
            vehicle_model: model.to_string(),
            vehicle_class,
            class_name,
            championship: String::new(),
            place: g.position.clamp(0, 255) as u8,
            lap,
            motion,
            engine,
            inputs,
            energy,
            electronics,
            wheels,
            gaps: Gaps {
                car_ahead_s: time_s(g.gap_ahead),
                car_behind_s: time_s(g.gap_behind),
                place_ahead_s: time_s(g.gap_ahead),
                place_behind_s: time_s(g.gap_behind),
            },
            tyres: TyreSet {
                front_compound: compound_name.clone(),
                rear_compound: compound_name,
                ..TyreSet::default()
            },
            in_pits: g.is_in_pit != 0 || g.is_in_pit_lane != 0,
            // Competizione says when the setup screen is shown; the first Assetto Corsa
            // only that the car is in its box.
            in_garage: if ac {
                g.is_in_pit != 0
            } else {
                g.is_setup_menu_visible != 0
            },
            ..PlayerTelemetry::default()
        }
    }
}

/// The player's car in the world, from the list of the cars on track.
fn player_position(g: &Graphics) -> [f64; 3] {
    let (ids, coords) = (g.car_id, g.car_coordinates);
    let n = (g.active_cars.clamp(0, MAX_CARS as i32)) as usize;
    let idx = ids[..n]
        .iter()
        .position(|id| *id == g.player_car_id)
        .unwrap_or(0);
    let c = coords[idx];
    [
        finite(c[0]) as f64,
        finite(c[1]) as f64,
        finite(c[2]) as f64,
    ]
}

/// The player's own line of the standings (the pages describe no other driver).
fn standing(p: &PlayerTelemetry, g: &Graphics) -> Standing {
    Standing {
        id: p.id,
        vehicle_name: p.vehicle_name.clone(),
        class_name: p.class_name.clone(),
        vehicle_class: p.vehicle_class,
        is_player: true,
        control: if g.status == status::REPLAY {
            Control::Replay
        } else {
            Control::LocalPlayer
        },
        place: p.place,
        class_place: p.place,
        laps_completed: p.lap.laps_completed,
        sector: p.lap.current_sector,
        lap_dist_m: p.lap.lap_dist_m,
        lap_fraction: p.lap.lap_fraction,
        in_pits: p.in_pits,
        in_garage: p.in_garage,
        best_lap_s: p.lap.best_lap_s,
        last_lap_s: p.lap.last_lap_s,
        time_into_lap_s: p.lap.time_into_lap_s,
        estimated_lap_s: p.lap.estimated_lap_s,
        pos: [
            p.motion.pos[0] as f32,
            p.motion.pos[1] as f32,
            p.motion.pos[2] as f32,
        ],
        speed_kmh: p.motion.speed_kmh,
        fuel_fraction: Some(p.energy.fuel_fraction),
        headlights: p.electronics.headlights,
        ..Standing::default()
    }
}
