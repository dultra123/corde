//! Where things are in the two mapped buffers of rFactor 2's shared-memory plugin.
//!
//! The plugin (`rFactor2SharedMemoryMapPlugin64.dll`, shipped with the game) copies the
//! game's own structures — the ones Le Mans Ultimate inherited — into named mappings.
//! Each mapping starts with a version block (two counters: one bumped before a write, one
//! after), then the buffer itself. Sizes and offsets were computed apart and are kept in
//! `reference/rf2-shared-memory-layout.txt`; the tests below hold this file to them.
//!
//! The per-vehicle structures have the size of [`crate::layout::TelemInfoV01`],
//! [`crate::layout::ScoringInfoV01`] and [`crate::layout::VehicleScoringInfoV01`], and
//! every field of rFactor 2 exists in Le Mans Ultimate. Compared field by field (all of
//! them, see the reference file), they sit at the same offset except six: Le Mans
//! Ultimate inserted one field (`delta_best`) before the battery and the electric motor,
//! which are therefore 8 bytes further there. Everything else Le Mans Ultimate added
//! went into bytes rFactor 2 keeps in reserve. The ranges below say what is copied
//! where, and what is left blank, when the structures are read as Le Mans Ultimate's.

use crate::layout::{SIZE_SCORING_INFO, SIZE_TELEM_INFO, SIZE_VEHICLE_SCORING_INFO};

/// Name of the telemetry mapping (session namespace).
pub const TELEMETRY_NAME: &str = "$rFactor2SMMP_Telemetry$";
/// Name of the scoring mapping.
pub const SCORING_NAME: &str = "$rFactor2SMMP_Scoring$";

/// Vehicles a buffer has room for.
pub const MAX_VEHICLES: usize = 128;

/// The counter bumped right before the plugin writes a buffer.
pub const OFF_VERSION_BEGIN: usize = 0;
/// The counter bumped once the write is done: equal to the first one when the buffer is whole.
pub const OFF_VERSION_END: usize = 4;

/// Telemetry: the number of vehicles (`i32`).
pub const OFF_TELEM_NUM: usize = 12;
/// Telemetry: the first vehicle.
pub const OFF_TELEM_VEHICLES: usize = 16;
/// Size of the telemetry mapping.
pub const TELEMETRY_SIZE: usize = OFF_TELEM_VEHICLES + MAX_VEHICLES * SIZE_TELEM_INFO;

/// Scoring: the session structure.
pub const OFF_SCORING_INFO: usize = 12;
/// Scoring: the first vehicle.
pub const OFF_SCORING_VEHICLES: usize = OFF_SCORING_INFO + SIZE_SCORING_INFO;
/// Size of the scoring mapping.
pub const SCORING_SIZE: usize = OFF_SCORING_VEHICLES + MAX_VEHICLES * SIZE_VEHICLE_SCORING_INFO;

/// Inside a vehicle's telemetry: what sits at the same place in both games.
pub const TELEM_SAME: std::ops::Range<usize> = 0..696;
/// Inside a vehicle's telemetry, in rFactor 2: battery charge, electric motor (torque,
/// rpm, two temperatures, state).
pub const TELEM_ELECTRIC: std::ops::Range<usize> = 696..737;
/// … and where Le Mans Ultimate has them (after its `delta_best`).
pub const TELEM_ELECTRIC_LMU: usize = 704;
/// Inside a vehicle's telemetry: the bytes rFactor 2 keeps in reserve.
pub const TELEM_RESERVED: std::ops::Range<usize> = 737..848;
/// Inside a vehicle's telemetry: the four wheels, at the same place in both games.
pub const TELEM_WHEELS: std::ops::Range<usize> = 848..1888;
/// Size of one wheel.
pub const SIZE_WHEEL: usize = 260;
/// Inside a wheel: the bytes of rFactor 2's reserve Le Mans Ultimate gave a meaning to
/// (ideal temperature, compound).
pub const WHEEL_LMU_ONLY: std::ops::Range<usize> = 236..242;
/// Inside the session structure: the reserve, and the pointer that follows it.
pub const SCORING_INFO_RESERVED: std::ops::Range<usize> = 340..548;
/// Inside a vehicle's scoring: the reserve.
pub const VEH_SCORING_RESERVED: std::ops::Range<usize> = 536..584;

/// Inside the session structure: the player's name (32 bytes).
pub const SCORING_INFO_PLAYER_NAME: std::ops::Range<usize> = 116..148;
/// Inside a vehicle's scoring: the driver's name (32 bytes).
pub const VEH_SCORING_DRIVER_NAME: std::ops::Range<usize> = 4..36;

