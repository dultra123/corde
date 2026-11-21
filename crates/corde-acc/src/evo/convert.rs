//! The pages of Assetto Corsa EVO → a `TelemetryFrame`.
//!
//! What the game publishes, and what it does not:
//!
//! * **Session clock.** Not published: counted here from the time between two frames,
//!   while the game is live (not paused, not a replay).
//! * **Official lap count.** As for Competizione: `lap_number` follows the game's counter
//!   at once (the boundary); `laps_completed` follows once the last lap time changed, or
//!   after [`LAP_TIME_WAIT_S`] — the lap then has no official time unless it lasted what
//!   the previous one did.
//! * **Other cars.** Their positions only (no names, places, laps): the standings hold
//!   the player alone; the gaps to the cars ahead and behind are the game's.
//! * **Sector times.** Not published. The sector shown is a third of the lap.
//! * **Tyre wear.** The field exists and stays at zero in the builds seen so far.
//! * **Red line, tank, levels of the driver aids with their maximum**: published.
//!
//! Everything marked `TODO(verify-in-game)` is an assumption that still has to be
//! confirmed against the running game — the senses of what is lateral, above all, which
//! are taken to be those measured on Competizione.

use super::layout::{
    engine, flag, location, session, status, text, GraphicsEvo, StaticEvo, MAX_CARS,
};
use crate::convert::car_class;
use crate::layout::Physics;
use crate::reader::EvoPages;
use crate::tracks;
use corde_protocol::{
    CompoundType, Control, Electronics, Energy, Engine, GameId, GamePhase, Gaps, Inputs, LapTiming,
    Motion, PlayerTelemetry, SessionInfo, SessionType, Standing, TelemetryFrame, TyreSet, Weather,
    Wheel, YellowFlagState, PROTOCOL_VERSION,
};
use std::time::Duration;

/// How long (session clock) the official time of a lap may take to appear after the lap
/// counter moved.
pub const LAP_TIME_WAIT_S: f64 = 1.0;
/// Two laps in a row with the very same time: the time that did not change is taken for
/// the new lap when it is this close to what the lap lasted on the session clock.
const SAME_TIME_TOLERANCE_S: f64 = 0.05;
/// The clock never jumps by more than this between two frames.
const MAX_STEP_S: f64 = 0.5;
const PSI_TO_KPA: f32 = 6.894_757;
const G: f32 = 9.806_65;

fn wall_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// An angle brought back between −π and π.
fn wrap(a: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (a + PI).rem_euclid(TAU) - PI
}

fn finite(v: f32) -> f32 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

/// A time of the game in ms → seconds, `None` when the game has none (0, −1, or the
/// largest integer).
fn time_s(ms: i32) -> Option<f32> {
    (ms > 0 && ms < i32::MAX).then(|| ms as f32 / 1000.0)
}

/// A level of the wheel's settings (the game writes signed bytes).
fn level(v: i8) -> u8 {
    v.clamp(0, i8::MAX) as u8
}

/// The compound, from its name (« Soft », « Medium », « Wet »…).
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

/// The kind of session. The game's own list has no qualifying: it is recognised by the
/// name the game gives the session or its phase.
pub fn session_type(raw: i32, name: &str, phase: &str) -> SessionType {
    let names = format!("{name} {phase}").to_ascii_lowercase();
    if names.contains("qual") {
        return SessionType::Qualify;
    }
    match raw {
        session::RACE => SessionType::Race,
        session::TIME_ATTACK | session::HOT_STINT | session::CRUISE => SessionType::Practice,
        _ if names.contains("race") => SessionType::Race,
        _ if names.contains("prac") => SessionType::Practice,
        _ => SessionType::Unknown,
    }
}

fn phase(g: &GraphicsEvo) -> GamePhase {
    if g.status == status::PAUSE {
        GamePhase::Paused
    } else if g.global_flag == flag::RED {
        GamePhase::SessionStopped
    } else if g.global_flag == flag::CHECKERED || g.flag == flag::CHECKERED {
        GamePhase::SessionOver
    } else if g.global_flag == flag::YELLOW {
        GamePhase::FullCourseYellow
    } else if g.status == status::LIVE {
        GamePhase::Green
    } else {
        GamePhase::Unknown
    }
}

