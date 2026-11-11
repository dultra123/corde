//! The fixed column schema of the Parquet sample files.
//!
//! [`CHANNELS`] is **the** definition: the order of the entries is the order
//! of the Parquet columns, each entry carries its unit, its physical type and
//! the accessor that pulls the value out of a [`TelemetryFrame`]. Readers
//! (`query::lap_samples`, the CLI, external tools) address columns by name.
//!
//! Rules:
//! * Human units, like the protocol (°C, km/h, kPa, degrees, metres, litres).
//! * Per-wheel channels are suffixed `_fl`, `_fr`, `_rl`, `_rr`.
//! * Adding a channel is backwards compatible (older files simply lack the
//!   column); renaming or retyping one is not — bump [`SCHEMA_VERSION`].

use corde_protocol::{PlayerTelemetry, TelemetryFrame, Wheel};

/// Version of the column set; stored in the Parquet key-value metadata as
/// `corde:channels`. Bump on any rename / retype.
pub const SCHEMA_VERSION: u32 = 1;

/// Physical type of a channel in the Parquet file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelKind {
    /// 64-bit float (session time, positions).
    F64,
    /// 32-bit float, never null.
    F32,
    /// 32-bit float, null when the game does not provide the value.
    OptF32,
    I64,
    I32,
    I8,
    U8,
    Bool,
}

/// One value of a channel for one sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    F64(f64),
    F32(f32),
    OptF32(Option<f32>),
    I64(i64),
    I32(i32),
    I8(i8),
    U8(u8),
    Bool(bool),
}

impl Value {
    /// Lossy conversion used by the JSON API and the CSV export (`None` for a
    /// missing optional value).
    pub fn as_f64(self) -> Option<f64> {
        match self {
            Value::F64(v) => Some(v),
            Value::F32(v) => Some(v as f64),
            Value::OptF32(v) => v.map(|v| v as f64),
            Value::I64(v) => Some(v as f64),
            Value::I32(v) => Some(v as f64),
            Value::I8(v) => Some(v as f64),
            Value::U8(v) => Some(v as f64),
            Value::Bool(v) => Some(if v { 1.0 } else { 0.0 }),
        }
    }
}

/// What a channel accessor sees: the frame plus its (present) player block.
pub struct Ctx<'a> {
    pub frame: &'a TelemetryFrame,
    pub player: &'a PlayerTelemetry,
}

/// Accessor of one channel.
pub type Getter = fn(&Ctx<'_>) -> Value;

/// One column of the sample files.
#[derive(Clone, Copy)]
pub struct Channel {
    pub name: &'static str,
    /// Unit as shown to the user (`"km/h"`, `"°C"`, `""` for ratios / flags).
    pub unit: &'static str,
    pub kind: ChannelKind,
    /// Pulls the value out of a frame.
    pub get: Getter,
}

impl std::fmt::Debug for Channel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Channel")
            .field("name", &self.name)
            .field("unit", &self.unit)
            .field("kind", &self.kind)
            .finish()
    }
}

impl Channel {
    /// `true` when the column may hold nulls.
    pub fn nullable(&self) -> bool {
        matches!(self.kind, ChannelKind::OptF32)
    }
}

/// Suffixes of the per-wheel channels, in protocol order (FL, FR, RL, RR).
pub const WHEEL_SUFFIXES: [&str; 4] = ["_fl", "_fr", "_rl", "_rr"];

