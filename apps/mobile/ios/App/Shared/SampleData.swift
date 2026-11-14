// SPDX-License-Identifier: GPL-3.0-or-later
//
// Données d'exemple, inventées : la galerie des widgets (avant que Corde ait écrit quoi que ce
// soit) et les images des vues dessinées par la CI (WidgetShots.swift). Aucun nom réel.
// Compilé dans les deux cibles.

import Foundation

enum SampleData {
    /// Clés des photos d'exemple (fabriquées par le code dans WidgetShots.swift ; dans la galerie
    /// des widgets, aucune photo n'existe sous ces clés : le fond dessiné s'affiche).
    static let sunsetKey = "i5a3b1e0000000001"
    static let brightKey = "i5a3b1e0000000002"
    static let nightKey = "i5a3b1e0000000003"

    /// Un planning et deux départs choisis, à partir de `now`.
    static func payload(now: Date, lang: String?, picks: Bool = true) -> WidgetPayload {
        let t = now.timeIntervalSince1970 * 1000
        let min = 60_000.0
        let hour = 3_600_000.0
        let lmp2Req = WidgetReq(sr: "S1", dr: "B3", miss: ["dr"])
        let weeklyReq = WidgetReq(sr: "S2", dr: "B2")
        let races = [
            WidgetRace(id: "sprint-gt3-beginner", cat: "daily", lvl: "beginner", name: "Sprint GT3",
                       track: "Monza", cls: ["GT3"], dur: "20 min",
                       s: [t + 12 * min, t + 42 * min, t + 72 * min], x: [t + 102 * min],
                       img: sunsetKey, req: WidgetReq(sr: "B2")),
            WidgetRace(id: "lmp2-cup-intermediate", cat: "daily", lvl: "intermediate", name: "Coupe LMP2",
                       track: "Spa-Francorchamps", cls: ["LMP2"], dur: "30 min",
                       s: [t + 27 * min, t + 87 * min], x: [t + 147 * min], img: brightKey, req: lmp2Req),
            WidgetRace(id: "hyper-advanced", cat: "daily", lvl: "advanced", name: "Hypercar Sprint",
                       track: "Fuji", cls: ["Hypercar"], dur: "40 min",
                       s: [t + 50 * min], img: nightKey, req: WidgetReq(sr: "S2", dr: "S0", miss: ["sr", "dr"])),
            WidgetRace(id: "weekly-endurance", cat: "weekly", lvl: nil, name: "Endurance de la semaine",
                       track: "Le Mans", cls: ["Hypercar", "LMP2", "GT3"], dur: "1 h 30",
                       s: [t + 3 * hour + 10 * min], req: weeklyReq),
            WidgetRace(id: "champ-gt3-pro", cat: "champ", lvl: nil, name: "Championnat GT3",
                       track: "Imola", cls: ["GT3"], dur: "45 min",
                       s: [t + 26 * hour], img: brightKey, req: WidgetReq(sr: "G1", miss: ["sr"])),
            WidgetRace(id: "special-6h", cat: "special", lvl: nil, name: "6 Heures d'exemple",
                       track: "Bahreïn (circuit international)", cls: ["Hypercar", "LMP2"], dur: "6 h",
                       s: [t + 2 * 24 * hour], to: t + 3 * 24 * hour),
        ]
        let mine = picks ? [
            WidgetPick(id: "lmp2-cup-intermediate@\(Int(t + 27 * min))", race: "lmp2-cup-intermediate",
                       cat: "daily", lvl: "intermediate", name: "Coupe LMP2", track: "Spa-Francorchamps",
                       cls: ["LMP2"], dur: "30 min", t: t + 27 * min, img: brightKey, req: lmp2Req),
            WidgetPick(id: "lmp2-cup-intermediate@\(Int(t + 147 * min))", race: "lmp2-cup-intermediate",
                       cat: "daily", lvl: "intermediate", name: "Coupe LMP2", track: "Spa-Francorchamps",
                       cls: ["LMP2"], dur: "30 min", t: t + 147 * min, img: brightKey, req: lmp2Req),
            WidgetPick(id: "weekly-endurance@\(Int(t + 3 * hour + 10 * min))", race: "weekly-endurance",
                       cat: "weekly", lvl: nil, name: "Endurance de la semaine", track: "Le Mans",
                       cls: ["Hypercar", "LMP2", "GT3"], dur: "1 h 30", t: t + 3 * hour + 10 * min, req: weeklyReq),
        ] : []
        let lang = lang ?? (L10n.device.fr ? "fr" : "en")
        return WidgetPayload(at: t, lang: lang, sched: "on", until: t + 3 * 24 * hour, races: races, mine: mine)
    }

    /// L'activité en direct d'exemple.
    static func activity(lang: String) -> CordeRaceAttributes {
        CordeRaceAttributes(pickId: "lmp2-cup-intermediate@0", cat: "daily", level: "intermediate",
                            name: "Coupe LMP2", track: "Spa-Francorchamps", classes: "LMP2", lang: lang)
    }
}
