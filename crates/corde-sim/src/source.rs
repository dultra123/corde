//! [`SimSource`]: the simulator seen through [`corde_core::TelemetrySource`].
//!
//! The world is advanced on a **monotonic clock**: on every `poll` we compute
//! how many ticks are due since `connect` and run exactly that many, so the
//! simulated time never drifts from real time however irregular the polling
//! is. When the process was suspended for more than a second (laptop asleep,
//! debugger…) we resynchronise instead of simulating the gap.

use crate::world::SimWorld;
use crate::SimConfig;
use corde_core::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
struct Clock {
    start: Instant,
    /// Ticks run since `start`.
    emitted: u64,
}

/// Real-time telemetry source backed by a [`SimWorld`].
#[derive(Debug)]
pub struct SimSource {
    world: SimWorld,
    rate_hz: f32,
    /// Simulated seconds per real second.
    time_scale: f64,
    clock: Option<Clock>,
    /// Maximum number of ticks run in one `poll` before we resynchronise.
    max_catch_up: u64,
}

impl SimSource {
    pub fn new(cfg: SimConfig) -> Self {
        let cfg = cfg.sanitized();
        let rate_hz = cfg.rate_hz;
        let time_scale = cfg.time_scale;
        Self {
            world: SimWorld::new(cfg),
            rate_hz,
            time_scale,
            clock: None,
            // One real second worth of ticks.
            max_catch_up: (rate_hz as f64 * time_scale).ceil().max(1.0) as u64,
        }
    }

    pub fn world(&self) -> &SimWorld {
        &self.world
    }

    pub fn world_mut(&mut self) -> &mut SimWorld {
        &mut self.world
    }

    pub fn rate_hz(&self) -> f32 {
        self.rate_hz
    }

    pub fn time_scale(&self) -> f64 {
        self.time_scale
    }

    /// Number of ticks due at `now` according to the monotonic clock.
    fn due_ticks(&self, now: Instant) -> Option<u64> {
        let clock = self.clock?;
        let elapsed = now.saturating_duration_since(clock.start).as_secs_f64() * self.time_scale;
        Some((elapsed * self.rate_hz as f64).floor() as u64)
    }
}

impl TelemetrySource for SimSource {
    fn info(&self) -> SourceInfo {
        SourceInfo {
            game: GameId::Sim,
            id: "sim",
            label: format!(
                "Simulateur intégré — Paul Ricard 1A, {} ({} IA, {:.0} Hz{})",
                self.world.config().player_class,
                self.world.config().num_ai,
                self.rate_hz,
                if (self.time_scale - 1.0).abs() > 1e-9 {
                    format!(", ×{}", self.time_scale)
                } else {
                    String::new()
                }
            ),
            native_rate_hz: self.rate_hz,
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        if self.clock.is_none() {
            self.clock = Some(Clock {
                start: Instant::now(),
                emitted: 0,
            });
        }
        Ok(())
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let now = Instant::now();
        let due = self
            .due_ticks(now)
            .ok_or_else(|| SourceError::NotAvailable("sim: connect() not called".into()))?;
        let clock = self.clock.as_mut().expect("clock checked above");
        if due <= clock.emitted {
            return Ok(None);
        }
        let mut behind = due - clock.emitted;
        if behind > self.max_catch_up {
            // Long suspension: skip the gap rather than simulating it all.
            behind = 1;
        }
        let mut frame = None;
        for _ in 0..behind {
            frame = Some(self.world.tick());
        }
        clock.emitted = due;
        Ok(frame)
    }

    fn disconnect(&mut self) {
        self.clock = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;
    use std::time::Duration;

    #[test]
    fn poll_requires_connect_then_paces_on_the_clock() {
        let mut src = SimSource::new(SimConfig::default().with_rate_hz(200.0).with_num_ai(3));
        assert!(matches!(src.poll(), Err(SourceError::NotAvailable(_))));
        src.connect().unwrap();
        assert_eq!(src.info().id, "sim");
        // Immediately after connect nothing is due yet (or at most a tick).
        let first = src.poll().unwrap();
        assert!(first.is_none() || first.unwrap().seq <= 2);
        sleep(Duration::from_millis(60));
        let f = src.poll().unwrap().expect("a frame is due after 60 ms");
        // ≈ 12 ticks at 200 Hz; allow generous scheduling slack.
        assert!((8..=40).contains(&f.seq), "seq {}", f.seq);
        assert_eq!(f.source, "sim");
        assert!(f.player.is_some());
    }

    #[test]
    fn long_suspension_is_skipped_not_simulated() {
        let mut src = SimSource::new(SimConfig::default().with_rate_hz(100.0).with_num_ai(1));
        src.connect().unwrap();
        // Pretend `connect` happened 10 s ago.
        src.clock = Some(Clock {
            start: Instant::now() - Duration::from_secs(10),
            emitted: 0,
        });
        let f = src.poll().unwrap().unwrap();
        assert_eq!(f.seq, 1, "only one tick must have been run");
        assert!(src.poll().unwrap().is_none());
    }
}
