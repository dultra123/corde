// SPDX-License-Identifier: GPL-3.0-or-later
//
// Couleurs, textes (fr / en) et formats d'heure des widgets et de l'activité en direct.
// Mêmes couleurs que la page Courses (packages/ui/src/styles/tokens.css, races.css) :
// quotidiennes ambre, hebdomadaires bleu, événements spéciaux rouge, championnats violet.
// Compilé dans les deux cibles.

import SwiftUI

extension Color {
    init(hex: UInt32) {
        self.init(
            red: Double((hex >> 16) & 0xFF) / 255,
            green: Double((hex >> 8) & 0xFF) / 255,
            blue: Double(hex & 0xFF) / 255
        )
    }
}

enum CordeColor {
    static let accent = Color(hex: 0xFFB454)
    static let urgent = Color(hex: 0xF0503C)

    static func cat(_ cat: String) -> Color {
        switch cat {
        case "weekly": return Color(hex: 0x4EA1E6)
        case "special": return Color(hex: 0xF0503C)
        case "champ": return Color(hex: 0xB982FF)
        default: return Color(hex: 0xF2B63C)
        }
    }

    static func level(_ level: String?) -> Color {
        switch level {
        case "intermediate": return Color(hex: 0xF2B63C)
        case "advanced": return Color(hex: 0xFF5A5A)
        default: return Color(hex: 0x37D08A)
        }
    }

    /// Fond des widgets : sombre comme l'application, clair en mode clair.
    static func background(_ scheme: ColorScheme) -> LinearGradient {
        let colors = scheme == .dark
            ? [Color(hex: 0x182029), Color(hex: 0x0B0E12)]
            : [Color(hex: 0xFFFFFF), Color(hex: 0xEDF1F5)]
        return LinearGradient(colors: colors, startPoint: .top, endPoint: .bottom)
    }
}

/// Fond d'un widget, selon le mode clair / sombre du téléphone.
struct CordeBackground: View {
    @Environment(\.colorScheme) private var scheme

    var body: some View {
        CordeColor.background(scheme)
    }
}

/// Les textes des widgets, dans la langue choisie dans Corde (à défaut, celle du téléphone).
struct L10n {
    let fr: Bool

    init(_ lang: String?) {
        if let lang {
            fr = lang != "en"
        } else {
            fr = Locale.preferredLanguages.first?.lowercased().hasPrefix("fr") ?? true
        }
    }

    static var device: L10n { L10n(nil) }

    var locale: Locale { Locale(identifier: fr ? "fr_FR" : "en_GB") }

    private func t(_ fr: String, _ en: String) -> String { self.fr ? fr : en }

    func cat(_ cat: String) -> String {
        switch cat {
        case "weekly": return t("HEBDO", "WEEKLY")
        case "special": return t("SPÉCIAL", "SPECIAL")
        case "champ": return "RC PRO"
        default: return t("QUOTIDIENNE", "DAILY")
        }
    }

    func level(_ level: String?) -> String {
        switch level {
        case "intermediate": return t("Intermédiaire", "Intermediate")
        case "advanced": return t("Avancé", "Advanced")
        case "beginner": return t("Débutant", "Beginner")
        default: return ""
        }
    }

    /// `QUOTIDIENNE · AVANCÉ`, `HEBDO`.
    func kind(_ category: String, _ lvl: String?) -> String {
        let name = category == "daily" ? self.level(lvl) : ""
        let head = self.cat(category)
        return name.isEmpty ? head : "\(head) · \(name.uppercased(with: locale))"
    }

    var racesTitle: String { t("Courses", "Races") }
    var racesDescription: String {
        t("Les prochaines courses en ligne de Le Mans Ultimate.", "The next online races of Le Mans Ultimate.")
    }
    var mineTitle: String { t("Mes courses", "My races") }
    var mineDescription: String {
        t("Le compte à rebours de tes départs choisis.", "The countdown to your picked starts.")
    }
    var nextTitle: String { t("Prochain départ", "Next start") }
    var nextDescription: String {
        t("Ton prochain départ choisi, à défaut la prochaine course.", "Your next picked start, or else the next race.")
    }
    var myRace: String { t("MA COURSE", "MY RACE") }
    var pickInCorde: String { t("Choisis un départ dans Corde", "Pick a start in Corde") }
    var openCorde: String { t("Ouvre Corde pour actualiser", "Open Corde to refresh") }
    var scheduleOff: String { t("Active le planning dans Corde", "Turn the schedule on in Corde") }
    var noRace: String { t("Aucune course à venir", "No race coming up") }
    var started: String { t("Départ donné", "Race started") }
    var startAt: String { t("Départ", "Start") }
    var ongoing: String { t("en cours", "on now") }
    var tomorrow: String { t("demain", "tomorrow") }
    var estimated: String { t("horaire estimé", "estimated time") }
    var countdownTitle: String { t("Compte à rebours", "Countdown") }

    /// `21:25`, dans le fuseau du téléphone.
    func hm(_ date: Date) -> String {
        let f = DateFormatter()
        f.locale = locale
        f.timeZone = .current
        f.dateFormat = "HH:mm"
        return f.string(from: date)
    }

    /// `21:25` aujourd'hui, `demain 09:10`, `sam. 14:00`.
    func when(_ date: Date, now: Date) -> String {
        let cal = Calendar.current
        if cal.isDate(date, inSameDayAs: now) { return hm(date) }
        if let next = cal.date(byAdding: .day, value: 1, to: now), cal.isDate(date, inSameDayAs: next) {
            return "\(tomorrow) \(hm(date))"
        }
        let f = DateFormatter()
        f.locale = locale
        f.timeZone = .current
        f.dateFormat = "EEE"
        return "\(f.string(from: date)) \(hm(date))"
    }
}

/// Le compte à rebours jusqu'à `start`, qui tourne sans rechargement du widget ; « Départ donné »
/// une fois passé (au prochain dessin).
struct Countdown: View {
    let start: Date
    let now: Date
    let l10n: L10n

    var body: some View {
        if start > now {
            Text(timerInterval: now...start, countsDown: true)
                .monospacedDigit()
        } else {
            Text(l10n.started)
        }
    }
}

/// Le temps restant en clair : compte à rebours sous l'heure, « dans 2 h 5 min » au-delà.
struct TimeLeft: View {
    let start: Date
    let now: Date
    let l10n: L10n

    var body: some View {
        if start <= now {
            Text(l10n.started)
        } else if start.timeIntervalSince(now) <= 3600 {
            Text(timerInterval: now...start, countsDown: true)
                .monospacedDigit()
        } else {
            Text(start, style: .relative)
        }
    }
}
