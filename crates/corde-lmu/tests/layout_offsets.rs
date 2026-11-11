//! Exhaustive size / offset tests of `corde_lmu::layout`, GENERATED from
//! `reference/lmu-shared-memory-layout.txt` by
//! `crates/corde-lmu/scripts/gen_layout_tests.py` — do not edit by hand, regenerate.
//!
//! Every direct member of every structure of the reference dump is checked;
//! nested members (e.g. `mPos.x`) are covered by the tests of their own type.

use corde_lmu::layout::*;
use std::mem::{align_of, offset_of, size_of};

#[test]
fn telem_vect3() {
    assert_eq!(size_of::<TelemVect3>(), 24, "sizeof(TelemVect3)");
    assert_eq!(align_of::<TelemVect3>(), 4, "alignof(TelemVect3)");
    assert_eq!(offset_of!(TelemVect3, x), 0, "TelemVect3::x");
    assert_eq!(offset_of!(TelemVect3, y), 8, "TelemVect3::y");
    assert_eq!(offset_of!(TelemVect3, z), 16, "TelemVect3::z");
}

#[test]
fn telem_wheel_v01() {
    assert_eq!(size_of::<TelemWheelV01>(), 260, "sizeof(TelemWheelV01)");
    assert_eq!(align_of::<TelemWheelV01>(), 4, "alignof(TelemWheelV01)");
    assert_eq!(
        offset_of!(TelemWheelV01, suspension_deflection),
        0,
        "TelemWheelV01::mSuspensionDeflection"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, ride_height),
        8,
        "TelemWheelV01::mRideHeight"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, susp_force),
        16,
        "TelemWheelV01::mSuspForce"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, brake_temp),
        24,
        "TelemWheelV01::mBrakeTemp"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, brake_pressure),
        32,
        "TelemWheelV01::mBrakePressure"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, rotation),
        40,
        "TelemWheelV01::mRotation"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, lateral_patch_vel),
        48,
        "TelemWheelV01::mLateralPatchVel"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, longitudinal_patch_vel),
        56,
        "TelemWheelV01::mLongitudinalPatchVel"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, lateral_ground_vel),
        64,
        "TelemWheelV01::mLateralGroundVel"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, longitudinal_ground_vel),
        72,
        "TelemWheelV01::mLongitudinalGroundVel"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, camber),
        80,
        "TelemWheelV01::mCamber"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, lateral_force),
        88,
        "TelemWheelV01::mLateralForce"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, longitudinal_force),
        96,
        "TelemWheelV01::mLongitudinalForce"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, tire_load),
        104,
        "TelemWheelV01::mTireLoad"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, grip_fract),
        112,
        "TelemWheelV01::mGripFract"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, pressure),
        120,
        "TelemWheelV01::mPressure"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, temperature),
        128,
        "TelemWheelV01::mTemperature"
    );
    assert_eq!(offset_of!(TelemWheelV01, wear), 152, "TelemWheelV01::mWear");
    assert_eq!(
        offset_of!(TelemWheelV01, terrain_name),
        160,
        "TelemWheelV01::mTerrainName"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, surface_type),
        176,
        "TelemWheelV01::mSurfaceType"
    );
    assert_eq!(offset_of!(TelemWheelV01, flat), 177, "TelemWheelV01::mFlat");
    assert_eq!(
        offset_of!(TelemWheelV01, detached),
        178,
        "TelemWheelV01::mDetached"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, static_undeflected_radius),
        179,
        "TelemWheelV01::mStaticUndeflectedRadius"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, vertical_tire_deflection),
        180,
        "TelemWheelV01::mVerticalTireDeflection"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, wheel_y_location),
        188,
        "TelemWheelV01::mWheelYLocation"
    );
    assert_eq!(offset_of!(TelemWheelV01, toe), 196, "TelemWheelV01::mToe");
    assert_eq!(
        offset_of!(TelemWheelV01, tire_carcass_temperature),
        204,
        "TelemWheelV01::mTireCarcassTemperature"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, tire_inner_layer_temperature),
        212,
        "TelemWheelV01::mTireInnerLayerTemperature"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, optimal_temp),
        236,
        "TelemWheelV01::mOptimalTemp"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, compound_index),
        240,
        "TelemWheelV01::mCompoundIndex"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, compound_type),
        241,
        "TelemWheelV01::mCompoundType"
    );
    assert_eq!(
        offset_of!(TelemWheelV01, expansion),
        242,
        "TelemWheelV01::mExpansion"
    );
}

