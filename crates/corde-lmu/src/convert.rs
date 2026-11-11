//! `SharedMemoryObjectOut` → [`TelemetryFrame`].
//!
//! Unit conventions of the protocol (see `docs/ARCHITECTURE.md` §4): °C, km/h
//! and m/s, degrees, kPa, litres, seconds, millimetres for aero heights.
//!
//! Frames of reference: the rFactor family uses a left-handed world with +y up,
//! and local vehicle axes **+x = left, +y = up, +z = back**; rotations: +x pitch
//! up, +y yaw right, +z roll right. The protocol wants ISO-like signs for the
//! g-forces (lateral positive to the right, longitudinal positive when
//! accelerating) and yaw rate positive when turning right.

use crate::layout::{
    as_bool, championship, vehicle_class, SharedMemoryObjectOut, TelemInfoV01, TelemWheelV01,
    VehicleScoringInfoV01, MAX_VEHICLES,
};
use corde_core::derive::{
    heading_from_ori_row2, k_to_c, ms_to_kmh, rad_to_deg, slip_angle_deg, slip_ratio, to_g,
};
use corde_protocol::*;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// A lap / sector time: `-1` and `0` are "no time".
#[inline]
pub fn opt_time(v: f64) -> Option<f32> {
    if v.is_finite() && v > 0.0 {
        Some(v as f32)
    } else {
        None
    }
}

/// Individual sector 2 time from the cumulative `sector2` (S1 + S2) value.
#[inline]
pub fn opt_sector2(sector1: f64, sector2_cumulative: f64) -> Option<f32> {
    match (opt_time(sector1), opt_time(sector2_cumulative)) {
        (Some(s1), Some(s12)) if s12 > s1 => Some(s12 - s1),
        _ => None,
    }
}

/// Sector 3 time from a lap time and the cumulative sector 2.
#[inline]
pub fn opt_sector3(sector2_cumulative: f64, lap_time: f64) -> Option<f32> {
    match (opt_time(sector2_cumulative), opt_time(lap_time)) {
        (Some(s12), Some(lap)) if lap > s12 => Some(lap - s12),
        _ => None,
    }
}

