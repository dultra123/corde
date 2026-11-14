// SPDX-License-Identifier: GPL-3.0-or-later
//
// Les vues de l'activité en direct (écran verrouillé, Dynamic Island), sans ActivityKit autour :
// l'extension les place (CordeLiveActivity.swift), l'application en build Debug les dessine en
// images pour la CI (WidgetShots.swift). `stale` vient de `context.isStale` : vrai à partir de
// l'heure du départ (`staleDate`), iOS redessine alors l'activité (« Départ donné »).
// Compilé dans les deux cibles.

import SwiftUI

/// Ce que montre l'activité, quelle que soit sa forme.
struct ActivityModel {
    let attributes: CordeRaceAttributes
    let start: Date
    let stale: Bool

    var l10n: L10n { L10n(attributes.lang) }
    var color: Color { CordeColor.cat(attributes.cat) }

    /// Le décompte n'a de sens qu'avant le départ.
    func running(now: Date) -> Bool { !stale && start > now }

    var detail: String {
        [attributes.track, attributes.classes].filter { !$0.isEmpty }.joined(separator: " · ")
    }
}

/// Le décompte de l'activité : `12:34`, puis « Départ donné ».
struct ActivityTimer: View {
    let model: ActivityModel
    let now: Date
    var size: CGFloat = 28

    var body: some View {
        if model.running(now: now) {
            Text(timerInterval: now...model.start, countsDown: true)
                .font(.system(size: size, weight: .heavy))
                .monospacedDigit()
                .multilineTextAlignment(.trailing)
                .foregroundStyle(model.start.timeIntervalSince(now) <= 300 ? CordeColor.urgent : CordeColor.accent)
        } else {
            Text(model.l10n.started)
                .font(.system(size: size * 0.6, weight: .heavy))
                .foregroundStyle(CordeColor.urgent)
                .multilineTextAlignment(.trailing)
        }
    }
}

/// Écran verrouillé (et bannière des iPhone sans Dynamic Island).
struct ActivityLockView: View {
    let model: ActivityModel
    let now: Date

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 4) {
                KindBadge(cat: model.attributes.cat, level: model.attributes.level, l10n: model.l10n, small: true)
                Text(model.attributes.name)
                    .font(.system(size: 17, weight: .heavy))
                    .lineLimit(1)
                    .minimumScaleFactor(0.75)
                Text(model.detail)
                    .font(.system(size: 13))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
            }
            Spacer(minLength: 6)
            VStack(alignment: .trailing, spacing: 2) {
                ActivityTimer(model: model, now: now, size: 30)
                    .lineLimit(1)
                    .frame(maxWidth: 130, alignment: .trailing)
                Text("\(model.l10n.startAt) \(model.l10n.hm(model.start))")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
            .layoutPriority(1)
        }
        .padding(16)
        .environment(\.locale, model.l10n.locale)
    }
}

/// Dynamic Island déployée : les quatre régions (à gauche et à droite de la caméra : le drapeau
/// et le décompte ; au centre : le nom ; en bas : circuit, classes, catégorie, heure).
struct IslandLeading: View {
    let model: ActivityModel

    var body: some View {
        Image(systemName: "flag.checkered")
            .font(.system(size: 18, weight: .bold))
            .foregroundStyle(model.color)
            .padding(.leading, 6)
    }
}

struct IslandTrailing: View {
    let model: ActivityModel
    let now: Date

    var body: some View {
        ActivityTimer(model: model, now: now, size: 22)
            .lineLimit(1)
            .frame(maxWidth: 110, alignment: .trailing)
            .padding(.trailing, 4)
    }
}

struct IslandCenter: View {
    let model: ActivityModel

    var body: some View {
        Text(model.attributes.name)
            .font(.system(size: 16, weight: .heavy))
            .foregroundStyle(.white)
            .lineLimit(1)
            .minimumScaleFactor(0.75)
    }
}

struct IslandBottom: View {
    let model: ActivityModel

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(model.detail)
                    .lineLimit(1)
                Spacer(minLength: 6)
                Text("\(model.l10n.startAt) \(model.l10n.hm(model.start))")
                    .monospacedDigit()
            }
            .font(.system(size: 13, weight: .semibold))
            .foregroundStyle(.white.opacity(0.75))
            Text(model.l10n.kind(model.attributes.cat, model.attributes.level))
                .font(.system(size: 10, weight: .heavy))
                .foregroundStyle(model.color)
                .lineLimit(1)
        }
        .padding(.horizontal, 6)
    }
}

/// Dynamic Island compacte : drapeau à gauche, décompte à droite.
struct IslandCompactLeading: View {
    let model: ActivityModel

    var body: some View {
        Image(systemName: "flag.checkered")
            .font(.system(size: 13, weight: .bold))
            .foregroundStyle(model.color)
    }
}

struct IslandCompactTrailing: View {
    let model: ActivityModel
    let now: Date

    var body: some View {
        if model.running(now: now) {
            Text(timerInterval: now...model.start, countsDown: true)
                .font(.system(size: 14, weight: .bold))
                .monospacedDigit()
                .multilineTextAlignment(.trailing)
                .frame(maxWidth: 56)
                .foregroundStyle(CordeColor.accent)
        } else {
            Image(systemName: "flag.checkered.2.crossed")
                .font(.system(size: 13, weight: .bold))
                .foregroundStyle(CordeColor.urgent)
        }
    }
}

/// Dynamic Island minimale (une autre application occupe l'île) : la pastille seule.
struct IslandMinimal: View {
    let model: ActivityModel
    let now: Date

    var body: some View {
        Image(systemName: model.running(now: now) ? "timer" : "flag.checkered.2.crossed")
            .font(.system(size: 13, weight: .bold))
            .foregroundStyle(model.running(now: now) ? model.color : CordeColor.urgent)
    }
}
