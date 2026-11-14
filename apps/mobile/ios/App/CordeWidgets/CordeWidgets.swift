// SPDX-License-Identifier: GPL-3.0-or-later
//
// L'extension de widgets de Corde (cible CordeWidgets, io.github.dultra123.corde.widgets).
// Aucune requête réseau : les widgets ne montrent que le bloc écrit par l'application dans le
// trousseau partagé (SharedStore.swift), à chaque changement du planning, des choix, des
// filtres ou de la langue (et l'application demande alors à iOS de les redessiner).
//
// Chronologie : une entrée maintenant, puis une à chaque départ à venir (la liste se décale
// d'elle-même) et une heure avant chaque départ choisi ; à la fin, iOS redemande une
// chronologie (relue dans le trousseau). Les comptes à rebours tournent sans rechargement.
// Le widget « Courses en images » a sa propre chronologie (une page par minute) : SlidesWidget.swift.

import SwiftUI
import WidgetKit

@main
struct CordeWidgetsBundle: WidgetBundle {
    var body: some Widget {
        RacesWidget()
        SlidesWidget()
        MyRacesWidget()
        NextStartWidget()
        CordeRaceLiveActivity()
    }
}

struct CordeEntry: TimelineEntry {
    let date: Date
    let load: WidgetLoad
}

struct CordeProvider: TimelineProvider {
    /// Entrées d'une chronologie au plus (iOS en redemande une après la dernière).
    static let maxEntries = 80

    func placeholder(in context: Context) -> CordeEntry {
        CordeEntry(date: Date(), load: .ready(SampleData.payload(now: Date(), lang: nil)))
    }

    func getSnapshot(in context: Context, completion: @escaping (CordeEntry) -> Void) {
        let now = Date()
        let load = WidgetLoad.load()
        // Galerie des widgets : un exemple tant que Corde n'a rien écrit.
        if context.isPreview, load.payload == nil {
            completion(CordeEntry(date: now, load: .ready(SampleData.payload(now: now, lang: nil))))
        } else {
            completion(CordeEntry(date: now, load: load))
        }
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<CordeEntry>) -> Void) {
        let now = Date()
        let load = WidgetLoad.load()
        var dates = [now]
        if let payload = load.payload {
            dates += payload.changeDates(after: now, limit: CordeProvider.maxEntries - 1)
        }
        let entries = dates.map { CordeEntry(date: $0, load: load) }
        let next = dates.count > 1 ? dates[dates.count - 1].addingTimeInterval(60) : now.addingTimeInterval(6 * 3600)
        completion(Timeline(entries: entries, policy: .after(next)))
    }
}

let racesURL = URL(string: "corde://races")

struct RacesWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CordeRaces", provider: CordeProvider()) { entry in
            RacesEntryView(entry: entry)
        }
        .configurationDisplayName(L10n.device.racesTitle)
        .description(L10n.device.racesDescription)
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct RacesEntryView: View {
    @Environment(\.widgetFamily) private var family
    let entry: CordeEntry

    var body: some View {
        RacesWidgetView(family: family, date: entry.date, load: entry.load)
            .widgetURL(racesURL)
            .containerBackground(for: .widget) { CordeBackground() }
    }
}

struct MyRacesWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CordeMyRaces", provider: CordeProvider()) { entry in
            MyRacesEntryView(entry: entry)
        }
        .configurationDisplayName(L10n.device.mineTitle)
        .description(L10n.device.mineDescription)
        .supportedFamilies([.systemSmall, .systemMedium])
    }
}

struct MyRacesEntryView: View {
    @Environment(\.widgetFamily) private var family
    let entry: CordeEntry

    var body: some View {
        MyRacesWidgetView(family: family, date: entry.date, load: entry.load)
            .widgetURL(racesURL)
            .containerBackground(for: .widget) { CordeBackground() }
    }
}

/// Écran verrouillé : le prochain départ choisi (à défaut la prochaine course).
struct NextStartWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CordeNextStart", provider: CordeProvider()) { entry in
            NextStartEntryView(entry: entry)
        }
        .configurationDisplayName(L10n.device.nextTitle)
        .description(L10n.device.nextDescription)
        .supportedFamilies([.accessoryRectangular, .accessoryInline, .accessoryCircular])
    }
}

struct NextStartEntryView: View {
    @Environment(\.widgetFamily) private var family
    let entry: CordeEntry

    var body: some View {
        Group {
            switch family {
            case .accessoryInline:
                LockInlineView(date: entry.date, load: entry.load)
            case .accessoryCircular:
                LockCircularView(date: entry.date, load: entry.load)
            default:
                LockRectangularView(date: entry.date, load: entry.load)
            }
        }
        .widgetURL(racesURL)
        .containerBackground(for: .widget) {
            if family == .accessoryCircular {
                AccessoryWidgetBackground()
            } else {
                Color.clear
            }
        }
    }
}
