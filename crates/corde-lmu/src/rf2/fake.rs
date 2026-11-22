//! A fake rFactor 2: the fake Le Mans Ultimate world, written the way rFactor 2's plugin
//! writes it (two buffers, each behind its version block, nothing in the reserve).

use super::layout::{
    OFF_SCORING_INFO, OFF_SCORING_VEHICLES, OFF_TELEM_NUM, OFF_TELEM_VEHICLES, OFF_VERSION_BEGIN,
    OFF_VERSION_END, SCORING_INFO_RESERVED, SIZE_WHEEL, TELEM_ELECTRIC, TELEM_ELECTRIC_LMU,
    TELEM_SAME, TELEM_WHEELS, VEH_SCORING_RESERVED, WHEEL_LMU_ONLY,
};
use super::reader::Rf2Raw;
use crate::fake::FakeWorld;
use crate::layout::{
    self as lmu, SharedMemoryObjectOut, SIZE_SCORING_INFO, SIZE_TELEM_INFO,
    SIZE_VEHICLE_SCORING_INFO,
};

fn set_versions(buf: &mut [u8], version: u32) {
    buf[OFF_VERSION_BEGIN..OFF_VERSION_BEGIN + 4].copy_from_slice(&version.to_le_bytes());
    buf[OFF_VERSION_END..OFF_VERSION_END + 4].copy_from_slice(&version.to_le_bytes());
}

/// The two buffers rFactor 2's plugin would publish for what this block of Le Mans
/// Ultimate describes. `telemetry_version` and `scoring_version` are the write counters.
pub fn buffers_from_block(
    block: &SharedMemoryObjectOut,
    telemetry_version: u32,
    scoring_version: u32,
) -> Rf2Raw {
    let bytes = bytemuck::bytes_of(block);
    let mut raw = Rf2Raw::new();

    set_versions(&mut raw.telemetry, telemetry_version);
    let active = block.telemetry.vehicles().len();
    raw.telemetry[OFF_TELEM_NUM..OFF_TELEM_NUM + 4].copy_from_slice(&(active as i32).to_le_bytes());
    for i in 0..active {
        let src = lmu::OFF_TELEM_INFO + i * SIZE_TELEM_INFO;
        let dst = OFF_TELEM_VEHICLES + i * SIZE_TELEM_INFO;
        let lmu = &bytes[src..src + SIZE_TELEM_INFO];
        let out = &mut raw.telemetry[dst..dst + SIZE_TELEM_INFO];
        out[TELEM_SAME].copy_from_slice(&lmu[TELEM_SAME]);
        out[TELEM_ELECTRIC]
            .copy_from_slice(&lmu[TELEM_ELECTRIC_LMU..TELEM_ELECTRIC_LMU + TELEM_ELECTRIC.len()]);
        out[TELEM_WHEELS].copy_from_slice(&lmu[TELEM_WHEELS]);
        for w in 0..4 {
            let wheel = TELEM_WHEELS.start + w * SIZE_WHEEL;
            out[wheel + WHEEL_LMU_ONLY.start..wheel + WHEEL_LMU_ONLY.end].fill(0);
        }
    }

    set_versions(&mut raw.scoring, scoring_version);
    let src = lmu::OFF_SCORING_INFO;
    raw.scoring[OFF_SCORING_INFO..OFF_SCORING_INFO + SIZE_SCORING_INFO]
        .copy_from_slice(&bytes[src..src + SIZE_SCORING_INFO]);
    raw.scoring[OFF_SCORING_INFO + SCORING_INFO_RESERVED.start
        ..OFF_SCORING_INFO + SCORING_INFO_RESERVED.end]
        .fill(0);
    for i in 0..block.scoring.vehicles().len() {
        let src = lmu::OFF_VEH_SCORING_INFO + i * SIZE_VEHICLE_SCORING_INFO;
        let dst = OFF_SCORING_VEHICLES + i * SIZE_VEHICLE_SCORING_INFO;
        raw.scoring[dst..dst + SIZE_VEHICLE_SCORING_INFO]
            .copy_from_slice(&bytes[src..src + SIZE_VEHICLE_SCORING_INFO]);
        raw.scoring[dst + VEH_SCORING_RESERVED.start..dst + VEH_SCORING_RESERVED.end].fill(0);
    }
    raw
}

/// A session of the fake game: the world moves, the plugin writes.
pub struct FakeRf2 {
    pub world: FakeWorld,
    block: Box<SharedMemoryObjectOut>,
    telemetry_version: u32,
    scoring_version: u32,
    since_scoring: f64,
    /// The scoring buffer as last written: the game only refreshes it five times a second.
    scoring: Vec<u8>,
}

impl Default for FakeRf2 {
    fn default() -> Self {
        Self::new()
    }
}

impl FakeRf2 {
    pub fn new() -> Self {
        Self {
            world: FakeWorld::new(),
            block: bytemuck::zeroed_box(),
            telemetry_version: 0,
            scoring_version: 0,
            // The first step writes the scoring too.
            since_scoring: f64::MAX,
            scoring: Vec::new(),
        }
    }

    /// Moves the world by `dt` seconds and gives what the plugin publishes then: the
    /// telemetry at every step, the scoring five times a second like the game.
    pub fn step(&mut self, dt: f64) -> Rf2Raw {
        self.world.step(dt);
        self.world.write_into(&mut self.block);
        self.telemetry_version = self.telemetry_version.wrapping_add(1);
        self.since_scoring += dt;
        let fresh = self.since_scoring >= 0.2 || self.scoring.is_empty();
        if fresh {
            self.since_scoring = 0.0;
            self.scoring_version = self.scoring_version.wrapping_add(1);
        }
        let mut raw = buffers_from_block(&self.block, self.telemetry_version, self.scoring_version);
        if fresh {
            self.scoring.clone_from(&raw.scoring);
        } else {
            raw.scoring.clone_from(&self.scoring);
        }
        raw
    }

    /// What the plugin leaves when the session ends: two blank buffers (it clears them),
    /// their counters still moving once.
    pub fn ended(&mut self) -> Rf2Raw {
        self.telemetry_version = self.telemetry_version.wrapping_add(1);
        self.scoring_version = self.scoring_version.wrapping_add(1);
        let mut raw = Rf2Raw::new();
        set_versions(&mut raw.telemetry, self.telemetry_version);
        set_versions(&mut raw.scoring, self.scoring_version);
        self.scoring.clear();
        raw
    }
}