#[test]
fn telem_info_v01() {
    assert_eq!(size_of::<TelemInfoV01>(), 1888, "sizeof(TelemInfoV01)");
    assert_eq!(align_of::<TelemInfoV01>(), 4, "alignof(TelemInfoV01)");
    assert_eq!(offset_of!(TelemInfoV01, id), 0, "TelemInfoV01::mID");
    assert_eq!(
        offset_of!(TelemInfoV01, delta_time),
        4,
        "TelemInfoV01::mDeltaTime"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, elapsed_time),
        12,
        "TelemInfoV01::mElapsedTime"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, lap_number),
        20,
        "TelemInfoV01::mLapNumber"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, lap_start_et),
        24,
        "TelemInfoV01::mLapStartET"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, vehicle_name),
        32,
        "TelemInfoV01::mVehicleName"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, track_name),
        96,
        "TelemInfoV01::mTrackName"
    );
    assert_eq!(offset_of!(TelemInfoV01, pos), 160, "TelemInfoV01::mPos");
    assert_eq!(
        offset_of!(TelemInfoV01, local_vel),
        184,
        "TelemInfoV01::mLocalVel"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, local_accel),
        208,
        "TelemInfoV01::mLocalAccel"
    );
    assert_eq!(offset_of!(TelemInfoV01, ori), 232, "TelemInfoV01::mOri");
    assert_eq!(
        offset_of!(TelemInfoV01, local_rot),
        304,
        "TelemInfoV01::mLocalRot"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, local_rot_accel),
        328,
        "TelemInfoV01::mLocalRotAccel"
    );
    assert_eq!(offset_of!(TelemInfoV01, gear), 352, "TelemInfoV01::mGear");
    assert_eq!(
        offset_of!(TelemInfoV01, engine_rpm),
        356,
        "TelemInfoV01::mEngineRPM"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, engine_water_temp),
        364,
        "TelemInfoV01::mEngineWaterTemp"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, engine_oil_temp),
        372,
        "TelemInfoV01::mEngineOilTemp"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, clutch_rpm),
        380,
        "TelemInfoV01::mClutchRPM"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, unfiltered_throttle),
        388,
        "TelemInfoV01::mUnfilteredThrottle"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, unfiltered_brake),
        396,
        "TelemInfoV01::mUnfilteredBrake"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, unfiltered_steering),
        404,
        "TelemInfoV01::mUnfilteredSteering"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, unfiltered_clutch),
        412,
        "TelemInfoV01::mUnfilteredClutch"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, filtered_throttle),
        420,
        "TelemInfoV01::mFilteredThrottle"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, filtered_brake),
        428,
        "TelemInfoV01::mFilteredBrake"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, filtered_steering),
        436,
        "TelemInfoV01::mFilteredSteering"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, filtered_clutch),
        444,
        "TelemInfoV01::mFilteredClutch"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, steering_shaft_torque),
        452,
        "TelemInfoV01::mSteeringShaftTorque"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front3rd_deflection),
        460,
        "TelemInfoV01::mFront3rdDeflection"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear3rd_deflection),
        468,
        "TelemInfoV01::mRear3rdDeflection"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_wing_height),
        476,
        "TelemInfoV01::mFrontWingHeight"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_ride_height),
        484,
        "TelemInfoV01::mFrontRideHeight"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_ride_height),
        492,
        "TelemInfoV01::mRearRideHeight"
    );
    assert_eq!(offset_of!(TelemInfoV01, drag), 500, "TelemInfoV01::mDrag");
    assert_eq!(
        offset_of!(TelemInfoV01, front_downforce),
        508,
        "TelemInfoV01::mFrontDownforce"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_downforce),
        516,
        "TelemInfoV01::mRearDownforce"
    );
    assert_eq!(offset_of!(TelemInfoV01, fuel), 524, "TelemInfoV01::mFuel");
    assert_eq!(
        offset_of!(TelemInfoV01, engine_max_rpm),
        532,
        "TelemInfoV01::mEngineMaxRPM"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, scheduled_stops),
        540,
        "TelemInfoV01::mScheduledStops"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, overheating),
        541,
        "TelemInfoV01::mOverheating"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, detached),
        542,
        "TelemInfoV01::mDetached"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, headlights),
        543,
        "TelemInfoV01::mHeadlights"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, dent_severity),
        544,
        "TelemInfoV01::mDentSeverity"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, last_impact_et),
        552,
        "TelemInfoV01::mLastImpactET"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, last_impact_magnitude),
        560,
        "TelemInfoV01::mLastImpactMagnitude"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, last_impact_pos),
        568,
        "TelemInfoV01::mLastImpactPos"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, engine_torque),
        592,
        "TelemInfoV01::mEngineTorque"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, current_sector),
        600,
        "TelemInfoV01::mCurrentSector"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, speed_limiter),
        604,
        "TelemInfoV01::mSpeedLimiter"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, max_gears),
        605,
        "TelemInfoV01::mMaxGears"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_tire_compound_index),
        606,
        "TelemInfoV01::mFrontTireCompoundIndex"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_tire_compound_index),
        607,
        "TelemInfoV01::mRearTireCompoundIndex"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, fuel_capacity),
        608,
        "TelemInfoV01::mFuelCapacity"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_flap_activated),
        616,
        "TelemInfoV01::mFrontFlapActivated"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_flap_activated),
        617,
        "TelemInfoV01::mRearFlapActivated"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_flap_legal_status),
        618,
        "TelemInfoV01::mRearFlapLegalStatus"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, ignition_starter),
        619,
        "TelemInfoV01::mIgnitionStarter"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_tire_compound_name),
        620,
        "TelemInfoV01::mFrontTireCompoundName"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_tire_compound_name),
        638,
        "TelemInfoV01::mRearTireCompoundName"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, speed_limiter_available),
        656,
        "TelemInfoV01::mSpeedLimiterAvailable"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, anti_stall_activated),
        657,
        "TelemInfoV01::mAntiStallActivated"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, unused),
        658,
        "TelemInfoV01::mUnused"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, visual_steering_wheel_range),
        660,
        "TelemInfoV01::mVisualSteeringWheelRange"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_brake_bias),
        664,
        "TelemInfoV01::mRearBrakeBias"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, turbo_boost_pressure),
        672,
        "TelemInfoV01::mTurboBoostPressure"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, physics_to_graphics_offset),
        680,
        "TelemInfoV01::mPhysicsToGraphicsOffset"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, physical_steering_wheel_range),
        692,
        "TelemInfoV01::mPhysicalSteeringWheelRange"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, delta_best),
        696,
        "TelemInfoV01::mDeltaBest"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, battery_charge_fraction),
        704,
        "TelemInfoV01::mBatteryChargeFraction"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, electric_boost_motor_torque),
        712,
        "TelemInfoV01::mElectricBoostMotorTorque"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, electric_boost_motor_rpm),
        720,
        "TelemInfoV01::mElectricBoostMotorRPM"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, electric_boost_motor_temperature),
        728,
        "TelemInfoV01::mElectricBoostMotorTemperature"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, electric_boost_water_temperature),
        736,
        "TelemInfoV01::mElectricBoostWaterTemperature"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, electric_boost_motor_state),
        744,
        "TelemInfoV01::mElectricBoostMotorState"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, lap_invalidated),
        745,
        "TelemInfoV01::mLapInvalidated"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, abs_active),
        746,
        "TelemInfoV01::mABSActive"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, tc_active),
        747,
        "TelemInfoV01::mTCActive"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, speed_limiter_active),
        748,
        "TelemInfoV01::mSpeedLimiterActive"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, wiper_state),
        749,
        "TelemInfoV01::mWiperState"
    );
    assert_eq!(offset_of!(TelemInfoV01, tc), 750, "TelemInfoV01::mTC");
    assert_eq!(
        offset_of!(TelemInfoV01, tc_max),
        751,
        "TelemInfoV01::mTCMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, tc_slip),
        752,
        "TelemInfoV01::mTCSlip"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, tc_slip_max),
        753,
        "TelemInfoV01::mTCSlipMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, tc_cut),
        754,
        "TelemInfoV01::mTCCut"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, tc_cut_max),
        755,
        "TelemInfoV01::mTCCutMax"
    );
    assert_eq!(offset_of!(TelemInfoV01, abs), 756, "TelemInfoV01::mABS");
    assert_eq!(
        offset_of!(TelemInfoV01, abs_max),
        757,
        "TelemInfoV01::mABSMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, motor_map),
        758,
        "TelemInfoV01::mMotorMap"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, motor_map_max),
        759,
        "TelemInfoV01::mMotorMapMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, migration),
        760,
        "TelemInfoV01::mMigration"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, migration_max),
        761,
        "TelemInfoV01::mMigrationMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_anti_sway),
        762,
        "TelemInfoV01::mFrontAntiSway"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, front_anti_sway_max),
        763,
        "TelemInfoV01::mFrontAntiSwayMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_anti_sway),
        764,
        "TelemInfoV01::mRearAntiSway"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, rear_anti_sway_max),
        765,
        "TelemInfoV01::mRearAntiSwayMax"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, lift_and_coast_progress),
        766,
        "TelemInfoV01::mLiftAndCoastProgress"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, track_limits_steps),
        767,
        "TelemInfoV01::mTrackLimitsSteps"
    );
    assert_eq!(offset_of!(TelemInfoV01, regen), 768, "TelemInfoV01::mRegen");
    assert_eq!(offset_of!(TelemInfoV01, soc), 772, "TelemInfoV01::mSoC");
    assert_eq!(
        offset_of!(TelemInfoV01, virtual_energy),
        776,
        "TelemInfoV01::mVirtualEnergy"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, time_gap_car_ahead),
        780,
        "TelemInfoV01::mTimeGapCarAhead"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, time_gap_car_behind),
        784,
        "TelemInfoV01::mTimeGapCarBehind"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, time_gap_place_ahead),
        788,
        "TelemInfoV01::mTimeGapPlaceAhead"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, time_gap_place_behind),
        792,
        "TelemInfoV01::mTimeGapPlaceBehind"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, vehicle_model),
        796,
        "TelemInfoV01::mVehicleModel"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, vehicle_class),
        826,
        "TelemInfoV01::mVehicleClass"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, vehicle_championship),
        827,
        "TelemInfoV01::mVehicleChampionship"
    );
    assert_eq!(
        offset_of!(TelemInfoV01, expansion),
        828,
        "TelemInfoV01::mExpansion"
    );
    assert_eq!(offset_of!(TelemInfoV01, wheel), 848, "TelemInfoV01::mWheel");
}

