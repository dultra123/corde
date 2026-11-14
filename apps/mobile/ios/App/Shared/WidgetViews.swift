// SPDX-License-Identifier: GPL-3.0-or-later
//
// Les vues des widgets, sans rien de WidgetKit autour (fond, lien, familles) : l'extension les
// enveloppe (CordeWidgets.swift), et l'application en build Debug les dessine en images pour la
// CI (WidgetShots.swift). D'où la famille passée en paramètre plutôt que lue dans
// l'environnement. Compilé dans les deux cibles.

import SwiftUI
import WidgetKit

// MARK: - Morceaux communs

/// Pastille de catégorie : `● QUOTIDIENNE · AVANCÉ`.
struct KindBadge: View {
    let cat: String
    let level: String?
    let l10n: L10n
    var small = false

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(cat == "daily" ? CordeColor.level(level) : CordeColor.cat(cat))
                .frame(width: small ? 6 : 7, height: small ? 6 : 7)
            Text(l10n.kind(cat, level))
                .font(.system(size: small ? 9 : 10, weight: .heavy))
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(CordeColor.cat(cat).opacity(0.18), in: Capsule())
        .foregroundStyle(CordeColor.cat(cat))
    }
}

/// `Monza · GT3 · LMP2`.
func trackLine(_ track: String, _ classes: [String]) -> String {
    ([track] + classes.prefix(3)).filter { !$0.isEmpty }.joined(separator: " · ")
}

/// Un message à la place du contenu (planning éteint, rien à venir, ouvrir Corde).
struct WidgetMessage: View {
    let title: String
    let message: String
    var symbol = "flag.checkered"

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 5) {
                Image(systemName: symbol)
                    .font(.system(size: 12, weight: .bold))
                    .foregroundStyle(CordeColor.accent)
                Text(title)
                    .font(.system(size: 13, weight: .heavy))
            }
            Spacer(minLength: 0)
            Text(message)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(.secondary)
                .lineLimit(3)
                .minimumScaleFactor(0.8)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// Une ligne de liste : barre de couleur, nom, circuit et classes, heure et temps restant.
struct RaceRow: View {
    let cat: String
    let level: String?
    let name: String
    let detail: String
    let start: Date
    let now: Date
    let estimated: Bool
    let ongoing: Bool
    let l10n: L10n

    var body: some View {
        HStack(spacing: 8) {
            RoundedRectangle(cornerRadius: 2)
                .fill(cat == "daily" ? CordeColor.level(level) : CordeColor.cat(cat))
                .frame(width: 4)
            VStack(alignment: .leading, spacing: 1) {
                Text(name)
                    .font(.system(size: 13, weight: .bold))
                    .lineLimit(1)
                Text(detail)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 4)
            VStack(alignment: .trailing, spacing: 1) {
                if ongoing {
                    Text(l10n.ongoing)
                        .font(.system(size: 12, weight: .bold))
                        .foregroundStyle(CordeColor.cat(cat))
                } else {
                    Text((estimated ? "≈ " : "") + l10n.when(start, now: now))
                        .font(.system(size: 13, weight: .bold))
                        .monospacedDigit()
                        .lineLimit(1)
                    TimeLeft(start: start, now: now, l10n: l10n)
                        .font(.system(size: 10.5))
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.trailing)
                        .lineLimit(1)
                        .frame(maxWidth: 92, alignment: .trailing)
                }
            }
            .layoutPriority(1)
        }
        .frame(maxHeight: 34)
    }
}

// MARK: - Widget « Courses »

/// Ce que montre un widget quand il n'y a pas de bloc utilisable, `nil` sinon.
private func blockMessage(_ load: WidgetLoad, at date: Date, l10n: L10n) -> String? {
    guard let p = load.payload else { return l10n.openCorde }
    if p.isStale(at: date) { return l10n.openCorde }
    return nil
}

struct RacesWidgetView: View {
    let family: WidgetFamily
    let date: Date
    let load: WidgetLoad

    private var l10n: L10n { L10n(load.payload?.lang) }

    var body: some View {
        content
            .environment(\.locale, l10n.locale)
    }

    @ViewBuilder
    private var content: some View {
        if let message = blockMessage(load, at: date, l10n: l10n) {
            WidgetMessage(title: l10n.racesTitle, message: message)
        } else if let p = load.payload, p.sched == "off" {
            WidgetMessage(title: l10n.racesTitle, message: l10n.scheduleOff)
        } else if let p = load.payload {
            let count = family == .systemLarge ? 6 : family == .systemMedium ? 3 : 1
            let list = p.races(at: date, max: count)
            if list.isEmpty {
                WidgetMessage(title: l10n.racesTitle, message: p.sched == "none" ? l10n.openCorde : l10n.noRace)
            } else if family == .systemSmall, let first = list.first {
                SmallRace(item: first, now: date, l10n: l10n)
            } else {
                RaceList(title: l10n.racesTitle, items: list, now: date, l10n: l10n, roomy: family == .systemLarge)
            }
        }
    }
}

