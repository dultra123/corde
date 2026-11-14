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

/// Couleurs de Corde ; en mode clair, des teintes plus foncées (lisibles sur fond blanc).
enum CordeColor {
    static let accent = Color(hex: 0xFFB454)
    static let urgent = Color(hex: 0xF0503C)

    static func accent(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? accent : Color(hex: 0xB05F00)
    }

    static func urgent(_ scheme: ColorScheme) -> Color {
        scheme == .dark ? urgent : Color(hex: 0xC8321F)
    }

    static func cat(_ cat: String, _ scheme: ColorScheme = .dark) -> Color {
        let dark = scheme == .dark
        switch cat {
        case "weekly": return Color(hex: dark ? 0x4EA1E6 : 0x1F6DB5)
        case "special": return Color(hex: dark ? 0xF0503C : 0xC8321F)
        case "champ": return Color(hex: dark ? 0xB982FF : 0x7647C9)
        default: return Color(hex: dark ? 0xF2B63C : 0xA86F00)
        }
    }

    static func level(_ level: String?, _ scheme: ColorScheme = .dark) -> Color {
        let dark = scheme == .dark
        switch level {
        case "intermediate": return Color(hex: dark ? 0xF2B63C : 0xA86F00)
        case "advanced": return Color(hex: dark ? 0xFF5A5A : 0xCC3333)
        default: return Color(hex: dark ? 0x37D08A : 0x16935A)
        }
    }

    /// La couleur d'une course : son niveau pour une quotidienne, sa catégorie sinon.
    static func mark(_ cat: String, _ level: String?, _ scheme: ColorScheme = .dark) -> Color {
        cat == "daily" ? CordeColor.level(level, scheme) : CordeColor.cat(cat, scheme)
    }

    /// Couleur d'un rang SR / DR (la lettre : bronze, argent, or, platine), comme la page Courses.
    static func rating(_ value: String) -> Color {
        switch value.first {
        case "S": return Color(hex: 0xCDD5E1)
        case "G": return Color(hex: 0xFFC62B)
        case "P": return Color(hex: 0x8FE3FF)
        default: return Color(hex: 0xE0A06B)
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
    var slidesTitle: String { t("Courses en images", "Races in pictures") }
    var slidesDescription: String {
        t("Une course par page, avec sa photo et le SR / DR qu'il faut. Change de page chaque minute ; touche › pour la suivante.",
          "One race per page, with its picture and the SR / DR it requires. Turns every minute; tap › for the next one.")
    }
    var nextStarts: String { t("Départs suivants", "Next starts") }
    var nextRace: String { t("Course suivante", "Next race") }
    var previousRace: String { t("Course précédente", "Previous race") }
    var locked: String { t("au-dessus de ton profil", "above your profile") }
    var myRace: String { t("MA COURSE", "MY RACE") }
    var pickInCorde: String { t("Choisis un départ dans Corde", "Pick a start in Corde") }
    var openCorde: String { t("Ouvre Corde pour actualiser", "Open Corde to refresh") }
    var scheduleOff: String { t("Active le planning dans Corde", "Turn the schedule on in Corde") }
    var noRace: String { t("Aucune course à venir", "No race coming up") }
    var started: String { t("Départ donné", "Race started") }
    var inWord: String { t("dans ", "in ") }
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

    /// `sam. 16 nov.`
    func day(_ date: Date) -> String {
        let f = DateFormatter()
        f.locale = locale
        f.timeZone = .current
        f.setLocalizedDateFormatFromTemplate("EEEdMMM")
        return f.string(from: date)
    }

    /// Dates d'un événement spécial : `sam. 16 nov. → dim. 17 nov.` (un seul jour : la date).
    func dateRange(_ from: Date, _ to: Date) -> String {
        Calendar.current.isDate(from, inSameDayAs: to) ? day(from) : "\(day(from)) → \(day(to))"
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

/// Le temps restant en clair, « dans 12 min 5 s », « dans 3 h et 9 min » : il avance seul, et ne
/// se confond pas avec une heure de départ (ce que ferait un décompte `12:05` dans une liste).
struct TimeLeft: View {
    let start: Date
    let now: Date
    let l10n: L10n

    var body: some View {
        if start <= now {
            Text(l10n.started)
        } else {
            Text(l10n.inWord) + Text(start, style: .relative)
        }
    }
}