/// A time gap reported by the game: positive and finite, else unknown.
#[inline]
fn opt_gap(v: f32) -> Option<f32> {
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

#[inline]
fn m_to_mm(m: f64) -> f32 {
    (m * 1000.0) as f32
}

#[inline]
fn wall_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `mSession` → [`SessionType`] (0 test day, 1-4 practice, 5-8 qualify, 9 warmup, 10-13 race).
pub fn session_type(session: i32) -> SessionType {
    match session {
        0 => SessionType::TestDay,
        1..=4 => SessionType::Practice,
        5..=8 => SessionType::Qualify,
        9 => SessionType::Warmup,
        10..=13 => SessionType::Race,
        _ => SessionType::Unknown,
    }
}

pub fn game_phase(v: u8) -> GamePhase {
    match v {
        0 => GamePhase::BeforeSession,
        1 => GamePhase::Reconnaissance,
        2 => GamePhase::GridWalk,
        3 => GamePhase::Formation,
        4 => GamePhase::Countdown,
        5 => GamePhase::Green,
        6 => GamePhase::FullCourseYellow,
        7 => GamePhase::SessionStopped,
        8 => GamePhase::SessionOver,
        9 => GamePhase::Paused,
        _ => GamePhase::Unknown,
    }
}

pub fn yellow_flag(v: i8) -> YellowFlagState {
    match v {
        -1 => YellowFlagState::Invalid,
        0 => YellowFlagState::None,
        1 => YellowFlagState::Pending,
        2 => YellowFlagState::PitsClosed,
        3 => YellowFlagState::PitLeadLap,
        4 => YellowFlagState::PitsOpen,
        5 => YellowFlagState::LastLap,
        6 => YellowFlagState::Resume,
        7 => YellowFlagState::RaceHalt,
        _ => YellowFlagState::Invalid,
    }
}

pub fn surface_type(v: u8) -> SurfaceType {
    match v {
        0 => SurfaceType::Dry,
        1 => SurfaceType::Wet,
        2 => SurfaceType::Grass,
        3 => SurfaceType::Dirt,
        4 => SurfaceType::Gravel,
        5 => SurfaceType::Rumble,
        6 => SurfaceType::Special,
        _ => SurfaceType::Unknown,
    }
}

pub fn compound_type(v: u8) -> CompoundType {
    match v {
        0 => CompoundType::Soft,
        1 => CompoundType::Medium,
        2 => CompoundType::Hard,
        3 => CompoundType::Wet,
        _ => CompoundType::Unknown,
    }
}

pub fn pit_state(v: u8) -> PitState {
    match v {
        0 => PitState::None,
        1 => PitState::Request,
        2 => PitState::Entering,
        3 => PitState::Stopped,
        4 => PitState::Exiting,
        _ => PitState::None,
    }
}

pub fn motor_state(v: u8) -> MotorState {
    match v {
        1 => MotorState::Inactive,
        2 => MotorState::Propulsion,
        3 => MotorState::Regeneration,
        _ => MotorState::Unavailable,
    }
}

pub fn finish_status(v: i8) -> FinishStatus {
    match v {
        1 => FinishStatus::Finished,
        2 => FinishStatus::Dnf,
        3 => FinishStatus::Dq,
        _ => FinishStatus::None,
    }
}

pub fn control(v: i8) -> Control {
    match v {
        -1 => Control::Nobody,
        0 => Control::LocalPlayer,
        1 => Control::LocalAi,
        2 => Control::Remote,
        3 => Control::Replay,
        _ => Control::Unknown,
    }
}

/// `IP_VehicleClass` (telemetry enum) → [`VehicleClass`].
pub fn vehicle_class_from_enum(v: u8) -> VehicleClass {
    match v {
        vehicle_class::HYPERCAR => VehicleClass::Hypercar,
        vehicle_class::LMP2_ELMS | vehicle_class::LMP2 => VehicleClass::Lmp2,
        vehicle_class::LMP3 => VehicleClass::Lmp3,
        vehicle_class::GTE => VehicleClass::Gte,
        vehicle_class::GT3 => VehicleClass::Gt3,
        vehicle_class::PACE_CAR => VehicleClass::PaceCar,
        _ => VehicleClass::Unknown,
    }
}

/// Scoring class string ("Hyper", "LMP2", "GT3", "LMGT3", "LMP3", "GTE"…) → [`VehicleClass`].
///
/// TODO(verify-on-pc): record the exact `mVehicleClass` strings of the native
/// interface on a multi-class grid ("Hyper" / "LMP2" / "GT3" were observed with
/// the legacy plugin only).
pub fn vehicle_class_from_name(name: &str) -> VehicleClass {
    let n = name.trim().to_ascii_lowercase();
    if n.is_empty() {
        return VehicleClass::Unknown;
    }
    if n.contains("hyper") || n.contains("lmh") || n.contains("lmdh") {
        VehicleClass::Hypercar
    } else if n.contains("lmp2") || n.contains("p2") {
        VehicleClass::Lmp2
    } else if n.contains("lmp3") || n.contains("p3") {
        VehicleClass::Lmp3
    } else if n.contains("gte") {
        VehicleClass::Gte
    } else if n.contains("gt3") {
        VehicleClass::Gt3
    } else if n.contains("pace") || n.contains("safety") {
        VehicleClass::PaceCar
    } else {
        VehicleClass::Unknown
    }
}

/// `IP_VehicleChampionship` → display string.
pub fn championship_name(v: u8) -> String {
    match v {
        championship::WEC_2023 => "WEC 2023".into(),
        championship::WEC_2024 => "WEC 2024".into(),
        championship::WEC_2025 => "WEC 2025".into(),
        championship::WEC_2026 => "WEC 2026".into(),
        championship::ELMS_2025 => "ELMS 2025".into(),
        championship::ELMS_2026 => "ELMS 2026".into(),
        championship::UNKNOWN => String::new(),
        other => format!("championship {other}"),
    }
}

/// `VehicleScoringInfoV01::mSector` (0 = S3, 1 = S1, 2 = S2) → 1..3.
#[inline]
pub fn scoring_sector(v: i8) -> u8 {
    match v {
        1 => 1,
        2 => 2,
        _ => 3,
    }
}

/// `TelemInfoV01::mCurrentSector` (zero-based, pit lane in the sign bit) → (sector 1..3, in pit lane).
#[inline]
pub fn telemetry_sector(v: i32) -> (u8, bool) {
    let in_pits = v < 0;
    let idx = (v & 0x7FFF_FFFF).clamp(0, 2) as u8;
    (idx + 1, in_pits)
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

pub fn session_info(raw: &SharedMemoryObjectOut) -> SessionInfo {
    let s = &raw.scoring.scoring_info;
    let (session, current_et, end_et, max_laps, lap_dist, num_vehicles) = (
        s.session,
        s.current_et,
        s.end_et,
        s.max_laps,
        s.lap_dist,
        s.num_vehicles,
    );
    let (ambient, track, raining, min_w, max_w, avg_w, dark, wind, tod, remaining) = (
        s.ambient_temp,
        s.track_temp,
        s.raining,
        s.min_path_wetness,
        s.max_path_wetness,
        s.avg_path_wetness,
        s.dark_cloud,
        s.wind,
        s.time_of_day,
        s.session_time_remaining,
    );
    let (game_mode, grip, cloud) = (s.game_mode, s.track_grip_level, s.cloud_coverage);
    SessionInfo {
        game: GameId::Lmu,
        track_name: s.track_name(),
        track_length_m: lap_dist as f32,
        session_type: session_type(session),
        session_index: session,
        phase: game_phase(s.game_phase),
        yellow_flag: yellow_flag(s.yellow_flag_state),
        sector_flags: s.sector_flag,
        elapsed_s: current_et,
        end_s: end_et,
        time_remaining_s: remaining,
        max_laps,
        num_vehicles,
        in_realtime: as_bool(s.in_realtime),
        is_fixed_setup: as_bool(s.is_fixed_setup),
        player_name: s.player_name(),
        server_name: s.server_name(),
        // TODO(verify-on-pc): mGameMode offline value (0 or 1?); 2 = client,
        // 3 = server+client are taken as multiplayer.
        is_multiplayer: game_mode >= 2,
        weather: Weather {
            ambient_c: ambient as f32,
            track_c: track as f32,
            rain: raining as f32,
            wetness_min: min_w as f32,
            wetness_max: max_w as f32,
            wetness_avg: avg_w as f32,
            cloud_darkness: dark as f32,
            // LMU ≥ 1.3 fields: the game writes 0 on older builds, which is
            // indistinguishable from "clear"; out-of-range values mean "absent".
            cloud_coverage: (cloud <= 10).then_some(cloud),
            track_grip_level: (grip <= 4).then_some(grip),
            wind_ms: wind.as_f32(),
            // TODO(verify-on-pc): unit of mTimeOfDay (seconds since midnight
            // like mStartET, or hours?).
            time_of_day_s: tod,
        },
    }
}

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

fn wheel(w: &TelemWheelV01) -> Wheel {
    let (susp, rh, force, bt, bp, rot, lpv, lgpv, lgv, lggv) = (
        w.suspension_deflection,
        w.ride_height,
        w.susp_force,
        w.brake_temp,
        w.brake_pressure,
        w.rotation,
        w.lateral_patch_vel,
        w.longitudinal_patch_vel,
        w.lateral_ground_vel,
        w.longitudinal_ground_vel,
    );
    let (camber, toe, lat_f, long_f, load, grip, press, temp, wear) = (
        w.camber,
        w.toe,
        w.lateral_force,
        w.longitudinal_force,
        w.tire_load,
        w.grip_fract,
        w.pressure,
        w.temperature,
        w.wear,
    );
    let (vdefl, carcass, inner, opt, radius_cm) = (
        w.vertical_tire_deflection,
        w.tire_carcass_temperature,
        w.tire_inner_layer_temperature,
        w.optimal_temp,
        w.static_undeflected_radius,
    );
    Wheel {
        susp_deflection_m: susp as f32,
        ride_height_m: rh as f32,
        susp_force_n: force as f32,
        // TODO(verify-on-pc): the header comments "Celsius" but every rF2 tool
        // (TinyPedal…) converts mBrakeTemp from Kelvin; check a cold car (~300 K
        // in Kelvin vs ~25 in Celsius).
        brake_temp_c: k_to_c(bt),
        brake_pressure: bp as f32,
        rotation_rads: rot as f32,
        camber_deg: rad_to_deg(camber),
        toe_deg: rad_to_deg(toe),
        lateral_patch_vel: lpv as f32,
        longitudinal_patch_vel: lgpv as f32,
        lateral_ground_vel: lgv as f32,
        longitudinal_ground_vel: lggv as f32,
        lateral_force_n: lat_f as f32,
        longitudinal_force_n: long_f as f32,
        load_n: load as f32,
        grip_fract: grip as f32,
        pressure_kpa: press as f32,
        temp_c: [k_to_c(temp[0]), k_to_c(temp[1]), k_to_c(temp[2])],
        temp_inner_c: [k_to_c(inner[0]), k_to_c(inner[1]), k_to_c(inner[2])],
        carcass_c: k_to_c(carcass),
        // mOptimalTemp is a float, documented by the community as °C already.
        optimal_temp_c: (opt.is_finite() && opt > 0.0).then_some(opt),
        // TODO(verify-on-pc): mWear is "0.0-1.0, fraction of maximum"; we assume
        // 1.0 = new tyre (as TinyPedal does). If it turns out to be wear
        // *consumed*, use 1 - mWear here.
        wear_remaining: wear as f32,
        surface: surface_type(w.surface_type),
        terrain: crate::layout::cstr(&w.terrain_name),
        flat: as_bool(w.flat),
        detached: as_bool(w.detached),
        compound_index: w.compound_index,
        compound_type: compound_type(w.compound_type),
        radius_m: radius_cm as f32 / 100.0,
        vertical_deflection_m: vdefl as f32,
        slip_ratio: slip_ratio(lgpv, lggv),
        slip_angle_deg: slip_angle_deg(lgv, lggv),
    }
}

/// Lap distance at telemetry rate.
///
/// The game only publishes the lap distance in the scoring block (≈5 Hz): at
/// 250 km/h it moves in 14 m steps. Between two scoring updates the distance
/// is extrapolated with the car's speed over the time elapsed since the
/// scoring snapshot (`scoring_et`), both clocks being the session clock.
pub fn extrapolated_lap_dist(
    scoring_dist: f64,
    scoring_et: f64,
    telemetry_et: f64,
    lap_start_et: f64,
    speed_ms: f64,
    track_length_m: f64,
) -> f64 {
    if !(scoring_et > 0.0 && telemetry_et > 0.0 && speed_ms.is_finite()) {
        return scoring_dist;
    }
    if lap_start_et > scoring_et && telemetry_et >= lap_start_et {
        // The line was crossed after the scoring snapshot: the snapshot still
        // shows the end of the previous lap. Count from the crossing instead.
        return speed_ms * (telemetry_et - lap_start_et).min(0.5);
    }
    let dt = (telemetry_et - scoring_et).clamp(0.0, 0.5);
    let d = scoring_dist + speed_ms * dt;
    if track_length_m > 1.0 {
        // Never run past the line before the game says the lap is over.
        d.min(track_length_m)
    } else {
        d
    }
}

fn lap_timing(
    t: &TelemInfoV01,
    s: Option<&VehicleScoringInfoV01>,
    track_length_m: f32,
    scoring_et: f64,
) -> LapTiming {
    let (lap_number, lap_start, elapsed, delta_best) =
        (t.lap_number, t.lap_start_et, t.elapsed_time, t.delta_best);
    let speed_ms = { t.local_vel }.norm();
    let (sector, in_pit_lane) = telemetry_sector(t.current_sector);
    let mut lt = LapTiming {
        lap_number,
        lap_start_s: lap_start,
        current_lap_s: if lap_start > 0.0 && elapsed >= lap_start {
            (elapsed - lap_start) as f32
        } else {
            0.0
        },
        current_sector: sector,
        in_pit_lane,
        lap_invalidated: as_bool(t.lap_invalidated),
        delta_best_s: delta_best as f32,
        ..Default::default()
    };
    if let Some(s) = s {
        let (laps, last, best, ls1, ls2, bs1, bs2, cs1, cs2) = (
            s.total_laps,
            s.last_lap_time,
            s.best_lap_time,
            s.last_sector1,
            s.last_sector2,
            s.best_sector1,
            s.best_sector2,
            s.cur_sector1,
            s.cur_sector2,
        );
        let (bls1, bls2, est, into, dist, count_flag) = (
            s.best_lap_sector1,
            s.best_lap_sector2,
            s.estimated_lap_time,
            s.time_into_lap,
            s.lap_dist,
            s.count_lap_flag,
        );
        let dist = extrapolated_lap_dist(
            dist,
            scoring_et,
            elapsed,
            lap_start,
            speed_ms,
            f64::from(track_length_m),
        );
        lt.count_lap_flag = Some(count_flag);
        lt.laps_completed = laps as i32;
        lt.last_lap_s = opt_time(last);
        lt.best_lap_s = opt_time(best);
        lt.last_s1 = opt_time(ls1);
        lt.last_s2 = opt_sector2(ls1, ls2);
        lt.last_s3 = opt_sector3(ls2, last);
        lt.best_s1 = opt_time(bs1);
        lt.best_s2 = opt_sector2(bs1, bs2);
        // The game has no "best sector 3" field (only the S3 of the best lap).
        lt.best_s3 = None;
        lt.cur_s1 = opt_time(cs1);
        lt.cur_s2 = opt_sector2(cs1, cs2);
        lt.best_lap_s1 = opt_time(bls1 as f64);
        lt.best_lap_s2 = opt_sector2(bls1 as f64, bls2 as f64);
        lt.estimated_lap_s = est as f32;
        lt.time_into_lap_s = into as f32;
        lt.lap_dist_m = dist as f32;
        lt.lap_fraction = if track_length_m > 1.0 {
            (dist as f32 / track_length_m).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }
    lt
}

fn motion(t: &TelemInfoV01) -> Motion {
    let (pos, vel, acc, rot, ori2) = (t.pos, t.local_vel, t.local_accel, t.local_rot, t.ori[2]);
    let speed_ms = vel.norm() as f32;
    Motion {
        pos: pos.as_array(),
        local_vel: vel.as_f32(),
        local_accel: acc.as_f32(),
        local_rot: rot.as_f32(),
        speed_ms,
        speed_kmh: ms_to_kmh(speed_ms),
        // rF +x = left → ISO lateral (positive right) is -x.
        g_lat: -to_g(acc.x),
        // rF +z = back → forward acceleration is -z.
        g_long: -to_g(acc.z),
        g_vert: to_g(acc.y),
        // rF +y rotation = yaw to the right, which is what the protocol wants.
        yaw_rate_dps: rad_to_deg(rot.y),
        heading_rad: heading_from_ori_row2(ori2.as_array()),
    }
}

fn engine(t: &TelemInfoV01) -> Engine {
    let (rpm, max_rpm, gear, water, oil, torque, boost, clutch) = (
        t.engine_rpm,
        t.engine_max_rpm,
        t.gear,
        t.engine_water_temp,
        t.engine_oil_temp,
        t.engine_torque,
        t.turbo_boost_pressure,
        t.clutch_rpm,
    );
    Engine {
        rpm: rpm as f32,
        max_rpm: max_rpm as f32,
        gear: gear.clamp(i8::MIN as i32, i8::MAX as i32) as i8,
        max_gears: t.max_gears,
        water_c: water as f32,
        oil_c: oil as f32,
        torque_nm: torque as f32,
        turbo_boost: boost as f32,
        clutch_rpm: clutch as f32,
        ignition_starter: t.ignition_starter,
        speed_limiter: as_bool(t.speed_limiter),
        speed_limiter_available: as_bool(t.speed_limiter_available),
        anti_stall: as_bool(t.anti_stall_activated),
        overheating: as_bool(t.overheating),
    }
}

fn inputs(t: &TelemInfoV01, ffb_torque: f32) -> Inputs {
    let (ft, fb, fs, fc, ut, ub, us, uc, torque, range) = (
        t.filtered_throttle,
        t.filtered_brake,
        t.filtered_steering,
        t.filtered_clutch,
        t.unfiltered_throttle,
        t.unfiltered_brake,
        t.unfiltered_steering,
        t.unfiltered_clutch,
        t.steering_shaft_torque,
        t.physical_steering_wheel_range,
    );
    Inputs {
        throttle: ft as f32,
        brake: fb as f32,
        steering: fs as f32,
        clutch: fc as f32,
        throttle_raw: ut as f32,
        brake_raw: ub as f32,
        steering_raw: us as f32,
        clutch_raw: uc as f32,
        steering_torque_nm: torque as f32,
        // TODO(verify-on-pc): mPhysicalSteeringWheelRange is assumed to be in
        // degrees lock-to-lock (e.g. 540 / 900), as used by TinyPedal.
        steering_angle_deg: us as f32 * range / 2.0,
        wheel_range_deg: range,
        ffb_torque,
    }
}

fn aero(t: &TelemInfoV01) -> Aero {
    let (frh, rrh, fwh, fdf, rdf, drag, f3, r3) = (
        t.front_ride_height,
        t.rear_ride_height,
        t.front_wing_height,
        t.front_downforce,
        t.rear_downforce,
        t.drag,
        t.front3rd_deflection,
        t.rear3rd_deflection,
    );
    Aero {
        front_ride_height_mm: m_to_mm(frh),
        rear_ride_height_mm: m_to_mm(rrh),
        front_wing_height_mm: m_to_mm(fwh),
        front_downforce_n: fdf as f32,
        rear_downforce_n: rdf as f32,
        drag_n: drag as f32,
        third_front_mm: m_to_mm(f3),
        third_rear_mm: m_to_mm(r3),
        front_flap_active: as_bool(t.front_flap_activated),
        rear_flap_active: as_bool(t.rear_flap_activated),
        rear_flap_legal: t.rear_flap_legal_status,
    }
}

fn energy(t: &TelemInfoV01) -> Energy {
    let (fuel, cap, ve, batt, regen, mt, mrpm, mtemp, mwater) = (
        t.fuel,
        t.fuel_capacity,
        t.virtual_energy,
        t.battery_charge_fraction,
        t.regen,
        t.electric_boost_motor_torque,
        t.electric_boost_motor_rpm,
        t.electric_boost_motor_temperature,
        t.electric_boost_water_temperature,
    );
    let state = motor_state(t.electric_boost_motor_state);
    let has_motor = state != MotorState::Unavailable;
    Energy {
        fuel_l: fuel as f32,
        fuel_capacity_l: cap as f32,
        fuel_fraction: if cap > 0.0 {
            (fuel / cap).clamp(0.0, 1.0) as f32
        } else {
            0.0
        },
        // TODO(verify-on-pc): unit of mVirtualEnergy (fraction 0..1 or %).
        // Kept raw; `None` when the car has no virtual energy (value 0).
        virtual_energy: (ve.is_finite() && ve > 0.0).then_some(ve),
        battery_soc: has_motor.then_some(batt.clamp(0.0, 1.0) as f32),
        // LMU ≥ 1.3 only; older builds leave 0.
        regen_kw: has_motor.then_some(regen),
        motor_torque_nm: mt as f32,
        motor_rpm: mrpm as f32,
        // TODO(verify-on-pc): motor temperatures assumed in °C like water/oil.
        motor_temp_c: mtemp as f32,
        motor_water_c: mwater as f32,
        motor_state: state,
        scheduled_stops: t.scheduled_stops,
    }
}

fn electronics(t: &TelemInfoV01) -> Electronics {
    let bias = t.rear_brake_bias;
    Electronics {
        tc: t.tc,
        tc_max: t.tc_max,
        tc_slip: t.tc_slip,
        tc_slip_max: t.tc_slip_max,
        tc_cut: t.tc_cut,
        tc_cut_max: t.tc_cut_max,
        abs: t.abs,
        abs_max: t.abs_max,
        motor_map: t.motor_map,
        motor_map_max: t.motor_map_max,
        brake_migration: t.migration,
        brake_migration_max: t.migration_max,
        arb_front: t.front_anti_sway,
        arb_front_max: t.front_anti_sway_max,
        arb_rear: t.rear_anti_sway,
        arb_rear_max: t.rear_anti_sway_max,
        brake_bias_rear: bias as f32,
        tc_active: as_bool(t.tc_active),
        abs_active: as_bool(t.abs_active),
        lift_and_coast: t.lift_and_coast_progress,
        wiper: t.wiper_state,
        headlights: as_bool(t.headlights),
    }
}

fn damage(t: &TelemInfoV01) -> Damage {
    let (et, mag, pos) = (t.last_impact_et, t.last_impact_magnitude, t.last_impact_pos);
    Damage {
        dents: t.dent_severity,
        detached_parts: as_bool(t.detached),
        last_impact_s: et,
        last_impact_magnitude: mag as f32,
        last_impact_pos: pos.as_f32(),
    }
}

fn gaps(t: &TelemInfoV01) -> Gaps {
    let (ca, cb, pa, pb) = (
        t.time_gap_car_ahead,
        t.time_gap_car_behind,
        t.time_gap_place_ahead,
        t.time_gap_place_behind,
    );
    Gaps {
        car_ahead_s: opt_gap(ca),
        car_behind_s: opt_gap(cb),
        place_ahead_s: opt_gap(pa),
        place_behind_s: opt_gap(pb),
    }
}

/// Build the player's telemetry from its `TelemInfoV01` and (when the scoring
/// block already knows the vehicle) its `VehicleScoringInfoV01`.
pub fn player_telemetry(
    raw: &SharedMemoryObjectOut,
    t: &TelemInfoV01,
    s: Option<&VehicleScoringInfoV01>,
    track_length_m: f32,
) -> PlayerTelemetry {
    let class_name = s.map(|s| s.vehicle_class()).unwrap_or_default();
    let mut vclass = vehicle_class_from_enum(t.vehicle_class);
    if vclass == VehicleClass::Unknown {
        vclass = vehicle_class_from_name(&class_name);
    }
    let wheels = [
        wheel(&t.wheel[0]),
        wheel(&t.wheel[1]),
        wheel(&t.wheel[2]),
        wheel(&t.wheel[3]),
    ];
    let id = t.id;
    PlayerTelemetry {
        id,
        vehicle_name: t.vehicle_name(),
        vehicle_model: t.vehicle_model(),
        vehicle_class: vclass,
        class_name,
        championship: championship_name(t.vehicle_championship),
        place: s.map(|s| s.place).unwrap_or(0),
        lap: lap_timing(t, s, track_length_m, {
            raw.scoring.scoring_info.current_et
        }),
        motion: motion(t),
        engine: engine(t),
        inputs: inputs(t, raw.generic.ffb_torque),
        aero: aero(t),
        energy: energy(t),
        electronics: electronics(t),
        wheels,
        damage: damage(t),
        gaps: gaps(t),
        tyres: TyreSet {
            front_compound: t.front_tire_compound_name(),
            rear_compound: t.rear_tire_compound_name(),
            front_compound_index: t.front_tire_compound_index,
            rear_compound_index: t.rear_tire_compound_index,
        },
        track_limits_steps: t.track_limits_steps,
        in_pits: s.map(|s| as_bool(s.in_pits)).unwrap_or(false),
        pit_state: s.map(|s| pit_state(s.pit_state)).unwrap_or_default(),
        in_garage: s.map(|s| as_bool(s.in_garage_stall)).unwrap_or(false),
        finish_status: s
            .map(|s| finish_status(s.finish_status))
            .unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Standings
// ---------------------------------------------------------------------------

fn standing(
    s: &VehicleScoringInfoV01,
    class_from_telemetry: Option<u8>,
    track_length_m: f32,
) -> Standing {
    let class_name = s.vehicle_class();
    let mut vclass = class_from_telemetry
        .map(vehicle_class_from_enum)
        .unwrap_or(VehicleClass::Unknown);
    if vclass == VehicleClass::Unknown {
        vclass = vehicle_class_from_name(&class_name);
    }
    let (id, laps, dist, lateral, tbn, lbn, tbl, lbl) = (
        s.id,
        s.total_laps,
        s.lap_dist,
        s.path_lateral,
        s.time_behind_next,
        s.laps_behind_next,
        s.time_behind_leader,
        s.laps_behind_leader,
    );
    let (best, last, bs1, bs2, ls1, ls2, cs1, cs2, into, est) = (
        s.best_lap_time,
        s.last_lap_time,
        s.best_sector1,
        s.best_sector2,
        s.last_sector1,
        s.last_sector2,
        s.cur_sector1,
        s.cur_sector2,
        s.time_into_lap,
        s.estimated_lap_time,
    );
    let (pos, vel, qual, pit_dist, pits, pens, fuel) = (
        s.pos,
        s.local_vel,
        s.qualification,
        s.pit_lap_dist,
        s.num_pitstops,
        s.num_penalties,
        s.fuel_fraction,
    );
    Standing {
        id,
        driver_name: s.driver_name(),
        vehicle_name: s.vehicle_name(),
        class_name,
        vehicle_class: vclass,
        is_player: as_bool(s.is_player),
        control: control(s.control),
        place: s.place,
        class_place: 0, // filled by `standings()`
        laps_completed: laps as i32,
        sector: scoring_sector(s.sector),
        finish_status: finish_status(s.finish_status),
        lap_dist_m: dist as f32,
        lap_fraction: if track_length_m > 1.0 {
            (dist as f32 / track_length_m).clamp(0.0, 1.0)
        } else {
            0.0
        },
        path_lateral_m: lateral as f32,
        in_pits: as_bool(s.in_pits),
        pit_state: pit_state(s.pit_state),
        in_garage: as_bool(s.in_garage_stall),
        num_pitstops: pits.max(0) as u16,
        num_penalties: pens.max(0) as u16,
        time_behind_next_s: tbn as f32,
        laps_behind_next: lbn,
        time_behind_leader_s: tbl as f32,
        laps_behind_leader: lbl,
        best_lap_s: opt_time(best),
        last_lap_s: opt_time(last),
        best_s1: opt_time(bs1),
        best_s2: opt_sector2(bs1, bs2),
        last_s1: opt_time(ls1),
        last_s2: opt_sector2(ls1, ls2),
        cur_s1: opt_time(cs1),
        cur_s2: opt_sector2(cs1, cs2),
        time_into_lap_s: into as f32,
        estimated_lap_s: est as f32,
        pos: pos.as_f32(),
        speed_kmh: ms_to_kmh(vel.norm() as f32),
        // 0x00 = 0 %, 0xFF = 100 %; 0 is also what remote cars report when
        // the value is not transmitted, hence `None`.
        fuel_fraction: (fuel > 0).then(|| fuel as f32 / 255.0),
        flag_blue: s.flag == 6,
        under_yellow: as_bool(s.under_yellow),
        headlights: as_bool(s.headlights),
        drs: as_bool(s.drs_state),
        qualification: qual,
        pit_lap_dist_m: pit_dist,
    }
}

/// All vehicles of `mNumVehicles`, sorted by place, with `class_place` derived.
pub fn standings(raw: &SharedMemoryObjectOut, track_length_m: f32) -> Vec<Standing> {
    let telem = raw.telemetry.vehicles();
    let mut out: Vec<Standing> = raw
        .scoring
        .vehicles()
        .iter()
        .map(|s| {
            let id = s.id;
            let class_enum = telem
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.vehicle_class)
                .filter(|&c| c != vehicle_class::UNKNOWN);
            standing(s, class_enum, track_length_m)
        })
        .collect();
    // Place 0 (unknown) goes last; ties broken by id for stability.
    out.sort_by_key(|s| (if s.place == 0 { u8::MAX } else { s.place }, s.id));
    let mut seen: Vec<(String, u8)> = Vec::new();
    for st in out.iter_mut() {
        let key = st.class_name.clone();
        match seen.iter_mut().find(|(k, _)| *k == key) {
            Some((_, n)) => {
                *n = n.saturating_add(1);
                st.class_place = *n;
            }
            None => {
                seen.push((key, 1));
                st.class_place = 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------

/// Convert a consistent snapshot into a [`TelemetryFrame`].
///
/// * `prev` is the previous frame from the same source. It is only used as a
///   fallback when the player has a vehicle in the telemetry block but the
///   scoring block (updated at ~5 Hz) does not know it yet: the scoring-derived
///   fields (place, lap times, pit flags) are then carried over from `prev`
///   instead of being zeroed for a few frames.
/// * `seq` is the monotonic sequence number, `source_id` goes to
///   [`TelemetryFrame::source`].
pub fn to_frame(
    raw: &SharedMemoryObjectOut,
    prev: Option<&TelemetryFrame>,
    seq: u64,
    source_id: &str,
) -> TelemetryFrame {
    let mut session = session_info(raw);
    let track_length_m = session.track_length_m;
    // The session clock at telemetry rate. The scoring block only moves every
    // 0.2 s; the player's telemetry carries the same clock at ≈100 Hz. Frozen
    // telemetry (garage, menus) falls back on the scoring clock, so the result
    // never goes backwards.
    if let Some(t) = raw.player_telemetry() {
        let telemetry_et = t.elapsed_time;
        if telemetry_et.is_finite() && telemetry_et > session.elapsed_s {
            session.elapsed_s = telemetry_et;
        }
    }

    let player = raw.player_telemetry().map(|t| {
        let s = raw.player_scoring();
        let mut p = player_telemetry(raw, t, s, track_length_m);
        if s.is_none() {
            if let Some(pp) = prev
                .and_then(|f| f.player.as_ref())
                .filter(|pp| pp.id == p.id)
            {
                p.place = pp.place;
                p.class_name = pp.class_name.clone();
                if p.vehicle_class == VehicleClass::Unknown {
                    p.vehicle_class = pp.vehicle_class;
                }
                p.in_pits = pp.in_pits;
                p.pit_state = pp.pit_state;
                p.in_garage = pp.in_garage;
                p.finish_status = pp.finish_status;
                let cur = p.lap.clone();
                p.lap = LapTiming {
                    lap_number: cur.lap_number,
                    lap_start_s: cur.lap_start_s,
                    current_lap_s: cur.current_lap_s,
                    current_sector: cur.current_sector,
                    in_pit_lane: cur.in_pit_lane,
                    lap_invalidated: cur.lap_invalidated,
                    delta_best_s: cur.delta_best_s,
                    ..pp.lap.clone()
                };
            }
        }
        p
    });

    TelemetryFrame {
        v: PROTOCOL_VERSION,
        seq,
        wall_ms: wall_ms_now(),
        source: source_id.to_string(),
        session,
        player,
        standings: standings(raw, track_length_m),
    }
}

/// Convenience: the number of vehicles announced by both blocks agree (the
/// "copy access" sanity check used by pyLMUSharedMemory).
pub fn blocks_agree(raw: &SharedMemoryObjectOut) -> bool {
    let n = raw.scoring.scoring_info.num_vehicles;
    n >= 0 && (n as usize) <= MAX_VEHICLES && n == raw.telemetry.active_vehicles as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fake::FakeWorld;

    #[test]
    fn sentinels_and_sectors() {
        assert_eq!(opt_time(-1.0), None);
        assert_eq!(opt_time(0.0), None);
        assert_eq!(opt_time(92.5), Some(92.5));
        assert_eq!(opt_sector2(30.0, 65.0), Some(35.0));
        assert_eq!(opt_sector2(-1.0, 65.0), None);
        assert_eq!(opt_sector3(65.0, 100.0), Some(35.0));
        assert_eq!(scoring_sector(0), 3);
        assert_eq!(scoring_sector(1), 1);
        assert_eq!(scoring_sector(2), 2);
        assert_eq!(telemetry_sector(0), (1, false));
        assert_eq!(telemetry_sector(2), (3, false));
        assert_eq!(telemetry_sector(0x8000_0002u32 as i32), (3, true));
        assert_eq!(session_type(0), SessionType::TestDay);
        assert_eq!(session_type(3), SessionType::Practice);
        assert_eq!(session_type(6), SessionType::Qualify);
        assert_eq!(session_type(9), SessionType::Warmup);
        assert_eq!(session_type(12), SessionType::Race);
        assert_eq!(vehicle_class_from_name("Hyper"), VehicleClass::Hypercar);
        assert_eq!(vehicle_class_from_name("LMGT3"), VehicleClass::Gt3);
        assert_eq!(vehicle_class_from_name("LMP2"), VehicleClass::Lmp2);
        assert_eq!(vehicle_class_from_enum(0x06), VehicleClass::Gt3);
    }

    #[test]
    fn frame_from_fake_world() {
        let mut world = FakeWorld::new();
        // ≈ 220 s at 100 Hz: the player (68 m/s on 13.6 km) has closed a lap.
        for _ in 0..(220 * 100) {
            world.step(0.01);
        }
        let mut raw = SharedMemoryObjectOut::boxed();
        world.write_into(&mut raw);

        let f = to_frame(&raw, None, 7, "test");
        assert_eq!(f.seq, 7);
        assert_eq!(f.source, "test");
        assert_eq!(f.session.game, GameId::Lmu);
        assert_eq!(f.session.track_name, world.track_name);
        assert_eq!(f.session.num_vehicles, 4);
        assert_eq!(f.standings.len(), 4);
        // sorted by place, class places are 1-based within class
        assert!(f.standings.windows(2).all(|w| w[0].place <= w[1].place));
        assert!(f.standings.iter().all(|s| s.class_place >= 1));
        let p = f.player.expect("player");
        assert_eq!(p.vehicle_class, VehicleClass::Hypercar);
        assert!(p.motion.speed_kmh > 0.0);
        assert!((p.motion.speed_kmh - p.motion.speed_ms * 3.6).abs() < 1e-3);
        // tyre temps converted from Kelvin
        assert!(p.wheels[0].temp_c[1] > 50.0 && p.wheels[0].temp_c[1] < 150.0);
        let (sector, in_pits) = telemetry_sector(raw.telemetry.telem_info[0].current_sector);
        assert_eq!(p.lap.current_sector, sector);
        assert_eq!(p.lap.in_pit_lane, in_pits);
        assert!(p.lap.last_lap_s.is_some(), "a lap has been completed");
        assert!(p.lap.laps_completed >= 1);
        assert_eq!(p.lap.lap_number, raw.telemetry.telem_info[0].lap_number);
        assert!(p.lap.lap_fraction >= 0.0 && p.lap.lap_fraction <= 1.0);
        assert_eq!(p.engine.gear, raw.telemetry.telem_info[0].gear as i8);
    }

    #[test]
    fn g_force_signs_follow_iso_convention() {
        let mut raw = SharedMemoryObjectOut::boxed();
        raw.telemetry.player_has_vehicle = 1;
        raw.telemetry.active_vehicles = 1;
        let t = &mut raw.telemetry.telem_info[0];
        // braking: the car decelerates → acceleration points backwards (+z in rF)
        t.local_accel = crate::layout::TelemVect3::new(-9.80665, 9.80665, 9.80665);
        t.local_vel = crate::layout::TelemVect3::new(0.0, 0.0, -50.0);
        t.local_rot = crate::layout::TelemVect3::new(0.0, 0.5, 0.0);
        let f = to_frame(&raw, None, 1, "t");
        let m = f.player.unwrap().motion;
        assert!(
            (m.g_lat - 1.0).abs() < 1e-4,
            "accel to the right (-x) → +g_lat"
        );
        assert!(
            (m.g_long + 1.0).abs() < 1e-4,
            "accel backwards (+z) → -g_long"
        );
        assert!((m.g_vert - 1.0).abs() < 1e-4);
        assert!(m.yaw_rate_dps > 0.0);
        assert!((m.speed_kmh - 180.0).abs() < 1e-3);
    }

    #[test]
    fn no_player_without_vehicle_and_prev_fallback() {
        let mut raw = SharedMemoryObjectOut::boxed();
        assert!(to_frame(&raw, None, 1, "t").player.is_none());

        // player present in telemetry but scoring empty → scoring fields from prev
        raw.telemetry.player_has_vehicle = 1;
        raw.telemetry.active_vehicles = 1;
        raw.telemetry.telem_info[0].id = 5;
        let mut prev = TelemetryFrame::default();
        let mut pp = PlayerTelemetry {
            id: 5,
            place: 3,
            ..Default::default()
        };
        pp.lap.best_lap_s = Some(100.0);
        prev.player = Some(pp);
        let f = to_frame(&raw, Some(&prev), 2, "t");
        let p = f.player.unwrap();
        assert_eq!(p.place, 3);
        assert_eq!(p.lap.best_lap_s, Some(100.0));
    }

    #[test]
    fn lap_distance_is_extrapolated_between_scoring_updates() {
        // 60 m/s, scoring snapshot 0.15 s old → 9 m further.
        let d = extrapolated_lap_dist(1000.0, 500.0, 500.15, 450.0, 60.0, 4000.0);
        assert!((d - 1009.0).abs() < 1e-6, "{d}");
        // Never beyond the line while the game still counts the same lap.
        let d = extrapolated_lap_dist(3995.0, 500.0, 500.19, 450.0, 60.0, 4000.0);
        assert_eq!(d, 4000.0);
        // Line crossed after the snapshot: distance counted from the crossing.
        let d = extrapolated_lap_dist(3995.0, 500.0, 500.10, 500.06, 60.0, 4000.0);
        assert!((d - 2.4).abs() < 1e-6, "{d}");
        // No usable clock: the scoring value as is (negative before a race start).
        assert_eq!(
            extrapolated_lap_dist(-142.0, 0.0, 0.0, 0.0, 16.0, 4000.0),
            -142.0
        );
        // A stale snapshot (paused game) is not extrapolated for ever.
        let d = extrapolated_lap_dist(1000.0, 500.0, 530.0, 450.0, 60.0, 4000.0);
        assert!((d - 1030.0).abs() < 1e-6, "{d}");
    }

    #[test]
    fn frame_clock_follows_the_telemetry_block() {
        let mut world = FakeWorld::new();
        for _ in 0..500 {
            world.step(0.01);
        }
        let mut raw = SharedMemoryObjectOut::boxed();
        world.write_into(&mut raw);
        let idx = raw.telemetry.player_vehicle_idx as usize;
        let scoring_et = { raw.scoring.scoring_info.current_et };
        raw.telemetry.telem_info[idx].elapsed_time = scoring_et + 0.13;
        let f = to_frame(&raw, None, 1, "test");
        assert!((f.session.elapsed_s - (scoring_et + 0.13)).abs() < 1e-9);
        // Frozen telemetry (garage): the scoring clock keeps the time moving.
        raw.scoring.scoring_info.current_et = scoring_et + 30.0;
        let f = to_frame(&raw, None, 2, "test");
        assert!((f.session.elapsed_s - (scoring_et + 30.0)).abs() < 1e-9);
        let p = f.player.expect("player");
        assert!(p.lap.count_lap_flag.is_some());
        assert_eq!(p.finish_status, FinishStatus::None);
    }
}