#[test]
fn vehicle_scoring_info_v01() {
    assert_eq!(
        size_of::<VehicleScoringInfoV01>(),
        584,
        "sizeof(VehicleScoringInfoV01)"
    );
    assert_eq!(
        align_of::<VehicleScoringInfoV01>(),
        4,
        "alignof(VehicleScoringInfoV01)"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, id),
        0,
        "VehicleScoringInfoV01::mID"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, driver_name),
        4,
        "VehicleScoringInfoV01::mDriverName"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, vehicle_name),
        36,
        "VehicleScoringInfoV01::mVehicleName"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, total_laps),
        100,
        "VehicleScoringInfoV01::mTotalLaps"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, sector),
        102,
        "VehicleScoringInfoV01::mSector"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, finish_status),
        103,
        "VehicleScoringInfoV01::mFinishStatus"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, lap_dist),
        104,
        "VehicleScoringInfoV01::mLapDist"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, path_lateral),
        112,
        "VehicleScoringInfoV01::mPathLateral"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, track_edge),
        120,
        "VehicleScoringInfoV01::mTrackEdge"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, best_sector1),
        128,
        "VehicleScoringInfoV01::mBestSector1"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, best_sector2),
        136,
        "VehicleScoringInfoV01::mBestSector2"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, best_lap_time),
        144,
        "VehicleScoringInfoV01::mBestLapTime"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, last_sector1),
        152,
        "VehicleScoringInfoV01::mLastSector1"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, last_sector2),
        160,
        "VehicleScoringInfoV01::mLastSector2"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, last_lap_time),
        168,
        "VehicleScoringInfoV01::mLastLapTime"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, cur_sector1),
        176,
        "VehicleScoringInfoV01::mCurSector1"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, cur_sector2),
        184,
        "VehicleScoringInfoV01::mCurSector2"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, num_pitstops),
        192,
        "VehicleScoringInfoV01::mNumPitstops"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, num_penalties),
        194,
        "VehicleScoringInfoV01::mNumPenalties"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, is_player),
        196,
        "VehicleScoringInfoV01::mIsPlayer"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, control),
        197,
        "VehicleScoringInfoV01::mControl"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, in_pits),
        198,
        "VehicleScoringInfoV01::mInPits"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, place),
        199,
        "VehicleScoringInfoV01::mPlace"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, vehicle_class),
        200,
        "VehicleScoringInfoV01::mVehicleClass"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, time_behind_next),
        232,
        "VehicleScoringInfoV01::mTimeBehindNext"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, laps_behind_next),
        240,
        "VehicleScoringInfoV01::mLapsBehindNext"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, time_behind_leader),
        244,
        "VehicleScoringInfoV01::mTimeBehindLeader"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, laps_behind_leader),
        252,
        "VehicleScoringInfoV01::mLapsBehindLeader"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, lap_start_et),
        256,
        "VehicleScoringInfoV01::mLapStartET"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, pos),
        264,
        "VehicleScoringInfoV01::mPos"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, local_vel),
        288,
        "VehicleScoringInfoV01::mLocalVel"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, local_accel),
        312,
        "VehicleScoringInfoV01::mLocalAccel"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, ori),
        336,
        "VehicleScoringInfoV01::mOri"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, local_rot),
        408,
        "VehicleScoringInfoV01::mLocalRot"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, local_rot_accel),
        432,
        "VehicleScoringInfoV01::mLocalRotAccel"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, headlights),
        456,
        "VehicleScoringInfoV01::mHeadlights"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, pit_state),
        457,
        "VehicleScoringInfoV01::mPitState"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, server_scored),
        458,
        "VehicleScoringInfoV01::mServerScored"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, individual_phase),
        459,
        "VehicleScoringInfoV01::mIndividualPhase"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, qualification),
        460,
        "VehicleScoringInfoV01::mQualification"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, time_into_lap),
        464,
        "VehicleScoringInfoV01::mTimeIntoLap"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, estimated_lap_time),
        472,
        "VehicleScoringInfoV01::mEstimatedLapTime"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, pit_group),
        480,
        "VehicleScoringInfoV01::mPitGroup"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, flag),
        504,
        "VehicleScoringInfoV01::mFlag"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, under_yellow),
        505,
        "VehicleScoringInfoV01::mUnderYellow"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, count_lap_flag),
        506,
        "VehicleScoringInfoV01::mCountLapFlag"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, in_garage_stall),
        507,
        "VehicleScoringInfoV01::mInGarageStall"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, upgrade_pack),
        508,
        "VehicleScoringInfoV01::mUpgradePack"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, pit_lap_dist),
        524,
        "VehicleScoringInfoV01::mPitLapDist"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, best_lap_sector1),
        528,
        "VehicleScoringInfoV01::mBestLapSector1"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, best_lap_sector2),
        532,
        "VehicleScoringInfoV01::mBestLapSector2"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, steam_id),
        536,
        "VehicleScoringInfoV01::mSteamID"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, veh_filename),
        544,
        "VehicleScoringInfoV01::mVehFilename"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, attack_mode),
        576,
        "VehicleScoringInfoV01::mAttackMode"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, fuel_fraction),
        578,
        "VehicleScoringInfoV01::mFuelFraction"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, drs_state),
        579,
        "VehicleScoringInfoV01::mDRSState"
    );
    assert_eq!(
        offset_of!(VehicleScoringInfoV01, expansion),
        580,
        "VehicleScoringInfoV01::mExpansion"
    );
}

