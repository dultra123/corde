//! [`AutoSource`] — several games behind one source: whichever is running.
//!
//! Corde does not ask which game is played. Each game has its adapter (a
//! [`TelemetrySource`]); this source holds them all, in a fixed order, and:
//!
//! * `connect` asks each adapter in turn; the first one that connects becomes the active
//!   game. An adapter refuses when its game is not running — or is running without a
//!   session, for the games that say so — which is cheap (a few system calls);
//! * `poll` reads the active game; when it reports itself gone, the source goes back to
//!   looking at all of them (the caller retries `connect` with its usual back-off).
//!
//! The order only matters if two games run at once: the first of the list wins.

use crate::source::{SourceError, SourceInfo, TelemetrySource};
use corde_protocol::{GameId, TelemetryFrame};
use std::time::Duration;

/// Identifier used in `TelemetryFrame::source` while no game is active.
pub const AUTO_ID: &str = "auto";

pub struct AutoSource {
    candidates: Vec<Box<dyn TelemetrySource>>,
    active: Option<usize>,
}

impl AutoSource {
    /// `candidates`: one adapter per game, the preferred one first.
    pub fn new(candidates: Vec<Box<dyn TelemetrySource>>) -> Self {
        Self {
            candidates,
            active: None,
        }
    }

    /// The games looked for, in order.
    pub fn games(&self) -> Vec<GameId> {
        self.candidates.iter().map(|c| c.info().game).collect()
    }

    /// The game being read, if any.
    pub fn active_game(&self) -> Option<GameId> {
        self.active.map(|i| self.candidates[i].info().game)
    }
}

impl TelemetrySource for AutoSource {
    fn info(&self) -> SourceInfo {
        match self.active {
            Some(i) => self.candidates[i].info(),
            None => SourceInfo {
                game: GameId::Unknown,
                id: AUTO_ID,
                label: format!(
                    "détection du jeu ({})",
                    self.candidates
                        .iter()
                        .map(|c| c.info().label)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
                native_rate_hz: self
                    .candidates
                    .iter()
                    .map(|c| c.info().native_rate_hz)
                    .fold(1.0, f32::max),
            },
        }
    }

    fn connect(&mut self) -> Result<(), SourceError> {
        if let Some(i) = self.active.take() {
            self.candidates[i].disconnect();
        }
        let mut why = Vec::new();
        for (i, c) in self.candidates.iter_mut().enumerate() {
            match c.connect() {
                Ok(()) => {
                    self.active = Some(i);
                    return Ok(());
                }
                Err(e) => why.push(format!("{} : {e}", c.info().label)),
            }
        }
        Err(SourceError::NotAvailable(if why.is_empty() {
            "aucun jeu pris en charge".into()
        } else {
            format!("aucun jeu en cours ({})", why.join(" ; "))
        }))
    }

    fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
        let Some(i) = self.active else {
            return Err(SourceError::NotAvailable("aucun jeu en cours".into()));
        };
        match self.candidates[i].poll() {
            Err(SourceError::NotAvailable(why)) => {
                self.candidates[i].disconnect();
                self.active = None;
                Err(SourceError::NotAvailable(why))
            }
            other => other,
        }
    }

    fn poll_interval(&self) -> Duration {
        match self.active {
            Some(i) => self.candidates[i].poll_interval(),
            None => Duration::from_millis(5),
        }
    }

    fn disconnect(&mut self) {
        if let Some(i) = self.active.take() {
            self.candidates[i].disconnect();
        }
    }

