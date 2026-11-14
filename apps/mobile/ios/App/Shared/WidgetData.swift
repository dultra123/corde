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

/// L'exigence d'accès d'une course : SR / DR demandés, et ceux que le profil du pilote
/// n'atteint pas (`miss` : `sr`, `dr` ; jamais les notes du pilote elles-mêmes).
struct WidgetReq: Decodable, Hashable {
    let sr: String
    let dr: String
    let miss: [String]

    enum CodingKeys: String, CodingKey { case sr, dr, miss }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        sr = c.string(.sr)
        dr = c.string(.dr)
        miss = c.lossyArray(.miss)
    }

    init(sr: String = "", dr: String = "", miss: [String] = []) {
        self.sr = sr
        self.dr = dr
        self.miss = miss
    }

    var isEmpty: Bool { sr.isEmpty && dr.isEmpty }
    var srLocked: Bool { !sr.isEmpty && miss.contains("sr") }
    var drLocked: Bool { !dr.isEmpty && miss.contains("dr") }
}

/// Champ facultatif : absent (bloc d'une version précédente de l'application) ou illisible → nil.
private func optionalReq<K: CodingKey>(_ c: KeyedDecodingContainer<K>, _ key: K) -> WidgetReq? {
    let req = (try? c.decodeIfPresent(WidgetReq.self, forKey: key)) ?? nil
    return req?.isEmpty == true ? nil : req
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
    /// Clé de sa photo (gardée à part, SharedStore), facultative.
    let img: String?
    /// Son exigence SR / DR, facultative.
    let req: WidgetReq?

    enum CodingKeys: String, CodingKey { case id, cat, lvl, name, track, cls, dur, s, x, to, img, req }

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
        img = (try? c.decodeIfPresent(String.self, forKey: .img)) ?? nil
        req = optionalReq(c, .req)
    }

    init(id: String, cat: String, lvl: String?, name: String, track: String, cls: [String],
         dur: String, s: [Double], x: [Double] = [], to: Double? = nil, img: String? = nil,
         req: WidgetReq? = nil) {
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
        self.img = img
        self.req = req
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
    let img: String?
    let req: WidgetReq?

    enum CodingKeys: String, CodingKey { case id, race, cat, lvl, name, track, cls, dur, t, img, req }

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
        img = (try? c.decodeIfPresent(String.self, forKey: .img)) ?? nil
        req = optionalReq(c, .req)
    }

    init(id: String, race: String, cat: String, lvl: String?, name: String, track: String,
         cls: [String], dur: String, t: Double, img: String? = nil, req: WidgetReq? = nil) {
        self.id = id
        self.race = race
        self.cat = cat
        self.lvl = lvl
        self.name = name
        self.track = track
        self.cls = cls
        self.dur = dur
        self.t = t
        self.img = img
        self.req = req
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

// MARK: - Widget « Courses en images »

/// Un départ suivant, sur une page du widget « Courses en images ».
struct SlideStart: Hashable {
    let date: Date
    let estimated: Bool
    let picked: Bool
}

/// Une page du widget « Courses en images » : une course, le départ affiché, les suivants.
struct Slide: Hashable {
    let race: WidgetRace
    /// Le départ affiché (événement spécial : son premier jour).
    let start: Date
    let estimated: Bool
    /// Événement spécial en cours.
    let ongoing: Bool
    /// Le départ affiché est un départ choisi (« Ma course »).
    let picked: Bool
    /// Les départs suivants (quatre au plus, connus puis estimés).
    let next: [SlideStart]

    /// Identité de la page pour les transitions (une course, un départ).
    var id: String { "\(race.id)@\(Int64(start.timeIntervalSince1970))" }
}

/// La page affichée et sa place.
struct SlidePage: Hashable {
    let slide: Slide
    let index: Int
    let count: Int
}

extension WidgetPayload {
    /// Pages au plus (miroir de `MAX_SLIDES`, widgets.ts).
    static let maxSlides = 8

    private func isPicked(_ race: WidgetRace, at ms: Double) -> Bool {
        if race.cat == "special" { return mine.contains { $0.race == race.id } }
        return mine.contains { $0.race == race.id && abs($0.t - ms) < 1 }
    }

    private func slide(_ race: WidgetRace, start: Date, estimated: Bool, ongoing: Bool) -> Slide {
        let ms = start.timeIntervalSince1970 * 1000
        var next: [SlideStart] = []
        if race.cat != "special" {
            let known = race.s.filter { $0 > ms }.map { ($0, false) }
            let guessed = race.x.filter { $0 > ms }.map { ($0, true) }
            next = (known + guessed).sorted { $0.0 < $1.0 }.prefix(4).map {
                SlideStart(date: dateOf($0.0), estimated: $0.1, picked: isPicked(race, at: $0.0))
            }
        }
        return Slide(race: race, start: start, estimated: estimated, ongoing: ongoing,
                     picked: isPicked(race, at: ms), next: next)
    }

    /// Les pages à l'instant `date` : la liste du widget « Courses » (`races(at:)`, même ordre,
    /// huit au plus). Petit widget : le prochain départ choisi d'abord (« Ma course »), comme
    /// l'ancien widget du pilote ; sa course n'apparaît pas une seconde fois.
    func slides(at date: Date, small: Bool) -> [Slide] {
        var out: [Slide] = []
        if small, let pick = nextPick(at: date) {
            let race = races.first { $0.id == pick.race } ?? WidgetRace(
                id: pick.race, cat: pick.cat, lvl: pick.lvl, name: pick.name, track: pick.track,
                cls: pick.cls, dur: pick.dur, s: [pick.t], img: pick.img, req: pick.req
            )
            out.append(slide(race, start: pick.start, estimated: false, ongoing: false))
        }
        for item in races(at: date, max: WidgetPayload.maxSlides) where !out.contains(where: { $0.race.id == item.race.id }) {
            out.append(slide(item.race, start: item.start, estimated: item.estimated, ongoing: item.ongoing))
        }
        return Array(out.prefix(WidgetPayload.maxSlides))
    }

    /// La page de la position `position` (qui avance d'une page par créneau, plus les touchers
    /// de « suivant ») : position modulo le nombre de pages.
    func slidePage(at date: Date, small: Bool, position: Int) -> SlidePage? {
        let list = slides(at: date, small: small)
        guard !list.isEmpty else { return nil }
        let i = ((position % list.count) + list.count) % list.count
        return SlidePage(slide: list[i], index: i, count: list.count)
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