#[test]
fn scoring_info_v01() {
    assert_eq!(size_of::<ScoringInfoV01>(), 548, "sizeof(ScoringInfoV01)");
    assert_eq!(align_of::<ScoringInfoV01>(), 4, "alignof(ScoringInfoV01)");
    assert_eq!(
        offset_of!(ScoringInfoV01, track_name),
        0,
        "ScoringInfoV01::mTrackName"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, session),
        64,
        "ScoringInfoV01::mSession"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, current_et),
        68,
        "ScoringInfoV01::mCurrentET"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, end_et),
        76,
        "ScoringInfoV01::mEndET"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, max_laps),
        84,
        "ScoringInfoV01::mMaxLaps"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, lap_dist),
        88,
        "ScoringInfoV01::mLapDist"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, results_stream),
        96,
        "ScoringInfoV01::mResultsStream"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, num_vehicles),
        104,
        "ScoringInfoV01::mNumVehicles"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, game_phase),
        108,
        "ScoringInfoV01::mGamePhase"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, yellow_flag_state),
        109,
        "ScoringInfoV01::mYellowFlagState"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, sector_flag),
        110,
        "ScoringInfoV01::mSectorFlag"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, start_light),
        113,
        "ScoringInfoV01::mStartLight"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, num_red_lights),
        114,
        "ScoringInfoV01::mNumRedLights"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, in_realtime),
        115,
        "ScoringInfoV01::mInRealtime"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, player_name),
        116,
        "ScoringInfoV01::mPlayerName"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, plr_file_name),
        148,
        "ScoringInfoV01::mPlrFileName"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, dark_cloud),
        212,
        "ScoringInfoV01::mDarkCloud"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, raining),
        220,
        "ScoringInfoV01::mRaining"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, ambient_temp),
        228,
        "ScoringInfoV01::mAmbientTemp"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, track_temp),
        236,
        "ScoringInfoV01::mTrackTemp"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, wind),
        244,
        "ScoringInfoV01::mWind"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, min_path_wetness),
        268,
        "ScoringInfoV01::mMinPathWetness"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, max_path_wetness),
        276,
        "ScoringInfoV01::mMaxPathWetness"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, game_mode),
        284,
        "ScoringInfoV01::mGameMode"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, is_password_protected),
        285,
        "ScoringInfoV01::mIsPasswordProtected"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, server_port),
        286,
        "ScoringInfoV01::mServerPort"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, server_public_ip),
        288,
        "ScoringInfoV01::mServerPublicIP"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, max_players),
        292,
        "ScoringInfoV01::mMaxPlayers"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, server_name),
        296,
        "ScoringInfoV01::mServerName"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, start_et),
        328,
        "ScoringInfoV01::mStartET"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, avg_path_wetness),
        332,
        "ScoringInfoV01::mAvgPathWetness"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, session_time_remaining),
        340,
        "ScoringInfoV01::mSessionTimeRemaining"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, time_of_day),
        344,
        "ScoringInfoV01::mTimeOfDay"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, is_fixed_setup),
        348,
        "ScoringInfoV01::mIsFixedSetup"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, track_grip_level),
        349,
        "ScoringInfoV01::mTrackGripLevel"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, cloud_coverage),
        350,
        "ScoringInfoV01::mCloudCoverage"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, track_limits_steps_per_penalty),
        351,
        "ScoringInfoV01::mTrackLimitsStepsPerPenalty"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, track_limits_steps_per_point),
        352,
        "ScoringInfoV01::mTrackLimitsStepsPerPoint"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, expansion),
        353,
        "ScoringInfoV01::mExpansion"
    );
    assert_eq!(
        offset_of!(ScoringInfoV01, vehicle),
        540,
        "ScoringInfoV01::mVehicle"
    );
}