/// A readable name from an identifier of the game (its own content starts with « ks_ »).
fn pretty(id: &str) -> String {
    let id = id.trim();
    let bare = id
        .get(..3)
        .filter(|p| p.eq_ignore_ascii_case("ks_"))
        .map_or(id, |_| &id[3..]);
    tracks::pretty(bare)
}

/// What a session is told apart by: the clock and the counts restart when it changes.
#[derive(Debug, Clone, PartialEq)]
struct SessionKey {
    track: String,
    layout: String,
    session: i32,
    event: u8,
    index: u8,
}

/// The lap counter of the game, and the official count that follows it.
#[derive(Debug, Default)]
struct LapCount {
    counter: i32,
    confirmed: i32,
    /// The last lap time before the counter moved, when it moved (session clock), and
    /// what the lap lasted on that clock when its start was seen too.
    waiting: Option<(i32, f64, Option<f64>)>,
    /// The last lap time of the previous frame.
    last_time_seen: i32,
    /// The published last lap time belongs to an older lap.
    stale: Option<i32>,
    /// Session clock of the last move of the counter.
    boundary_s: Option<f64>,
}

impl LapCount {
    fn starting_at(laps: i32, last_time: i32) -> Self {
        Self {
            counter: laps,
            confirmed: laps,
            last_time_seen: last_time,
            ..Self::default()
        }
    }

