//! Derives [`GameEvent`]s from the stream of frames, whatever the source.

use corde_protocol::{
    FinishStatus, GameEvent, GamePhase, SessionType, TelemetryFrame, YellowFlagState,
};

/// Once the game says "session over", how long the player's own end
/// condition (flag taken, or back in the menus) must hold before the session
/// is declared ended. Leaves time for the last lap to be made official.
const END_SETTLE_MS: u64 = 3_000;
/// Give up waiting for the player to finish after this long.
const END_MAX_WAIT_MS: u64 = 15 * 60 * 1_000;

#[derive(Debug, Clone)]
struct Prev {
    track: String,
    session_index: i32,
    session_type: SessionType,
    phase: GamePhase,
    yellow: YellowFlagState,
    in_realtime: bool,
    in_pits: Option<bool>,
    last_impact_s: f64,
    /// `SessionEnded` already sent for this session.
    ended: bool,
    /// When the game phase became "session over".
    over_since_ms: Option<u64>,
    /// Since when the player is done (flag taken / out of the car).
    done_since_ms: Option<u64>,
}

/// Wall clock of a frame, falling back on the session clock (tests, replays).
fn clock_ms(f: &TelemetryFrame) -> u64 {
    if f.wall_ms > 0 {
        f.wall_ms
    } else {
        (f.session.elapsed_s.max(0.0) * 1000.0) as u64
    }
}

/// Stateful detector: feed every frame, collect the returned events.
#[derive(Debug, Default)]
pub struct EventDetector {
    prev: Option<Prev>,
}