#[test]
fn application_state_v01() {
    assert_eq!(
        size_of::<ApplicationStateV01>(),
        260,
        "sizeof(ApplicationStateV01)"
    );
    assert_eq!(
        align_of::<ApplicationStateV01>(),
        4,
        "alignof(ApplicationStateV01)"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, app_window),
        0,
        "ApplicationStateV01::mAppWindow"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, width),
        8,
        "ApplicationStateV01::mWidth"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, height),
        12,
        "ApplicationStateV01::mHeight"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, refresh_rate),
        16,
        "ApplicationStateV01::mRefreshRate"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, windowed),
        20,
        "ApplicationStateV01::mWindowed"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, options_location),
        24,
        "ApplicationStateV01::mOptionsLocation"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, options_page),
        25,
        "ApplicationStateV01::mOptionsPage"
    );
    assert_eq!(
        offset_of!(ApplicationStateV01, expansion),
        56,
        "ApplicationStateV01::mExpansion"
    );
}

#[test]
fn shared_memory_generic() {
    assert_eq!(
        size_of::<SharedMemoryGeneric>(),
        332,
        "sizeof(SharedMemoryGeneric)"
    );
    assert_eq!(
        align_of::<SharedMemoryGeneric>(),
        4,
        "alignof(SharedMemoryGeneric)"
    );
    assert_eq!(
        offset_of!(SharedMemoryGeneric, events),
        0,
        "SharedMemoryGeneric::events"
    );
    assert_eq!(
        offset_of!(SharedMemoryGeneric, game_version),
        64,
        "SharedMemoryGeneric::gameVersion"
    );
    assert_eq!(
        offset_of!(SharedMemoryGeneric, ffb_torque),
        68,
        "SharedMemoryGeneric::FFBTorque"
    );
    assert_eq!(
        offset_of!(SharedMemoryGeneric, app_info),
        72,
        "SharedMemoryGeneric::appInfo"
    );
}

