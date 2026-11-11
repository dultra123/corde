//! Détection des fronts dans l'état du jeu (`GET /api/status`, une fois par seconde).
//!
//! Sans dépendance à Tauri : c'est la partie du veilleur qui se teste seule.

use crate::server::GameStatus;

/// Ce que le veilleur doit faire après une nouvelle lecture de l'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    /// Rien de nouveau.
    None,
    /// Le jeu vient d'être détecté (front montant de `connected`).
    GameDetected,
    /// Le jeu a disparu (front descendant de `connected`).
    GameLost,
    /// Le pilote vient d'entrer en piste (front montant de `in_realtime`).
    EnteredTrack,
    /// Le pilote a quitté la piste (retour au garage, menus).
    LeftTrack,
}

/// Mémoire du dernier état lu. Démarre à « pas de jeu, pas en piste ».
#[derive(Debug, Default)]
pub struct EdgeDetector {
    last: GameStatus,
}

impl EdgeDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Compare la nouvelle lecture à la précédente.
    ///
    /// Un seul événement par lecture ; l'entrée en piste est prioritaire (quand le jeu est
    /// détecté alors que le pilote roule déjà, c'est l'entrée en piste qui compte). Chaque
    /// entrée en piste ne produit `EnteredTrack` qu'une fois : il faut repasser par
    /// `in_realtime = false` pour en obtenir un autre.
    pub fn update(&mut self, now: GameStatus) -> Edge {
        let before = std::mem::replace(&mut self.last, now);
        if now.in_realtime && !before.in_realtime {
            Edge::EnteredTrack
        } else if now.connected && !before.connected {
            Edge::GameDetected
        } else if !now.connected && before.connected {
            Edge::GameLost
        } else if !now.in_realtime && before.in_realtime {
            Edge::LeftTrack
        } else {
            Edge::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(connected: bool, in_realtime: bool) -> GameStatus {
        GameStatus {
            connected,
            in_realtime,
        }
    }

    #[test]
    fn one_event_per_track_entry() {
        let mut d = EdgeDetector::new();
        assert_eq!(d.update(s(false, false)), Edge::None);
        assert_eq!(d.update(s(true, false)), Edge::GameDetected);
        assert_eq!(d.update(s(true, false)), Edge::None);
        assert_eq!(d.update(s(true, true)), Edge::EnteredTrack);
        // Tant que le pilote roule : plus rien.
        for _ in 0..10 {
            assert_eq!(d.update(s(true, true)), Edge::None);
        }
        // Retour au garage puis nouvelle sortie : un nouvel événement, un seul.
        assert_eq!(d.update(s(true, false)), Edge::LeftTrack);
        assert_eq!(d.update(s(true, true)), Edge::EnteredTrack);
        assert_eq!(d.update(s(true, true)), Edge::None);
        assert_eq!(d.update(s(false, false)), Edge::GameLost);
        assert_eq!(d.update(s(false, false)), Edge::None);
    }

    #[test]
    fn entering_track_wins_over_game_detected() {
        // Corde lancé alors que le pilote roule déjà.
        let mut d = EdgeDetector::new();
        assert_eq!(d.update(s(true, true)), Edge::EnteredTrack);
        assert_eq!(d.update(s(true, true)), Edge::None);
    }
}
