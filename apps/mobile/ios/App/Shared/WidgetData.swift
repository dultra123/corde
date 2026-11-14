// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le bloc des widgets (format version 1), écrit par l'application (packages/ui/src/mobile/
// widgets.ts, `buildPayload`) et lu ici. Décodage tolérant : un champ inconnu est ignoré, un
// élément illisible d'une liste est sauté, une autre version donne l'état « Ouvre Corde ».
// Les heures sont en millisecondes UTC ; l'affichage se fait dans le fuseau du téléphone.
// Compilé dans les deux cibles. Format décrit dans docs/IOS.md.

import Foundation
import Security

/// Un élément de liste qui ne se décode pas est remplacé par `nil` (puis sauté).
private struct Lossy<T: Decodable>: Decodable {
    let value: T?
    init(from decoder: Decoder) throws {
        value = try? T(from: decoder)
    }
}

private extension KeyedDecodingContainer {
    func lossyArray<T: Decodable>(_ key: Key) -> [T] {
        ((try? decodeIfPresent([Lossy<T>].self, forKey: key)) ?? nil)?.compactMap { $0.value } ?? []
    }

    func string(_ key: Key, _ fallback: String = "") -> String {
        ((try? decodeIfPresent(String.self, forKey: key)) ?? nil) ?? fallback
    }

    func number(_ key: Key) -> Double? {
        (try? decodeIfPresent(Double.self, forKey: key)) ?? nil
    }
}

/// Instant en millisecondes depuis 1970 (UTC).
func dateOf(_ ms: Double) -> Date {
    Date(timeIntervalSince1970: ms / 1000)
}

struct WidgetRace: Decodable, Hashable {
    let id: String
    /// `daily`, `weekly`, `special`, `champ`.
    let cat: String
    let lvl: String?
    let name: String
    let track: String
    let cls: [String]
    let dur: String
    /// Départs connus (événement spécial : premier jour 00:00).
    let s: [Double]
    /// Départs estimés d'une quotidienne.
    let x: [Double]
    /// Événement spécial : dernier jour 23:59.
    let to: Double?

    enum CodingKeys: String, CodingKey { case id, cat, lvl, name, track, cls, dur, s, x, to }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        cat = c.string(.cat, "daily")
        lvl = (try? c.decodeIfPresent(String.self, forKey: .lvl)) ?? nil
        name = c.string(.name)
        track = c.string(.track)
        cls = c.lossyArray(.cls)
        dur = c.string(.dur)
        s = c.lossyArray(.s)
        x = c.lossyArray(.x)
        to = c.number(.to)
    }

    init(id: String, cat: String, lvl: String?, name: String, track: String, cls: [String],
         dur: String, s: [Double], x: [Double] = [], to: Double? = nil) {
        self.id = id
        self.cat = cat
        self.lvl = lvl
        self.name = name
        self.track = track
        self.cls = cls
        self.dur = dur
        self.s = s
        self.x = x
        self.to = to
    }
}

struct WidgetPick: Decodable, Hashable {
    let id: String
    let race: String
    let cat: String
    let lvl: String?
    let name: String
    let track: String
    let cls: [String]
    let dur: String
    /// Départ (événement spécial : son rappel du jour, 10:00).
    let t: Double

    enum CodingKeys: String, CodingKey { case id, race, cat, lvl, name, track, cls, dur, t }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        id = try c.decode(String.self, forKey: .id)
        race = c.string(.race)
        cat = c.string(.cat, "daily")
        lvl = (try? c.decodeIfPresent(String.self, forKey: .lvl)) ?? nil
        name = c.string(.name)
        track = c.string(.track)
        cls = c.lossyArray(.cls)
        dur = c.string(.dur)
        t = try c.decode(Double.self, forKey: .t)
    }

    init(id: String, race: String, cat: String, lvl: String?, name: String, track: String,
         cls: [String], dur: String, t: Double) {
        self.id = id
        self.race = race
        self.cat = cat
        self.lvl = lvl
        self.name = name
        self.track = track
        self.cls = cls
        self.dur = dur
        self.t = t
    }

    var start: Date { dateOf(t) }
}

struct WidgetPayload: Decodable {
    static let version = 1

    let v: Int
    let at: Double
    let lang: String
    /// `on`, `off` (planning désactivé), `none` (rien pu lire).
    let sched: String
    let until: Double
    let races: [WidgetRace]
    let mine: [WidgetPick]

    enum CodingKeys: String, CodingKey { case v, at, lang, sched, until, races, mine }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        v = try c.decode(Int.self, forKey: .v)
        at = c.number(.at) ?? 0
        lang = c.string(.lang, "fr")
        sched = c.string(.sched, "none")
        until = c.number(.until) ?? 0
        races = c.lossyArray(.races)
        mine = c.lossyArray(.mine)
    }

    init(at: Double, lang: String, sched: String, until: Double, races: [WidgetRace], mine: [WidgetPick]) {
        v = WidgetPayload.version
        self.at = at
        self.lang = lang
        self.sched = sched
        self.until = until
        self.races = races
        self.mine = mine
    }
}

