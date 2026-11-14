// SPDX-License-Identifier: GPL-3.0-or-later
//
// L'activité en direct de Corde (écran verrouillé, Dynamic Island) : le compte à rebours du
// prochain départ choisi. Type partagé par l'application (qui la démarre et la termine,
// CordeNativePlugin.swift) et l'extension (qui la dessine, CordeLiveActivity.swift).
// Aucune mise à jour à distance (pas de notification push) : le décompte tourne seul
// (`Text(timerInterval:)`), et iOS la marque « périmée » à l'heure du départ (`staleDate`).

import ActivityKit
import Foundation

struct CordeRaceAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        /// Départ, en millisecondes UTC.
        var startMs: Double

        var start: Date { dateOf(startMs) }
    }

    /// Identifiant du choix (`<race_id>@<start_ms>`).
    var pickId: String
    /// `daily`, `weekly`, `champ`.
    var cat: String
    var level: String?
    var name: String
    var track: String
    /// `GT3 · LMP2`, peut être vide.
    var classes: String
    /// `fr` ou `en`.
    var lang: String
}