#[test]
fn shared_memory_path_data() {
    assert_eq!(
        size_of::<SharedMemoryPathData>(),
        1300,
        "sizeof(SharedMemoryPathData)"
    );
    assert_eq!(
        align_of::<SharedMemoryPathData>(),
        1,
        "alignof(SharedMemoryPathData)"
    );
    assert_eq!(
        offset_of!(SharedMemoryPathData, user_data),
        0,
        "SharedMemoryPathData::userData"
    );
    assert_eq!(
        offset_of!(SharedMemoryPathData, custom_variables),
        260,
        "SharedMemoryPathData::customVariables"
    );
    assert_eq!(
        offset_of!(SharedMemoryPathData, steward_results),
        520,
        "SharedMemoryPathData::stewardResults"
    );
    assert_eq!(
        offset_of!(SharedMemoryPathData, player_profile),
        780,
        "SharedMemoryPathData::playerProfile"
    );
    assert_eq!(
        offset_of!(SharedMemoryPathData, plugins_folder),
        1040,
        "SharedMemoryPathData::pluginsFolder"
    );
}

#[test]
fn shared_memory_scoring_data() {
    assert_eq!(
        size_of::<SharedMemoryScoringData>(),
        126832,
        "sizeof(SharedMemoryScoringData)"
    );
    assert_eq!(
        align_of::<SharedMemoryScoringData>(),
        8,
        "alignof(SharedMemoryScoringData)"
    );
    assert_eq!(
        offset_of!(SharedMemoryScoringData, scoring_info),
        0,
        "SharedMemoryScoringData::scoringInfo"
    );
    assert_eq!(
        offset_of!(SharedMemoryScoringData, scoring_stream_size),
        552,
        "SharedMemoryScoringData::scoringStreamSize"
    );
    assert_eq!(
        offset_of!(SharedMemoryScoringData, veh_scoring_info),
        560,
        "SharedMemoryScoringData::vehScoringInfo"
    );
    assert_eq!(
        offset_of!(SharedMemoryScoringData, scoring_stream),
        61296,
        "SharedMemoryScoringData::scoringStream"
    );
}