/// Une course de la liste du widget « Courses ».
struct ListedRace: Hashable {
    let race: WidgetRace
    let start: Date
    let estimated: Bool
    let ongoing: Bool
}

private let catOrder = ["daily", "weekly", "special", "champ"]

private func catRank(_ cat: String) -> Int {
    catOrder.firstIndex(of: cat) ?? catOrder.count
}

extension WidgetPayload {
    /// Le bloc a plus de 4 jours, ou tout ce qu'il annonçait est passé (`until`, 0 quand il
    /// n'annonçait rien) : « Ouvre Corde pour actualiser ».
    func isStale(at date: Date) -> Bool {
        let now = date.timeIntervalSince1970 * 1000
        return now - at > 4 * 86_400_000 || (until > 0 && now > until)
    }

    /// La liste du widget « Courses » à l'instant `date`. Miroir de `racesAt` (widgets.ts) :
    /// chaque course une fois, à son prochain départ, par départ ; les événements spéciaux en
    /// cours à la fin.
    func races(at date: Date, max: Int) -> [ListedRace] {
        let t = date.timeIntervalSince1970 * 1000
        var next: [ListedRace] = []
        var ongoing: [ListedRace] = []
        for race in races {
            if race.cat == "special" {
                guard let from = race.s.first else { continue }
                if from > t {
                    next.append(ListedRace(race: race, start: dateOf(from), estimated: false, ongoing: false))
                } else if (race.to ?? 0) > t {
                    ongoing.append(ListedRace(race: race, start: dateOf(from), estimated: false, ongoing: true))
                }
                continue
            }
            if let known = race.s.first(where: { $0 > t }) {
                next.append(ListedRace(race: race, start: dateOf(known), estimated: false, ongoing: false))
            } else if let guess = race.x.first(where: { $0 > t }) {
                next.append(ListedRace(race: race, start: dateOf(guess), estimated: true, ongoing: false))
            }
        }
        next.sort { a, b in
            if a.start != b.start { return a.start < b.start }
            if catRank(a.race.cat) != catRank(b.race.cat) { return catRank(a.race.cat) < catRank(b.race.cat) }
            return a.race.id < b.race.id
        }
        return Array((next + ongoing).prefix(max))
    }

    /// Les choix encore à venir à l'instant `date`, par départ (miroir de `picksAt`).
    func picks(at date: Date) -> [WidgetPick] {
        let t = date.timeIntervalSince1970 * 1000
        return mine.filter { $0.t > t }
    }

    /// Le prochain départ choisi (pas le rappel d'un événement spécial).
    func nextPick(at date: Date) -> WidgetPick? {
        picks(at: date).first { $0.cat != "special" }
    }

    /// La prochaine course du planning (pas un événement spécial), pour l'écran verrouillé.
    func nextRace(at date: Date) -> ListedRace? {
        races(at: date, max: 50).first { $0.race.cat != "special" && !$0.ongoing }
    }

    /// Les instants où un widget doit se redessiner après `date` : chaque départ (connu ou
    /// estimé), chaque départ choisi et l'heure qui le précède (passage au compte à rebours).
    func changeDates(after date: Date, limit: Int) -> [Date] {
        let t = date.timeIntervalSince1970 * 1000
        var all = Set<Double>()
        for race in races {
            for s in race.s where s > t { all.insert(s) }
            for s in race.x where s > t { all.insert(s) }
            if let to = race.to, to > t { all.insert(to) }
        }
        for p in mine {
            if p.t > t { all.insert(p.t) }
            if p.t - 3_600_000 > t { all.insert(p.t - 3_600_000) }
        }
        if until > 0 { all.insert(until) }
        return all.filter { $0 > t }.sorted().prefix(limit).map(dateOf)
    }
}

/// Ce que les widgets lisent : le bloc, ou pourquoi il n'y en a pas.
enum WidgetLoad {
    case ready(WidgetPayload)
    /// Rien d'écrit encore (Corde jamais ouverte depuis l'installation des widgets).
    case empty
    /// Illisible, autre version, ou trousseau refusé : « Ouvre Corde pour actualiser ».
    case unreadable

    var payload: WidgetPayload? {
        if case let .ready(p) = self { return p }
        return nil
    }

    static func decode(_ data: Data) -> WidgetLoad {
        struct Head: Decodable { let v: Int }
        guard let head = try? JSONDecoder().decode(Head.self, from: data),
              head.v == WidgetPayload.version,
              let payload = try? JSONDecoder().decode(WidgetPayload.self, from: data)
        else { return .unreadable }
        return .ready(payload)
    }

    static func load() -> WidgetLoad {
        let r = SharedStore.read()
        if let data = r.data { return decode(data) }
        return r.status == errSecItemNotFound ? .empty : .unreadable
    }
}