/// Builds [`CHANNELS`]: a block of scalar channels (the accessor sees `$c: &Ctx`)
/// followed by a block of per-wheel channels (the accessor sees `$w: &Wheel`),
/// each expanded four times with the `_fl _fr _rl _rr` suffixes.
macro_rules! channels {
    (
        ctx = $c:ident, wheel = $w:ident;
        scalars { $( $name:ident : $unit:literal $kind:ident => $get:expr ),* $(,)? }
        wheels  { $( $wname:ident : $wunit:literal $wkind:ident => $wget:expr ),* $(,)? }
    ) => {
        pub const CHANNELS: &[Channel] = &[
            $(
                Channel {
                    name: stringify!($name),
                    unit: $unit,
                    kind: ChannelKind::$kind,
                    get: |$c: &Ctx<'_>| Value::$kind($get),
                },
            )*
            $(
                Channel {
                    name: concat!(stringify!($wname), "_fl"),
                    unit: $wunit,
                    kind: ChannelKind::$wkind,
                    get: |$c: &Ctx<'_>| { let $w: &Wheel = &$c.player.wheels[0]; Value::$wkind($wget) },
                },
                Channel {
                    name: concat!(stringify!($wname), "_fr"),
                    unit: $wunit,
                    kind: ChannelKind::$wkind,
                    get: |$c: &Ctx<'_>| { let $w: &Wheel = &$c.player.wheels[1]; Value::$wkind($wget) },
                },
                Channel {
                    name: concat!(stringify!($wname), "_rl"),
                    unit: $wunit,
                    kind: ChannelKind::$wkind,
                    get: |$c: &Ctx<'_>| { let $w: &Wheel = &$c.player.wheels[2]; Value::$wkind($wget) },
                },
                Channel {
                    name: concat!(stringify!($wname), "_rr"),
                    unit: $wunit,
                    kind: ChannelKind::$wkind,
                    get: |$c: &Ctx<'_>| { let $w: &Wheel = &$c.player.wheels[3]; Value::$wkind($wget) },
                },
            )*
        ];
    };
}