    fn user_data_dir(&self) -> Option<std::path::PathBuf> {
        self.active.and_then(|i| self.candidates[i].user_data_dir())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
    use std::sync::Arc;

    /// A game that is running or not, on demand.
    struct Game {
        id: GameId,
        running: Arc<AtomicBool>,
        connects: Arc<AtomicU32>,
        disconnects: Arc<AtomicU32>,
    }

    struct Handle {
        running: Arc<AtomicBool>,
        connects: Arc<AtomicU32>,
        disconnects: Arc<AtomicU32>,
    }

    fn game(id: GameId) -> (Box<dyn TelemetrySource>, Handle) {
        let h = Handle {
            running: Arc::new(AtomicBool::new(false)),
            connects: Arc::new(AtomicU32::new(0)),
            disconnects: Arc::new(AtomicU32::new(0)),
        };
        let g = Game {
            id,
            running: Arc::clone(&h.running),
            connects: Arc::clone(&h.connects),
            disconnects: Arc::clone(&h.disconnects),
        };
        (Box::new(g), h)
    }

    impl TelemetrySource for Game {
        fn info(&self) -> SourceInfo {
            SourceInfo {
                game: self.id,
                id: if self.id == GameId::Lmu {
                    "lmu-shm"
                } else {
                    "acc-shm"
                },
                label: format!("{:?}", self.id),
                native_rate_hz: if self.id == GameId::Lmu { 100.0 } else { 60.0 },
            }
        }

        fn connect(&mut self) -> Result<(), SourceError> {
            self.connects.fetch_add(1, Ordering::Relaxed);
            if self.running.load(Ordering::Relaxed) {
                Ok(())
            } else {
                Err(SourceError::NotAvailable("pas lancé".into()))
            }
        }

        fn poll(&mut self) -> Result<Option<TelemetryFrame>, SourceError> {
            if !self.running.load(Ordering::Relaxed) {
                return Err(SourceError::NotAvailable("fermé".into()));
            }
            let mut f = TelemetryFrame::default();
            f.session.game = self.id;
            Ok(Some(f))
        }

        fn disconnect(&mut self) {
            self.disconnects.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn the_running_game_is_found_and_followed_from_one_game_to_the_other() {
        let (lmu, lmu_h) = game(GameId::Lmu);
        let (acc, acc_h) = game(GameId::Acc);
        let mut auto = AutoSource::new(vec![lmu, acc]);
        assert_eq!(auto.games(), [GameId::Lmu, GameId::Acc]);

        // No game: not available, and the reason names every game looked for.
        assert_eq!(auto.info().game, GameId::Unknown);
        assert_eq!(auto.info().id, AUTO_ID);
        assert_eq!(auto.info().native_rate_hz, 100.0);
        let err = auto.connect().unwrap_err().to_string();
        assert!(err.contains("Lmu") && err.contains("Acc"), "{err}");
        assert!(matches!(auto.poll(), Err(SourceError::NotAvailable(_))));

        // The second game of the list starts.
        acc_h.running.store(true, Ordering::Relaxed);
        auto.connect().unwrap();
        assert_eq!(auto.active_game(), Some(GameId::Acc));
        assert_eq!(auto.info().id, "acc-shm");
        assert_eq!(auto.poll().unwrap().unwrap().session.game, GameId::Acc);

        // It is closed: lost once, released, and looked for again with the others.
        acc_h.running.store(false, Ordering::Relaxed);
        assert!(matches!(auto.poll(), Err(SourceError::NotAvailable(_))));
        assert_eq!(auto.active_game(), None);
        assert_eq!(acc_h.disconnects.load(Ordering::Relaxed), 1);
        assert!(auto.connect().is_err());

        // The other game starts: the same source follows.
        lmu_h.running.store(true, Ordering::Relaxed);
        auto.connect().unwrap();
        assert_eq!(auto.active_game(), Some(GameId::Lmu));
        assert_eq!(auto.poll().unwrap().unwrap().session.game, GameId::Lmu);

        // Both at once: the first of the list.
        acc_h.running.store(true, Ordering::Relaxed);
        auto.connect().unwrap();
        assert_eq!(auto.active_game(), Some(GameId::Lmu));
        assert!(lmu_h.connects.load(Ordering::Relaxed) >= 3);

        auto.disconnect();
        assert_eq!(auto.active_game(), None);
    }
}
