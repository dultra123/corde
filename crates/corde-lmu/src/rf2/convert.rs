//! The two buffers of rFactor 2 → a `TelemetryFrame`, through Le Mans Ultimate's block.

use super::layout::{
    i32_at, MAX_VEHICLES as RF2_MAX_VEHICLES, OFF_SCORING_INFO, OFF_SCORING_VEHICLES,
    OFF_TELEM_VEHICLES, SCORING_INFO_RESERVED, SIZE_WHEEL, TELEM_ELECTRIC, TELEM_ELECTRIC_LMU,
    TELEM_SAME, TELEM_WHEELS, VEH_SCORING_RESERVED, WHEEL_LMU_ONLY,
};
use super::reader::Rf2Raw;
use crate::convert::to_frame;
use crate::layout::{
    self as lmu, championship, vehicle_class, SharedMemoryObjectOut, TelemInfoV01, TelemWheelV01,
    VehicleScoringInfoV01, MAX_VEHICLES, SIZE_SCORING_INFO, SIZE_TELEM_INFO,
    SIZE_VEHICLE_SCORING_INFO,
};
use corde_protocol::{GameId, TelemetryFrame};
use std::mem::offset_of;

/// `VehicleScoringInfoV01::mIsPlayer` inside a vehicle's scoring.
const VEH_IS_PLAYER: usize = offset_of!(VehicleScoringInfoV01, is_player);

/// The entries to keep when a buffer holds more vehicles than Le Mans Ultimate's block
/// has room for: the first ones, the wanted one in the last place if it would be cut.
fn pick(n: usize, wanted: Option<usize>) -> Vec<usize> {
    let mut picked: Vec<usize> = (0..n.min(MAX_VEHICLES)).collect();
    if let Some(w) = wanted {
        if w < n && w >= MAX_VEHICLES {
            if let Some(last) = picked.last_mut() {
                *last = w;
            }
        }
    }
    picked
}