channels! {
    ctx = c, wheel = w;
    scalars {
        // --- time & position on the lap -------------------------------------
        t_s:                 "s"     F64 => c.frame.session.elapsed_s,
        wall_ms:             "ms"    I64 => c.frame.wall_ms as i64,
        lap_number:          ""      I32 => c.player.lap.lap_number,
        lap_dist_m:          "m"     F32 => c.player.lap.lap_dist_m,
        lap_fraction:        ""      F32 => c.player.lap.lap_fraction,
        // --- speed, engine, driver inputs -----------------------------------
        speed_kmh:           "km/h"  F32 => c.player.motion.speed_kmh,
        rpm:                 "rpm"   F32 => c.player.engine.rpm,
        gear:                ""      I8  => c.player.engine.gear,
        throttle:            ""      F32 => c.player.inputs.throttle,
        brake:               ""      F32 => c.player.inputs.brake,
        steering:            ""      F32 => c.player.inputs.steering,
        clutch:              ""      F32 => c.player.inputs.clutch,
        throttle_raw:        ""      F32 => c.player.inputs.throttle_raw,
        brake_raw:           ""      F32 => c.player.inputs.brake_raw,
        steering_raw:        ""      F32 => c.player.inputs.steering_raw,
        steering_angle_deg:  "°"     F32 => c.player.inputs.steering_angle_deg,
        // --- motion ---------------------------------------------------------
        g_lat:               "g"     F32 => c.player.motion.g_lat,
        g_long:              "g"     F32 => c.player.motion.g_long,
        g_vert:              "g"     F32 => c.player.motion.g_vert,
        yaw_rate_dps:        "°/s"   F32 => c.player.motion.yaw_rate_dps,
        pos_x:               "m"     F64 => c.player.motion.pos[0],
        pos_y:               "m"     F64 => c.player.motion.pos[1],
        pos_z:               "m"     F64 => c.player.motion.pos[2],
        heading_rad:         "rad"   F32 => c.player.motion.heading_rad,
        // --- energy ---------------------------------------------------------
        fuel_l:              "L"     F32 => c.player.energy.fuel_l,
        virtual_energy:      ""      OptF32 => c.player.energy.virtual_energy,
        battery_soc:         ""      OptF32 => c.player.energy.battery_soc,
        regen_kw:            "kW"    OptF32 => c.player.energy.regen_kw,
        // --- electronics ----------------------------------------------------
        brake_bias_rear:     ""      F32 => c.player.electronics.brake_bias_rear,
        tc:                  ""      U8  => c.player.electronics.tc,
        abs:                 ""      U8  => c.player.electronics.abs,
        motor_map:           ""      U8  => c.player.electronics.motor_map,
        tc_active:           ""      Bool => c.player.electronics.tc_active,
        abs_active:          ""      Bool => c.player.electronics.abs_active,
        // --- temperatures, aero ---------------------------------------------
        water_c:             "°C"    F32 => c.player.engine.water_c,
        oil_c:               "°C"    F32 => c.player.engine.oil_c,
        front_ride_height_mm: "mm"   F32 => c.player.aero.front_ride_height_mm,
        rear_ride_height_mm: "mm"    F32 => c.player.aero.rear_ride_height_mm,
        front_downforce_n:   "N"     F32 => c.player.aero.front_downforce_n,
        rear_downforce_n:    "N"     F32 => c.player.aero.rear_downforce_n,
        drag_n:              "N"     F32 => c.player.aero.drag_n,
        third_front_mm:      "mm"    F32 => c.player.aero.third_front_mm,
        third_rear_mm:       "mm"    F32 => c.player.aero.third_rear_mm,
        // --- track state ----------------------------------------------------
        in_pits:             ""      Bool => c.player.in_pits,
        in_pit_lane:         ""      Bool => c.player.lap.in_pit_lane,
        current_sector:      ""      U8  => c.player.lap.current_sector,
        lap_invalidated:     ""      Bool => c.player.lap.lap_invalidated,
        track_c:             "°C"    F32 => c.frame.session.weather.track_c,
        ambient_c:           "°C"    F32 => c.frame.session.weather.ambient_c,
        rain:                ""      F32 => c.frame.session.weather.rain,
        wetness_avg:         ""      F32 => c.frame.session.weather.wetness_avg,
        // --- the game's lap bookkeeping (lets a session be re-timed offline) -
        laps_completed:      ""      I32 => c.player.lap.laps_completed,
        lap_start_s:         "s"     F64 => c.player.lap.lap_start_s,
        last_lap_s:          "s"     OptF32 => c.player.lap.last_lap_s,
        count_lap_flag:      ""      U8  => c.player.lap.count_lap_flag.unwrap_or(u8::MAX),
    }
    wheels {
        susp_deflection_m:   "m"     F32 => w.susp_deflection_m,
        ride_height_m:       "m"     F32 => w.ride_height_m,
        susp_force_n:        "N"     F32 => w.susp_force_n,
        brake_temp_c:        "°C"    F32 => w.brake_temp_c,
        brake_pressure:      ""      F32 => w.brake_pressure,
        rotation_rads:       "rad/s" F32 => w.rotation_rads,
        camber_deg:          "°"     F32 => w.camber_deg,
        toe_deg:             "°"     F32 => w.toe_deg,
        lateral_patch_vel:   "m/s"   F32 => w.lateral_patch_vel,
        longitudinal_patch_vel: "m/s" F32 => w.longitudinal_patch_vel,
        lateral_ground_vel:  "m/s"   F32 => w.lateral_ground_vel,
        longitudinal_ground_vel: "m/s" F32 => w.longitudinal_ground_vel,
        lateral_force_n:     "N"     F32 => w.lateral_force_n,
        longitudinal_force_n: "N"    F32 => w.longitudinal_force_n,
        load_n:              "N"     F32 => w.load_n,
        grip_fract:          ""      F32 => w.grip_fract,
        pressure_kpa:        "kPa"   F32 => w.pressure_kpa,
        temp_l_c:            "°C"    F32 => w.temp_c[0],
        temp_c_c:            "°C"    F32 => w.temp_c[1],
        temp_r_c:            "°C"    F32 => w.temp_c[2],
        temp_inner_l_c:      "°C"    F32 => w.temp_inner_c[0],
        temp_inner_c_c:      "°C"    F32 => w.temp_inner_c[1],
        temp_inner_r_c:      "°C"    F32 => w.temp_inner_c[2],
        carcass_c:           "°C"    F32 => w.carcass_c,
        // The compound's optimal temperature as the game reports it (null when
        // it does not); absent from files written before it was added.
        optimal_temp_c:      "°C"    OptF32 => w.optimal_temp_c,
        wear_remaining:      ""      F32 => w.wear_remaining,
        slip_ratio:          ""      F32 => w.slip_ratio,
        slip_angle_deg:      "°"     F32 => w.slip_angle_deg,
        surface:             ""      U8  => w.surface as u8,
    }
}