impl EventDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` when the frame belongs to a different session than the previous one.
    fn session_changed(prev: &Prev, f: &TelemetryFrame) -> bool {
        prev.track != f.session.track_name
            || prev.session_index != f.session.session_index
            || prev.session_type != f.session.session_type
    }

    pub fn feed(&mut self, f: &TelemetryFrame) -> Vec<GameEvent> {
        let mut out = Vec::new();
        let s = &f.session;
        let player = f.player.as_ref();
        let in_pits = player.map(|p| p.in_pits || p.lap.in_pit_lane);
        let lap_number = player.map(|p| p.lap.lap_number).unwrap_or(0);
        let impact_s = player.map(|p| p.damage.last_impact_s).unwrap_or(0.0);

        let now_ms = clock_ms(f);
        let mut ended = false;
        let mut over_since_ms = None;
        let mut done_since_ms = None;
        match &self.prev {
            None => out.push(GameEvent::SessionStarted { session: s.clone() }),
            Some(prev) if Self::session_changed(prev, f) => {
                if !prev.ended {
                    out.push(GameEvent::SessionEnded);
                }
                out.push(GameEvent::SessionStarted { session: s.clone() });
            }
            Some(prev) => {
                ended = prev.ended;
                if prev.phase != s.phase || prev.yellow != s.yellow_flag {
                    out.push(GameEvent::FlagChanged {
                        phase: s.phase,
                        yellow: s.yellow_flag,
                    });
                }
                // "Session over" is the chequered flag for the *leader*: the
                // player is usually still on his last lap, which counts. The
                // session only ends for us once the player has taken the flag
                // (or left the car), and stayed so for a few seconds.
                if s.phase == GamePhase::SessionOver {
                    let over_since = prev.over_since_ms.unwrap_or(now_ms);
                    over_since_ms = Some(over_since);
                    let player_done = match player {
                        Some(p) => p.finish_status != FinishStatus::None || !s.in_realtime,
                        None => true,
                    };
                    if player_done {
                        done_since_ms = Some(prev.done_since_ms.unwrap_or(now_ms));
                    }
                    let settled =
                        done_since_ms.is_some_and(|t| now_ms.saturating_sub(t) >= END_SETTLE_MS);
                    let too_long = now_ms.saturating_sub(over_since) >= END_MAX_WAIT_MS;
                    if !ended && (settled || too_long) {
                        out.push(GameEvent::SessionEnded);
                        ended = true;
                    }
                } else {
                    // Back to a live phase (restart): a new end can be declared.
                    ended = false;
                }
                if prev.in_realtime != s.in_realtime {
                    out.push(if s.in_realtime {
                        GameEvent::EnteredRealtime
                    } else {
                        GameEvent::ExitedRealtime
                    });
                }
                if let (Some(was), Some(now)) = (prev.in_pits, in_pits) {
                    if !was && now {
                        out.push(GameEvent::PitEntry { lap_number });
                    } else if was && !now {
                        out.push(GameEvent::PitExit { lap_number });
                    }
                }
                if let Some(p) = player {
                    if impact_s > prev.last_impact_s && p.damage.last_impact_magnitude > 0.0 {
                        out.push(GameEvent::Impact {
                            magnitude: p.damage.last_impact_magnitude,
                            lap_number,
                        });
                    }
                }
            }
        }

        self.prev = Some(Prev {
            track: s.track_name.clone(),
            session_index: s.session_index,
            session_type: s.session_type,
            phase: s.phase,
            yellow: s.yellow_flag,
            in_realtime: s.in_realtime,
            in_pits,
            last_impact_s: impact_s,
            ended,
            over_since_ms,
            done_since_ms,
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use corde_protocol::PlayerTelemetry;

    fn frame(track: &str, phase: GamePhase, in_pits: bool) -> TelemetryFrame {
        let mut f = TelemetryFrame::default();
        f.session.track_name = track.into();
        f.session.phase = phase;
        f.session.in_realtime = true;
        let mut p = PlayerTelemetry {
            in_pits,
            ..Default::default()
        };
        p.lap.lap_number = 3;
        f.player = Some(p);
        f
    }

    #[test]
    fn detects_session_flags_pits_and_impacts() {
        let mut d = EventDetector::new();
        let ev = d.feed(&frame("A", GamePhase::Green, false));
        assert!(matches!(ev.as_slice(), [GameEvent::SessionStarted { .. }]));
        assert!(d.feed(&frame("A", GamePhase::Green, false)).is_empty());
        let ev = d.feed(&frame("A", GamePhase::FullCourseYellow, false));
        assert!(matches!(
            ev.as_slice(),
            [GameEvent::FlagChanged {
                phase: GamePhase::FullCourseYellow,
                ..
            }]
        ));
        let ev = d.feed(&frame("A", GamePhase::FullCourseYellow, true));
        assert!(matches!(
            ev.as_slice(),
            [GameEvent::PitEntry { lap_number: 3 }]
        ));
        let ev = d.feed(&frame("A", GamePhase::FullCourseYellow, false));
        assert!(matches!(
            ev.as_slice(),
            [GameEvent::PitExit { lap_number: 3 }]
        ));
        let mut hit = frame("A", GamePhase::FullCourseYellow, false);
        let p = hit.player.as_mut().unwrap();
        p.damage.last_impact_s = 12.0;
        p.damage.last_impact_magnitude = 3.5;
        let ev = d.feed(&hit);
        assert!(matches!(
            ev.as_slice(),
            [GameEvent::Impact { lap_number: 3, .. }]
        ));
        let ev = d.feed(&frame("B", GamePhase::Green, false));
        assert!(matches!(
            ev.as_slice(),
            [GameEvent::SessionEnded, GameEvent::SessionStarted { .. }]
        ));
        let mut over = frame("B", GamePhase::SessionOver, false);
        over.session.in_realtime = false;
        let ev = d.feed(&over);
        assert_eq!(ev.len(), 2, "{ev:?}");
        assert!(matches!(ev[0], GameEvent::FlagChanged { .. }));
        assert!(matches!(ev[1], GameEvent::ExitedRealtime));
        // Out of the car for a few seconds: the session is over for us, once.
        over.session.elapsed_s = 2.0;
        assert!(d.feed(&over).is_empty());
        over.session.elapsed_s = 3.5;
        let ev = d.feed(&over);
        assert!(matches!(ev.as_slice(), [GameEvent::SessionEnded]), "{ev:?}");
        over.session.elapsed_s = 60.0;
        assert!(d.feed(&over).is_empty());
        // The next session does not repeat it.
        let ev = d.feed(&frame("C", GamePhase::Green, false));
        assert!(
            matches!(ev.as_slice(), [GameEvent::SessionStarted { .. }]),
            "{ev:?}"
        );
    }

    #[test]
    fn the_session_ends_when_the_player_takes_the_flag_not_the_leader() {
        let mut d = EventDetector::new();
        let at = |t: f64, phase, finish| {
            let mut f = frame("A", phase, false);
            f.session.elapsed_s = t;
            f.player.as_mut().unwrap().finish_status = finish;
            f
        };
        d.feed(&at(1000.0, GamePhase::Green, FinishStatus::None));
        // The leader takes the flag: the player is still racing.
        let ev = d.feed(&at(1800.0, GamePhase::SessionOver, FinishStatus::None));
        assert!(
            matches!(ev.as_slice(), [GameEvent::FlagChanged { .. }]),
            "{ev:?}"
        );
        for i in 1..60 {
            let ev = d.feed(&at(
                1800.0 + i as f64,
                GamePhase::SessionOver,
                FinishStatus::None,
            ));
            assert!(ev.is_empty(), "{ev:?}");
        }
        // The player crosses the line; the end is declared 3 s later.
        assert!(d
            .feed(&at(1861.0, GamePhase::SessionOver, FinishStatus::Finished))
            .is_empty());
        assert!(d
            .feed(&at(1863.0, GamePhase::SessionOver, FinishStatus::Finished))
            .is_empty());
        let ev = d.feed(&at(1864.1, GamePhase::SessionOver, FinishStatus::Finished));
        assert!(matches!(ev.as_slice(), [GameEvent::SessionEnded]), "{ev:?}");
        assert!(d
            .feed(&at(1900.0, GamePhase::SessionOver, FinishStatus::Finished))
            .is_empty());

        // A player who never finishes does not keep the session open for ever.
        let mut d = EventDetector::new();
        d.feed(&at(1000.0, GamePhase::Green, FinishStatus::None));
        d.feed(&at(1800.0, GamePhase::SessionOver, FinishStatus::None));
        assert!(d
            .feed(&at(
                1800.0 + 899.0,
                GamePhase::SessionOver,
                FinishStatus::None
            ))
            .is_empty());
        let ev = d.feed(&at(
            1800.0 + 901.0,
            GamePhase::SessionOver,
            FinishStatus::None,
        ));
        assert!(matches!(ev.as_slice(), [GameEvent::SessionEnded]), "{ev:?}");
    }
}