/// Puts the two buffers in the shape of Le Mans Ultimate's block. The bytes rFactor 2
/// keeps in reserve are blanked (Le Mans Ultimate reads them as fields), then the few of
/// those fields rFactor 2 can answer are filled from what it publishes.
pub fn assemble(raw: &Rf2Raw, block: &mut SharedMemoryObjectOut) {
    let bytes = bytemuck::bytes_of_mut(block);
    bytes.fill(0);

    // The session.
    let info = lmu::OFF_SCORING_INFO;
    bytes[info..info + SIZE_SCORING_INFO]
        .copy_from_slice(&raw.scoring[OFF_SCORING_INFO..OFF_SCORING_INFO + SIZE_SCORING_INFO]);
    bytes[info + SCORING_INFO_RESERVED.start..info + SCORING_INFO_RESERVED.end].fill(0);

    // The vehicles of the scoring, the player's among them.
    let veh = |i: usize| OFF_SCORING_VEHICLES + i * SIZE_VEHICLE_SCORING_INFO;
    let n = raw.scoring_vehicles();
    let player = (0..n).find(|&i| raw.scoring[veh(i) + VEH_IS_PLAYER] != 0);
    let player_id = player.map(|i| i32_at(&raw.scoring, veh(i)));
    let picked = pick(n, player);
    for (slot, &i) in picked.iter().enumerate() {
        let dst = lmu::OFF_VEH_SCORING_INFO + slot * SIZE_VEHICLE_SCORING_INFO;
        bytes[dst..dst + SIZE_VEHICLE_SCORING_INFO]
            .copy_from_slice(&raw.scoring[veh(i)..veh(i) + SIZE_VEHICLE_SCORING_INFO]);
        bytes[dst + VEH_SCORING_RESERVED.start..dst + VEH_SCORING_RESERVED.end].fill(0);
    }
    let scored = picked.len();

    // The telemetry: the plugin lists the vehicles in the order the game hands them
    // over; the player's is found by its slot id.
    let tel = |i: usize| OFF_TELEM_VEHICLES + i * SIZE_TELEM_INFO;
    let tn = raw.telemetry_vehicles().min(RF2_MAX_VEHICLES);
    let player_tel =
        player_id.and_then(|id| (0..tn).find(|&i| i32_at(&raw.telemetry, tel(i)) == id));
    let picked = pick(tn, player_tel);
    let mut player_slot = None;
    for (slot, &i) in picked.iter().enumerate() {
        let dst = lmu::OFF_TELEM_INFO + slot * SIZE_TELEM_INFO;
        let src = &raw.telemetry[tel(i)..tel(i) + SIZE_TELEM_INFO];
        // Field for field: what sits at the same place, the battery and the electric
        // motor 8 bytes further, the wheels. The rest (the block was blanked) is Le Mans
        // Ultimate's own.
        bytes[dst + TELEM_SAME.start..dst + TELEM_SAME.end].copy_from_slice(&src[TELEM_SAME]);
        bytes[dst + TELEM_ELECTRIC_LMU..dst + TELEM_ELECTRIC_LMU + TELEM_ELECTRIC.len()]
            .copy_from_slice(&src[TELEM_ELECTRIC]);
        bytes[dst + TELEM_WHEELS.start..dst + TELEM_WHEELS.end].copy_from_slice(&src[TELEM_WHEELS]);
        for w in 0..4 {
            let wheel = dst + TELEM_WHEELS.start + w * SIZE_WHEEL;
            bytes[wheel + WHEEL_LMU_ONLY.start..wheel + WHEEL_LMU_ONLY.end].fill(0);
            // Not « soft »: not known.
            bytes[wheel + offset_of!(TelemWheelV01, compound_type)] = u8::MAX;
        }
        // No class nor championship number here: the name of the class says it.
        bytes[dst + offset_of!(TelemInfoV01, vehicle_class)] = vehicle_class::UNKNOWN;
        bytes[dst + offset_of!(TelemInfoV01, vehicle_championship)] = championship::UNKNOWN;
        if Some(i) == player_tel {
            player_slot = Some(slot);
        }
    }
    bytes[lmu::OFF_ACTIVE_VEHICLES] = picked.len() as u8;
    bytes[lmu::OFF_PLAYER_VEHICLE_IDX] = player_slot.unwrap_or(0) as u8;
    bytes[lmu::OFF_PLAYER_HAS_VEHICLE] = u8::from(player_slot.is_some());

    // What Le Mans Ultimate publishes ready-made and rFactor 2 lets us work out.
    let s = &mut block.scoring.scoring_info;
    s.num_vehicles = scored as i32;
    let (current, end, start, dark) = (s.current_et, s.end_et, s.start_et, s.dark_cloud);
    s.session_time_remaining = if end.is_finite() && current.is_finite() && end > 0.0 {
        (end - current).max(0.0) as f32
    } else {
        0.0
    };
    // The event starts at `start` seconds after midnight.
    s.time_of_day = if start.is_finite() && current.is_finite() && start >= 0.0 {
        ((f64::from(start) + current.max(0.0)) % 86_400.0) as f32
    } else {
        0.0
    };
    s.cloud_coverage = if dark.is_finite() && (0.0..=1.0).contains(&dark) {
        (dark * 10.0).round() as u8
    } else {
        u8::MAX
    };
    // Not published: « unknown », not « green track ».
    s.track_grip_level = u8::MAX;
}

/// Converts the copies of the buffers, one frame per copy.
pub struct Rf2Converter {
    block: Box<SharedMemoryObjectOut>,
    prev: Option<TelemetryFrame>,
    seq: u64,
}

impl Default for Rf2Converter {
    fn default() -> Self {
        Self::new()
    }
}

impl Rf2Converter {
    pub fn new() -> Self {
        Self {
            block: bytemuck::zeroed_box(),
            prev: None,
            seq: 0,
        }
    }

    /// The buffers in the shape of Le Mans Ultimate's block, as of the last frame.
    pub fn block(&self) -> &SharedMemoryObjectOut {
        &self.block
    }

    pub fn frame(&mut self, raw: &Rf2Raw, source_id: &str) -> TelemetryFrame {
        assemble(raw, &mut self.block);
        self.seq += 1;
        let mut frame = to_frame(&self.block, self.prev.as_ref(), self.seq, source_id);
        frame.session.game = GameId::Rfactor2;
        if let Some(p) = frame.player.as_mut() {
            // The gaps to the places ahead and behind come ready-made in Le Mans
            // Ultimate; here the scoring gives them (same lap only).
            let vehicles = self.block.scoring.vehicles();
            let gap = |v: &VehicleScoringInfoV01| {
                let (t, laps) = (v.time_behind_next, v.laps_behind_next);
                (laps == 0 && t.is_finite() && t > 0.0).then_some(t as f32)
            };
            if let Some(me) = self.block.player_scoring() {
                let place = me.place;
                p.gaps.place_ahead_s = gap(me);
                p.gaps.place_behind_s = vehicles
                    .iter()
                    .find(|v| v.place == place.saturating_add(1))
                    .and_then(gap);
            }
        }
        self.prev = Some(TelemetryFrame {
            player: frame.player.clone(),
            ..Default::default()
        });
        frame
    }
}