#[test]
fn shared_memory_telemetry_data() {
    assert_eq!(
        size_of::<SharedMemoryTelemetryData>(),
        196356,
        "sizeof(SharedMemoryTelemetryData)"
    );
    assert_eq!(
        align_of::<SharedMemoryTelemetryData>(),
        4,
        "alignof(SharedMemoryTelemetryData)"
    );
    assert_eq!(
        offset_of!(SharedMemoryTelemetryData, active_vehicles),
        0,
        "SharedMemoryTelemetryData::activeVehicles"
    );
    assert_eq!(
        offset_of!(SharedMemoryTelemetryData, player_vehicle_idx),
        1,
        "SharedMemoryTelemetryData::playerVehicleIdx"
    );
    assert_eq!(
        offset_of!(SharedMemoryTelemetryData, player_has_vehicle),
        2,
        "SharedMemoryTelemetryData::playerHasVehicle"
    );
    assert_eq!(
        offset_of!(SharedMemoryTelemetryData, telem_info),
        4,
        "SharedMemoryTelemetryData::telemInfo"
    );
}

#[test]
fn shared_memory_object_out() {
    assert_eq!(
        size_of::<SharedMemoryObjectOut>(),
        324824,
        "sizeof(SharedMemoryObjectOut)"
    );
    assert_eq!(
        align_of::<SharedMemoryObjectOut>(),
        8,
        "alignof(SharedMemoryObjectOut)"
    );
    assert_eq!(
        offset_of!(SharedMemoryObjectOut, generic),
        0,
        "SharedMemoryObjectOut::generic"
    );
    assert_eq!(
        offset_of!(SharedMemoryObjectOut, paths),
        332,
        "SharedMemoryObjectOut::paths"
    );
    assert_eq!(
        offset_of!(SharedMemoryObjectOut, scoring),
        1632,
        "SharedMemoryObjectOut::scoring"
    );
    assert_eq!(
        offset_of!(SharedMemoryObjectOut, telemetry),
        128464,
        "SharedMemoryObjectOut::telemetry"
    );
}

#[test]
fn whole_block_constants() {
    assert_eq!(size_of::<SharedMemoryObjectOut>(), SHM_TOTAL_SIZE);
    assert_eq!(SHM_TOTAL_SIZE - 4, SHM_USEFUL_SIZE);
    assert_eq!(
        OFF_TELEM_INFO + MAX_VEHICLES * SIZE_TELEM_INFO,
        SHM_USEFUL_SIZE
    );
    assert_eq!(OFF_SCORING_STREAM + SCORING_STREAM_LEN, OFF_TELEMETRY);
    assert_eq!(OFF_CURRENT_ET, 1700);
    assert_eq!(telem_elapsed_time_offset(0), 128_480);
    assert_eq!(telem_elapsed_time_offset(103), 128_468 + 103 * 1888 + 12);
}