    fn step(&mut self, laps: i32, last_time: i32, elapsed_s: f64) {
        if laps < self.counter {
            // Restart of the session: the game counts from zero again.
            *self = Self::starting_at(laps, last_time);
        } else if laps > self.counter {
            self.counter = laps;
            let measured = self.boundary_s.map(|b| elapsed_s - b);
            self.boundary_s = Some(elapsed_s);
            if self.waiting.is_none() {
                self.waiting = Some((self.last_time_seen, elapsed_s, measured));
            }
        }
        if let Some((before, since, measured)) = self.waiting {
            if last_time != before {
                // The time of the lap landed.
                self.confirmed = self.counter;
                self.waiting = None;
                self.stale = None;
            } else if elapsed_s - since >= LAP_TIME_WAIT_S {
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
        if self.stale.is_some_and(|s| s != last_time) {
            self.stale = None;
        }
        self.last_time_seen = last_time;
    }
}

/// Keeps what has to be remembered from one frame to the next. One per connection to the
/// game.
#[derive(Debug, Default)]
pub struct EvoConverter {
    seq: u64,
    key: Option<SessionKey>,
    /// Monotonic time of the previous frame.
    last_now: Option<Duration>,
    /// The session clock.
    elapsed_s: f64,
    laps: LapCount,
}

impl EvoConverter {
    pub fn new() -> Self {
        Self::default()
    }

    /// The session clock, seconds.
    pub fn elapsed_s(&self) -> f64 {
        self.elapsed_s
    }

    /// One frame from one copy of the pages. `now` is a monotonic clock (any origin).
    pub fn frame(&mut self, pages: &EvoPages, now: Duration, source_id: &str) -> TelemetryFrame {
        let (p, g, s) = (&pages.physics, &pages.graphics, &pages.statics);
        let (track_field, layout_field) = (s.track, s.track_configuration);
        let track_id = text(&track_field);
        let layout_id = text(&layout_field);
        let key = SessionKey {
            track: track_id.clone(),
            layout: layout_id.clone(),
            session: s.session,
            event: s.event_id,
            index: s.session_id,
        };
        if self.key.as_ref() != Some(&key) {
            *self = Self {
                seq: self.seq,
                key: Some(key),
                laps: LapCount::starting_at(g.total_lap_count.max(0), g.last_laptime_ms),
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
        self.laps
            .step(g.total_lap_count.max(0), g.last_laptime_ms, self.elapsed_s);

        let fraction = finite(g.npos).clamp(0.0, 1.0);
        // The static page gives the length; the session's own summary repeats it in km.
        let lap_km = g.session_state.lap_length_km;
        let track_length_m = [finite(s.track_length_m), finite(lap_km) * 1000.0]
            .into_iter()
            .find(|len| *len > 100.0)
            .unwrap_or(0.0);

        let session = self.session_info(g, p, s, &track_id, &layout_id, track_length_m);
        let player = self.player(p, g, fraction, fraction * track_length_m);
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
        g: &GraphicsEvo,
        p: &Physics,
        s: &StaticEvo,
        track_id: &str,
        layout_id: &str,
        track_length_m: f32,
    ) -> SessionInfo {
        let (name, surname) = (g.driver_name, g.driver_surname);
        let player_name = format!("{} {}", text(&name), text(&surname))
            .trim()
            .to_string();
        let state = g.session_state;
        let (session_name, phase_name) = (s.session_name, state.phase_name);
        let left_s = (state.time_left_ms.max(0) as f32) / 1000.0;
        // The air of the physics page; the temperature the session started with otherwise.
        let or = |live: f32, start: f32| {
            let live = finite(live);
            if live != 0.0 {
                live
            } else {
                finite(start)
            }
        };
        SessionInfo {
            game: GameId::AcEvo,
            track_name: match tracks::layout(layout_id) {
                "" => pretty(track_id),
                layout => format!("{} — {}", pretty(track_id), pretty(layout)),
            },
            track_length_m,
            session_type: session_type(s.session, &text(&session_name), &text(&phase_name)),
            session_index: i32::from(s.session_id),
            phase: phase(g),
            yellow_flag: if g.global_flag == flag::YELLOW {
                YellowFlagState::Pending
            } else {
                YellowFlagState::None
            },
            sector_flags: [0; 3],
            elapsed_s: self.elapsed_s,
            end_s: if left_s > 0.0 {
                self.elapsed_s + f64::from(left_s)
            } else {
                0.0
            },
            time_remaining_s: left_s,
            max_laps: if s.is_timed_race == 0 {
                state.total_lap.max(0)
            } else {
                0
            },
            num_vehicles: (g.total_drivers.min(MAX_CARS as u32) as i32)
                .max(i32::from(g.active_cars).min(MAX_CARS as i32)),
            // Driving, or paused in the car: not a replay.
            in_realtime: matches!(g.status, status::LIVE | status::PAUSE),
            is_fixed_setup: false,
            player_name,
            server_name: String::new(),
            is_multiplayer: s.is_online != 0,
            weather: Weather {
                ambient_c: or(p.air_temp, s.starting_ambient_temperature_c),
                track_c: or(p.road_temp, s.starting_ground_temperature_c),
                // 0 green, 1 fast, 2 optimum: the first steps of Competizione's scale.
                track_grip_level: Some(s.starting_grip.clamp(0, 2) as u8),
                time_of_day_s: (g.time_of_day_hours.clamp(0, 23) * 3600
                    + g.time_of_day_minutes.clamp(0, 59) * 60
                    + g.time_of_day_seconds.clamp(0, 59)) as f32,
                ..Weather::default()
            },
        }
    }

    fn player(
        &self,
        p: &Physics,
        g: &GraphicsEvo,
        fraction: f32,
        lap_dist_m: f32,
    ) -> PlayerTelemetry {
        let model_field = g.car_model;
        let model = text(&model_field);
        let (vehicle_class, class_name) = car_class(&model);
        let timing = g.timing_state;
        let in_pit_lane = g.is_in_pit_lane != 0
            || matches!(
                g.car_location,
                location::PIT_LANE | location::PIT_ENTRY | location::PIT_EXIT
            );
        let current_lap_s = (g.current_lap_time_ms.max(0) as f32) / 1000.0;
        let lap = LapTiming {
            lap_number: self.laps.counter,
            laps_completed: self.laps.confirmed,
            lap_start_s: self.elapsed_s - f64::from(current_lap_s),
            current_lap_s,
            // The game publishes no sector: a third of the lap each.
            current_sector: ((fraction * 3.0) as u8).min(2) + 1,
            in_pit_lane,
            // Two fields say it; a lap is only marked when both do, so that one the game
            // would leave at zero does not mark every lap.
            lap_invalidated: g.is_valid_lap == 0 && timing.is_invalid != 0,
            delta_best_s: g.delta_time_ms as f32 / 1000.0,
            last_lap_s: if self.laps.stale.is_some() {
                None
            } else {
                time_s(g.last_laptime_ms)
            },
            best_lap_s: time_s(g.best_laptime_ms),
            estimated_lap_s: time_s(g.predicted_lap_time_ms).unwrap_or(0.0),
            time_into_lap_s: current_lap_s,
            lap_dist_m,
            lap_fraction: fraction,
            ..LapTiming::default()
        };

        let (vel, acc, rot) = (p.local_velocity, p.acc_g, p.local_angular_vel);
        let speed_kmh = finite(p.speed_kmh);
        // The documentation of this game orders the accelerations [lateral, longitudinal,
        // vertical] (not Competizione's order) and takes gravity out of the vertical one.
        // TODO(verify-in-game): that order, and the senses below (those of Competizione:
        // the game's world taken as the mirror of Corde's).
        let motion = Motion {
            pos: player_position(g),
            local_vel: [-finite(vel[0]), finite(vel[1]), finite(vel[2])],
            local_accel: [-finite(acc[0]) * G, finite(acc[2]) * G, finite(acc[1]) * G],
            local_rot: [finite(rot[0]), -finite(rot[1]), -finite(rot[2])],
            speed_ms: speed_kmh / 3.6,
            speed_kmh,
            g_lat: -finite(acc[0]),
            g_long: finite(acc[1]),
            g_vert: finite(acc[2]),
            yaw_rate_dps: -finite(rot[1]).to_degrees(),
            heading_rad: wrap(std::f32::consts::PI - finite(p.heading)),
        };

        let (now, max) = (g.electronics, g.electronics_max_limit);
        let lights = g.instrumentation;
        let engine = Engine {
            rpm: if p.rpm > 0 {
                p.rpm as f32
            } else {
                f32::from(g.rpm)
            },
            max_rpm: p.current_max_rpm.max(0) as f32,
            // The game counts 0 = reverse, 1 = neutral, 2 = first.
            gear: (p.gear - 1).clamp(-1, 12) as i8,
            max_gears: g.max_gears.min(12) as u8,
            water_c: if finite(p.water_temp) != 0.0 {
                finite(p.water_temp)
            } else {
                f32::from(g.water_temperature_c)
            },
            oil_c: finite(g.oil_temperature_c),
            torque_nm: finite(g.current_torque),
            turbo_boost: finite(p.turbo_boost),
            ignition_starter: (p.ignition_on != 0 || g.is_ignition_on != 0) as u8
                + (p.starter_engine_on != 0) as u8,
            speed_limiter: p.pit_limiter_on != 0 || now.is_pitlimiter_on != 0,
            speed_limiter_available: true,
            ..Engine::default()
        };

        let steering = finite(g.steering_percent).clamp(-1.0, 1.0);
        // The pedal as the game's own display shows it.
        // TODO(verify-in-game): 0 with the pedal released.
        let clutch = finite(g.clutch_percent).clamp(0.0, 1.0);
        let inputs = Inputs {
            throttle: finite(p.gas).clamp(0.0, 1.0),
            brake: finite(p.brake).clamp(0.0, 1.0),
            steering,
            clutch,
            throttle_raw: finite(p.gas).clamp(0.0, 1.0),
            brake_raw: finite(p.brake).clamp(0.0, 1.0),
            steering_raw: steering,
            clutch_raw: clutch,
            steering_angle_deg: g.steer_degrees.clamp(-3600, 3600) as f32,
            ffb_torque: finite(p.final_ff),
            ..Inputs::default()
        };

        let fuel_l = [finite(g.fuel_liter_current_quantity), finite(p.fuel)]
            .into_iter()
            .find(|l| *l > 0.0)
            .unwrap_or(0.0);
        let capacity = finite(g.max_fuel).max(0.0);
        let electric = g.engine_type == engine::ELECTRIC;
        let energy = Energy {
            fuel_l,
            fuel_capacity_l: capacity,
            fuel_fraction: if capacity > 0.0 {
                (fuel_l / capacity).clamp(0.0, 1.0)
            } else {
                finite(g.fuel_liter_current_quantity_percent).clamp(0.0, 1.0)
            },
            battery_soc: (g.has_kers != 0 || electric)
                .then(|| finite(g.kers_charge_perc).clamp(0.0, 1.0)),
            ..Energy::default()
        };

        let bias_front = [finite(now.brake_bias), finite(p.brake_bias)]
            .into_iter()
            .find(|b| *b > 0.0)
            .unwrap_or(0.0);
        let electronics = Electronics {
            tc: level(now.tc_level),
            tc_max: level(max.tc_level),
            tc_cut: level(now.tc_cut_level),
            tc_cut_max: level(max.tc_cut_level),
            abs: level(now.abs_level),
            abs_max: level(max.abs_level),
            motor_map: level(now.engine_map_level),
            motor_map_max: level(max.engine_map_level),
            brake_bias_rear: if bias_front > 0.0 {
                (1.0 - bias_front).clamp(0.0, 1.0)
            } else {
                0.0
            },
            tc_active: p.tc_in_action != 0 || g.tc_active != 0,
            abs_active: p.abs_in_action != 0 || g.abs_active != 0,
            wiper: level(lights.wiper_level),
            headlights: lights.main_light_stage > 0,
            ..Electronics::default()
        };

        let tyres = g.tyres();
        let (front_field, rear_field) = (tyres[0].compound_front, tyres[0].compound_rear);
        let (front_compound, rear_compound) = (text(&front_field), text(&rear_field));
        let (travel, spin, camber, load, core, wear, ride) = (
            p.suspension_travel,
            p.wheel_angular_speed,
            p.camber_rad,
            p.wheel_load,
            p.tyre_core_temperature,
            p.tyre_wear,
            p.ride_height,
        );
        let (psi, disc, fx, fy, ratio, angle) = (
            p.wheels_pressure,
            p.brake_temp,
            p.fx,
            p.fy,
            p.slip_ratio,
            p.slip_angle,
        );
        let wheels: [Wheel; 4] = std::array::from_fn(|i| {
            let t = tyres[i];
            let first = |a: f32, b: f32| {
                if finite(a) != 0.0 {
                    finite(a)
                } else {
                    finite(b)
                }
            };
            let carcass = first(core[i], t.temperature_c);
            // Metres; some cars write millimetres in the same field (no car rides a
            // metre high).
            let height = finite(ride[i / 2]);
            let name = if i < 2 {
                &front_compound
            } else {
                &rear_compound
            };
            // The camber of a right wheel has the other sign: brought back to one sense
            // (negative: the top of the wheel leans in).
            let side = if i % 2 == 0 { 1.0 } else { -1.0 };
            Wheel {
                susp_deflection_m: finite(travel[i]),
                ride_height_m: if height.abs() >= 1.0 {
                    height / 1000.0
                } else {
                    height
                },
                brake_temp_c: first(t.brake_temperature_c, disc[i]),
                brake_pressure: finite(t.brake_pressure),
                rotation_rads: finite(spin[i]),
                camber_deg: side * finite(camber[i]).to_degrees(),
                lateral_force_n: finite(fy[i]),
                longitudinal_force_n: finite(fx[i]),
                load_n: finite(load[i]),
                pressure_kpa: first(t.pressure, psi[i]) * PSI_TO_KPA,
                // Left, centre and right of the tread, as the car sees them: the order of
                // the frame.
                temp_c: [
                    finite(t.temperature_left),
                    finite(t.temperature_center),
                    finite(t.temperature_right),
                ],
                temp_inner_c: [carcass; 3],
                carcass_c: carcass,
                // 0 new … 1 worn out by the documentation; left at zero by the game so
                // far, which reads as « new ».
                wear_remaining: (1.0 - finite(wear[i])).clamp(0.0, 1.0),
                compound_type: compound_of(name),
                slip_ratio: finite(ratio[i]),
                slip_angle_deg: finite(angle[i]).to_degrees(),
                ..Wheel::default()
            }
        });

        let gap = |s: f32| (s.is_finite() && s > 0.0).then_some(s);
        PlayerTelemetry {
            id: player_slot(g) as i32,
            vehicle_name: pretty(&model),
            vehicle_model: pretty(&model),
            vehicle_class,
            class_name,
            championship: String::new(),
            place: g.current_pos.min(255) as u8,
            lap,
            motion,
            engine,
            inputs,
            energy,
            electronics,
            wheels,
            gaps: Gaps {
                car_ahead_s: gap(g.gap_ahead),
                car_behind_s: gap(g.gap_behind),
                place_ahead_s: gap(g.gap_ahead),
                place_behind_s: gap(g.gap_behind),
            },
            tyres: TyreSet {
                front_compound,
                rear_compound,
                ..TyreSet::default()
            },
            in_pits: in_pit_lane || g.is_in_pit_box != 0,
            in_garage: g.is_in_pit_box != 0,
            ..PlayerTelemetry::default()
        }
    }
}

/// The slot of the player's car in the table of positions: the one that carries its
/// identifier, the first when the game gives no identifier (builds before April 2026).
pub fn player_slot(g: &GraphicsEvo) -> usize {
    let (ids, me) = (g.car_ids, g.player_car_id);
    if me == [0, 0] {
        return 0;
    }
    ids.iter().position(|id| *id == me).unwrap_or(0)
}

/// The player's car in the world.
fn player_position(g: &GraphicsEvo) -> [f64; 3] {
    let coords = g.car_coordinates;
    let c = coords[player_slot(g)];
    // TODO(verify-in-game): as for Competizione, the third axis is turned round so that a
    // circuit that turns left is drawn turning left.
    [
        f64::from(finite(c[0])),
        f64::from(finite(c[1])),
        -f64::from(finite(c[2])),
    ]
}

/// The player's own line of the standings (the pages describe no other driver).
fn standing(p: &PlayerTelemetry, g: &GraphicsEvo) -> Standing {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kind_of_session_comes_from_the_list_and_the_names() {
        assert_eq!(session_type(session::RACE, "Race 1", ""), SessionType::Race);
        assert_eq!(
            session_type(session::TIME_ATTACK, "", ""),
            SessionType::Practice
        );
        assert_eq!(
            session_type(session::HOT_STINT, "Hot stint", ""),
            SessionType::Practice
        );
        // No qualifying in the game's list: the names say it.
        assert_eq!(
            session_type(session::TIME_ATTACK, "Qualifying", ""),
            SessionType::Qualify
        );
        assert_eq!(
            session_type(session::UNKNOWN, "", "Qualify"),
            SessionType::Qualify
        );
        assert_eq!(
            session_type(session::UNKNOWN, "", "Race"),
            SessionType::Race
        );
        assert_eq!(session_type(session::UNKNOWN, "", ""), SessionType::Unknown);
        assert_eq!(session_type(7, "Practice", ""), SessionType::Practice);
    }

    #[test]
    fn names_lose_the_prefix_of_the_game() {
        assert_eq!(pretty("ks_brands_hatch"), "Brands Hatch");
        assert_eq!(pretty("KS_mount_panorama"), "Mount Panorama");
        assert_eq!(pretty("imola"), "Imola");
        assert_eq!(pretty("ks"), "Ks");
        assert_eq!(pretty(""), "");
    }

    #[test]
    fn times_and_levels_are_read_with_care() {
        assert_eq!(time_s(0), None);
        assert_eq!(time_s(-1), None);
        assert_eq!(time_s(i32::MAX), None);
        assert_eq!(time_s(83_456), Some(83.456));
        assert_eq!(level(-3), 0);
        assert_eq!(level(11), 11);
    }
}