/// A little-endian `u32` of a buffer, 0 when the buffer is too short.
pub fn u32_at(buf: &[u8], off: usize) -> u32 {
    buf.get(off..off + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .unwrap_or(0)
}

/// A little-endian `i32` of a buffer, 0 when the buffer is too short.
pub fn i32_at(buf: &[u8], off: usize) -> i32 {
    u32_at(buf, off) as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{ScoringInfoV01, TelemInfoV01, TelemWheelV01, VehicleScoringInfoV01};
    use std::mem::offset_of;

    #[test]
    fn sizes_are_those_of_the_reference() {
        assert_eq!(TELEMETRY_SIZE, 241_680);
        assert_eq!(SCORING_SIZE, 75_312);
        assert_eq!(OFF_SCORING_VEHICLES, 560);
        assert_eq!(SIZE_TELEM_INFO, 1888);
        assert_eq!(SIZE_SCORING_INFO, 548);
        assert_eq!(SIZE_VEHICLE_SCORING_INFO, 584);
    }

    /// The fields both games share sit where the reference says, and the reserve of
    /// rFactor 2 starts where the fields proper to Le Mans Ultimate start.
    #[test]
    fn shared_fields_sit_where_rfactor2_puts_them() {
        assert_eq!(offset_of!(TelemInfoV01, id), 0);
        assert_eq!(offset_of!(TelemInfoV01, elapsed_time), 12);
        assert_eq!(offset_of!(TelemInfoV01, vehicle_name), 32);
        assert_eq!(offset_of!(TelemInfoV01, track_name), 96);
        assert_eq!(offset_of!(TelemInfoV01, engine_rpm), 356);
        assert_eq!(offset_of!(TelemInfoV01, fuel), 524);
        assert_eq!(offset_of!(TelemInfoV01, rear_brake_bias), 664);
        // The one place where the two games differ: Le Mans Ultimate's `delta_best`.
        assert_eq!(
            offset_of!(TelemInfoV01, physical_steering_wheel_range) + 4,
            TELEM_SAME.end
        );
        assert_eq!(offset_of!(TelemInfoV01, delta_best), TELEM_ELECTRIC.start);
        assert_eq!(
            offset_of!(TelemInfoV01, battery_charge_fraction),
            TELEM_ELECTRIC_LMU
        );
        assert_eq!(
            offset_of!(TelemInfoV01, electric_boost_motor_state),
            TELEM_ELECTRIC_LMU + TELEM_ELECTRIC.len() - 1
        );
        assert_eq!(
            offset_of!(TelemInfoV01, lap_invalidated),
            TELEM_ELECTRIC_LMU + TELEM_ELECTRIC.len()
        );
        assert_eq!(offset_of!(TelemInfoV01, wheel), TELEM_WHEELS.start);
        assert_eq!(TELEM_WHEELS.end, SIZE_TELEM_INFO);
        assert_eq!(TELEM_WHEELS.len(), 4 * SIZE_WHEEL);
        assert_eq!(TELEM_RESERVED.end, TELEM_WHEELS.start);
        assert_eq!(
            offset_of!(TelemWheelV01, optimal_temp),
            WHEEL_LMU_ONLY.start
        );
        assert_eq!(offset_of!(TelemWheelV01, expansion), WHEEL_LMU_ONLY.end);

        assert_eq!(offset_of!(ScoringInfoV01, track_name), 0);
        assert_eq!(offset_of!(ScoringInfoV01, session), 64);
        assert_eq!(offset_of!(ScoringInfoV01, current_et), 68);
        assert_eq!(offset_of!(ScoringInfoV01, end_et), 76);
        assert_eq!(offset_of!(ScoringInfoV01, lap_dist), 88);
        assert_eq!(offset_of!(ScoringInfoV01, num_vehicles), 104);
        assert_eq!(offset_of!(ScoringInfoV01, game_phase), 108);
        assert_eq!(offset_of!(ScoringInfoV01, in_realtime), 115);
        assert_eq!(
            offset_of!(ScoringInfoV01, player_name),
            SCORING_INFO_PLAYER_NAME.start
        );
        assert_eq!(offset_of!(ScoringInfoV01, dark_cloud), 212);
        assert_eq!(offset_of!(ScoringInfoV01, start_et), 328);
        assert_eq!(offset_of!(ScoringInfoV01, avg_path_wetness), 332);
        assert_eq!(
            offset_of!(ScoringInfoV01, session_time_remaining),
            SCORING_INFO_RESERVED.start
        );

        assert_eq!(offset_of!(VehicleScoringInfoV01, id), 0);
        assert_eq!(
            offset_of!(VehicleScoringInfoV01, driver_name),
            VEH_SCORING_DRIVER_NAME.start
        );
        assert_eq!(offset_of!(VehicleScoringInfoV01, vehicle_name), 36);
        assert_eq!(offset_of!(VehicleScoringInfoV01, total_laps), 100);
        assert_eq!(offset_of!(VehicleScoringInfoV01, is_player), 196);
        assert_eq!(offset_of!(VehicleScoringInfoV01, control), 197);
        assert_eq!(offset_of!(VehicleScoringInfoV01, place), 199);
        assert_eq!(offset_of!(VehicleScoringInfoV01, vehicle_class), 200);
        assert_eq!(offset_of!(VehicleScoringInfoV01, time_behind_next), 232);
        assert_eq!(offset_of!(VehicleScoringInfoV01, count_lap_flag), 506);
        assert_eq!(offset_of!(VehicleScoringInfoV01, best_lap_sector2), 532);
        assert_eq!(
            offset_of!(VehicleScoringInfoV01, steam_id),
            VEH_SCORING_RESERVED.start
        );
    }
}