/// Petit widget : la prochaine course, en grand.
private struct SmallRace: View {
    let item: ListedRace
    let now: Date
    let l10n: L10n

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            KindBadge(cat: item.race.cat, level: item.race.lvl, l10n: l10n, small: true)
            Spacer(minLength: 2)
            Text(item.race.name)
                .font(.system(size: 16, weight: .heavy))
                .lineLimit(2)
                .minimumScaleFactor(0.75)
            Text(trackLine(item.race.track, item.race.cls))
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            Spacer(minLength: 2)
            HStack(alignment: .firstTextBaseline) {
                if item.ongoing {
                    Text(l10n.ongoing)
                        .font(.system(size: 18, weight: .heavy))
                        .foregroundStyle(CordeColor.cat(item.race.cat))
                } else {
                    Text((item.estimated ? "≈ " : "") + l10n.when(item.start, now: now))
                        .font(.system(size: 18, weight: .heavy))
                        .monospacedDigit()
                        .lineLimit(1)
                        .minimumScaleFactor(0.6)
                    Spacer(minLength: 2)
                    TimeLeft(start: item.start, now: now, l10n: l10n)
                        .font(.system(size: 11, weight: .semibold))
                        .foregroundStyle(CordeColor.accent)
                        .multilineTextAlignment(.trailing)
                        .lineLimit(1)
                        .frame(maxWidth: 70, alignment: .trailing)
                }
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

/// Moyen et grand widgets : titre, puis 3 ou 6 courses.
private struct RaceList: View {
    let title: String
    let items: [ListedRace]
    let now: Date
    let l10n: L10n
    let roomy: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: roomy ? 9 : 6) {
            HStack(spacing: 5) {
                Image(systemName: "flag.checkered")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(CordeColor.accent)
                Text(title)
                    .font(.system(size: 12, weight: .heavy))
                Spacer()
                Text("Corde")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            ForEach(items, id: \.self) { item in
                RaceRow(
                    cat: item.race.cat,
                    level: item.race.lvl,
                    name: item.race.name,
                    detail: trackLine(item.race.track, item.race.cls),
                    start: item.start,
                    now: now,
                    estimated: item.estimated,
                    ongoing: item.ongoing,
                    l10n: l10n
                )
            }
            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

// MARK: - Widget « Mes courses »

struct MyRacesWidgetView: View {
    let family: WidgetFamily
    let date: Date
    let load: WidgetLoad

    private var l10n: L10n { L10n(load.payload?.lang) }

    var body: some View {
        content
            .environment(\.locale, l10n.locale)
    }

    @ViewBuilder
    private var content: some View {
        if let message = blockMessage(load, at: date, l10n: l10n) {
            WidgetMessage(title: l10n.mineTitle, message: message, symbol: "timer")
        } else if let p = load.payload, let next = p.nextPick(at: date) {
            if family == .systemSmall {
                SmallPick(pick: next, now: date, l10n: l10n)
            } else {
                HStack(spacing: 12) {
                    SmallPick(pick: next, now: date, l10n: l10n)
                        .frame(maxWidth: .infinity)
                    let rest = Array(p.picks(at: date).filter { $0.id != next.id }.prefix(3))
                    VStack(alignment: .leading, spacing: 6) {
                        if rest.isEmpty {
                            Spacer(minLength: 0)
                            Text(l10n.pickInCorde)
                                .font(.system(size: 12, weight: .medium))
                                .foregroundStyle(.secondary)
                            Spacer(minLength: 0)
                        } else {
                            ForEach(rest, id: \.self) { pick in
                                RaceRow(
                                    cat: pick.cat,
                                    level: pick.lvl,
                                    name: pick.name,
                                    detail: trackLine(pick.track, pick.cls),
                                    start: pick.start,
                                    now: date,
                                    estimated: false,
                                    ongoing: false,
                                    l10n: l10n
                                )
                            }
                            Spacer(minLength: 0)
                        }
                    }
                    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                }
            }
        } else {
            WidgetMessage(title: l10n.mineTitle, message: l10n.pickInCorde, symbol: "timer")
        }
    }
}

/// Le prochain départ choisi : nom, circuit, gros compte à rebours.
private struct SmallPick: View {
    let pick: WidgetPick
    let now: Date
    let l10n: L10n

    var body: some View {
        VStack(alignment: .leading, spacing: 3) {
            HStack(spacing: 4) {
                Image(systemName: "timer")
                    .font(.system(size: 9, weight: .heavy))
                Text(l10n.myRace)
                    .font(.system(size: 10, weight: .heavy))
            }
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .background(CordeColor.accent, in: Capsule())
            .foregroundStyle(Color(hex: 0x14110A))
            Spacer(minLength: 2)
            Text(pick.name)
                .font(.system(size: 15, weight: .heavy))
                .lineLimit(2)
                .minimumScaleFactor(0.75)
            Text(trackLine(pick.track, pick.cls))
                .font(.system(size: 11, weight: .medium))
                .foregroundStyle(.secondary)
                .lineLimit(1)
                .minimumScaleFactor(0.8)
            Spacer(minLength: 2)
            Countdown(start: pick.start, now: now, l10n: l10n)
                .font(.system(size: 24, weight: .heavy))
                .foregroundStyle(pick.start.timeIntervalSince(now) <= 300 ? CordeColor.urgent : CordeColor.accent)
                .lineLimit(1)
                .minimumScaleFactor(0.6)
            Text("\(l10n.startAt) \(l10n.when(pick.start, now: now))")
                .font(.system(size: 10.5, weight: .semibold))
                .foregroundStyle(.secondary)
                .lineLimit(1)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }
}

// MARK: - Écran verrouillé

/// Ce que suit l'écran verrouillé : le prochain départ choisi, à défaut la prochaine course.
struct LockTarget {
    let name: String
    let track: String
    let start: Date
    let picked: Bool

    static func of(_ load: WidgetLoad, at date: Date) -> LockTarget? {
        guard let p = load.payload, !p.isStale(at: date) else { return nil }
        if let pick = p.nextPick(at: date) {
            return LockTarget(name: pick.name, track: pick.track, start: pick.start, picked: true)
        }
        if let race = p.nextRace(at: date) {
            return LockTarget(name: race.race.name, track: race.race.track, start: race.start, picked: false)
        }
        return nil
    }
}

struct LockRectangularView: View {
    let date: Date
    let load: WidgetLoad

    var body: some View {
        let l10n = L10n(load.payload?.lang)
        if let target = LockTarget.of(load, at: date) {
            VStack(alignment: .leading, spacing: 0) {
                HStack(spacing: 4) {
                    Image(systemName: target.picked ? "timer" : "flag.checkered")
                        .font(.system(size: 11, weight: .bold))
                    Text(target.name)
                        .font(.system(size: 14, weight: .bold))
                        .lineLimit(1)
                }
                Text("\(target.track) · \(l10n.hm(target.start))")
                    .font(.system(size: 12))
                    .lineLimit(1)
                    .opacity(0.8)
                Countdown(start: target.start, now: date, l10n: l10n)
                    .font(.system(size: 15, weight: .heavy))
                    .lineLimit(1)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .environment(\.locale, l10n.locale)
        } else {
            VStack(alignment: .leading, spacing: 1) {
                HStack(spacing: 4) {
                    Image(systemName: "flag.checkered")
                        .font(.system(size: 11, weight: .bold))
                    Text("Corde")
                        .font(.system(size: 14, weight: .bold))
                }
                Text(load.payload == nil || load.payload?.isStale(at: date) == true ? l10n.openCorde : l10n.noRace)
                    .font(.system(size: 12))
                    .lineLimit(2)
                    .opacity(0.8)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
        }
    }
}

struct LockInlineView: View {
    let date: Date
    let load: WidgetLoad

    var body: some View {
        let l10n = L10n(load.payload?.lang)
        if let target = LockTarget.of(load, at: date) {
            Text("\(Image(systemName: "flag.checkered")) \(l10n.hm(target.start)) · \(target.name)")
        } else {
            Text("\(Image(systemName: "flag.checkered")) \(l10n.openCorde)")
        }
    }
}

struct LockCircularView: View {
    let date: Date
    let load: WidgetLoad

    var body: some View {
        let l10n = L10n(load.payload?.lang)
        let target = LockTarget.of(load, at: date)
        VStack(spacing: 1) {
            Image(systemName: target?.picked == true ? "timer" : "flag.checkered")
                .font(.system(size: 13, weight: .bold))
            Text(target.map { l10n.hm($0.start) } ?? "--:--")
                .font(.system(size: 13, weight: .heavy))
                .monospacedDigit()
                .minimumScaleFactor(0.7)
        }
    }
}