/// Index of a channel by name.
pub fn channel_index(name: &str) -> Option<usize> {
    CHANNELS.iter().position(|c| c.name == name)
}

/// Channel by name.
pub fn channel(name: &str) -> Option<&'static Channel> {
    CHANNELS.iter().find(|c| c.name == name)
}

/// Names of every channel, in column order.
pub fn channel_names() -> Vec<&'static str> {
    CHANNELS.iter().map(|c| c.name).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique_and_well_formed() {
        let mut seen = HashSet::new();
        for ch in CHANNELS {
            assert!(seen.insert(ch.name), "duplicate channel {}", ch.name);
            assert!(
                ch.name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
                "channel name {} must be snake_case",
                ch.name
            );
        }
        assert_eq!(CHANNELS[0].name, "t_s");
        assert_eq!(CHANNELS[1].name, "wall_ms");
        assert_eq!(CHANNELS[2].name, "lap_number");
        // 55 scalars + 29 per-wheel × 4.
        assert_eq!(CHANNELS.len(), 55 + 29 * 4);
        for suffix in WHEEL_SUFFIXES {
            assert!(channel(&format!("brake_temp_c{suffix}")).is_some());
            let opt = channel(&format!("optimal_temp_c{suffix}")).unwrap();
            assert!(opt.nullable());
        }
    }

    #[test]
    fn getters_read_the_right_fields() {
        let mut frame = TelemetryFrame {
            wall_ms: 1_700_000_000_123,
            ..Default::default()
        };
        frame.session.elapsed_s = 42.5;
        let mut p = PlayerTelemetry::default();
        p.motion.speed_kmh = 231.0;
        p.engine.gear = -1;
        p.electronics.tc = 7;
        p.energy.virtual_energy = None;
        p.wheels[3].brake_temp_c = 612.0;
        p.wheels[1].temp_c = [80.0, 85.0, 90.0];
        p.wheels[0].optimal_temp_c = Some(88.0);
        p.wheels[2].surface = corde_protocol::SurfaceType::Grass;
        let ctx = Ctx {
            frame: &frame,
            player: &p,
        };
        let get = |n: &str| (channel(n).unwrap().get)(&ctx);
        assert_eq!(get("t_s"), Value::F64(42.5));
        assert_eq!(get("wall_ms"), Value::I64(1_700_000_000_123));
        assert_eq!(get("speed_kmh"), Value::F32(231.0));
        assert_eq!(get("gear"), Value::I8(-1));
        assert_eq!(get("tc"), Value::U8(7));
        assert_eq!(get("virtual_energy"), Value::OptF32(None));
        assert_eq!(get("brake_temp_c_rr"), Value::F32(612.0));
        assert_eq!(get("brake_temp_c_fl"), Value::F32(0.0));
        assert_eq!(get("temp_r_c_fr"), Value::F32(90.0));
        assert_eq!(get("optimal_temp_c_fl"), Value::OptF32(Some(88.0)));
        assert_eq!(get("optimal_temp_c_rr"), Value::OptF32(None));
        assert_eq!(
            get("surface_rl"),
            Value::U8(corde_protocol::SurfaceType::Grass as u8)
        );
        assert_eq!(Value::Bool(true).as_f64(), Some(1.0));
        assert_eq!(Value::OptF32(None).as_f64(), None);
    }
}
