// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le widget « Courses en images » (kind `CordeSlides`) : une course par page, qui change toute
// seule, et des boutons « précédent » / « suivant ». Vues : Shared/SlideViews.swift.
//
// WidgetKit ne sait pas animer un widget en continu : le défilement se fait par la chronologie.
// Une entrée par minute pendant une heure, puis une toutes les 5 minutes pendant 3 heures (si iOS
// tarde à redemander une chronologie, les pages tournent encore), plus une à chaque départ (la
// liste change) ; iOS en redemande une après une heure. Chaque entrée porte une « position » ;
// la page affichée est cette position modulo le nombre de pages du moment.
//
// La position continue d'une chronologie à la suivante : l'ancre (instant, position) de la
// dernière chronologie est gardée dans les UserDefaults propres à l'extension
// (`UserDefaults.standard` : aucun groupe partagé n'est nécessaire, l'application n'en a pas
// besoin). Un toucher sur « suivant » / « précédent » (AppIntent, iOS 17) ajoute ±1 à un
// décalage gardé au même endroit ; WidgetKit redemande alors la chronologie, qui commence donc
// sur la page voisine. Tous les widgets « Courses en images » partagent ce décalage.
//
// Aucune requête réseau : la photo est lue dans le trousseau partagé au moment du dessin, jamais
// portée par l'entrée (mémoire de l'extension limitée à ~30 Mo).

import AppIntents
import SwiftUI
import WidgetKit

/// L'horloge des pages : position d'une entrée, ancre, décalage manuel.
enum SlideClock {
    private static var defaults: UserDefaults { .standard }
    private static let anchorDateKey = "corde.slides.anchorDate"
    private static let anchorBaseKey = "corde.slides.anchorBase"
    private static let shiftKey = "corde.slides.shift"

    /// Une page par minute pendant une heure…
    static let fastSteps = 60
    /// … puis une toutes les 5 minutes pendant 3 heures.
    static let slowStep: TimeInterval = 300
    static let slowSteps = 36
    /// Durée couverte par une chronologie.
    static var span: TimeInterval { Double(fastSteps) * 60 + Double(slowSteps) * slowStep }

    /// La position à `date` pour une chronologie ancrée à (`anchor`, `base`).
    static func position(at date: Date, anchor: Date, base: Int) -> Int {
        let dt = date.timeIntervalSince(anchor)
        if dt <= 0 { return base }
        let minutes = Int(dt / 60)
        if minutes < fastSteps { return base + minutes }
        return base + fastSteps + Int((dt - Double(fastSteps) * 60) / slowStep)
    }

    /// Le décalage manuel (touchers de « suivant » moins ceux de « précédent »).
    static var shift: Int { defaults.integer(forKey: shiftKey) }

    static func step(_ by: Int) {
        // Gardé petit : seule sa valeur modulo le nombre de pages compte.
        defaults.set((shift + by) % 10_080, forKey: shiftKey)
    }

    private static func storedAnchor() -> (date: Date, base: Int)? {
        guard let t = defaults.object(forKey: anchorDateKey) as? Double else { return nil }
        return (Date(timeIntervalSince1970: t), defaults.integer(forKey: anchorBaseKey))
    }

    /// La position à `date` selon l'ancre gardée (sans rien écrire).
    static func current(at date: Date) -> Int {
        guard let old = storedAnchor(), abs(date.timeIntervalSince(old.date)) < 2 * 86_400 else {
            return Int(date.timeIntervalSince1970 / 60) % 10_080
        }
        return position(at: date, anchor: old.date, base: old.base)
    }

    /// Une nouvelle ancre à la minute de `now`, qui continue la précédente.
    static func reanchor(now: Date) -> (date: Date, base: Int) {
        let minute = Date(timeIntervalSince1970: floor(now.timeIntervalSince1970 / 60) * 60)
        let base = current(at: minute) % 10_080
        defaults.set(minute.timeIntervalSince1970, forKey: anchorDateKey)
        defaults.set(base, forKey: anchorBaseKey)
        return (minute, base)
    }

    /// Les instants des entrées régulières d'une chronologie commencée à `now`.
    static func ticks(from now: Date, anchor: Date) -> [Date] {
        var dates = [now]
        for k in 1 ..< fastSteps { dates.append(anchor.addingTimeInterval(Double(k) * 60)) }
        for j in 0 ..< slowSteps {
            dates.append(anchor.addingTimeInterval(Double(fastSteps) * 60 + Double(j) * slowStep))
        }
        return dates.filter { $0 >= now }
    }
}

/// « Course suivante » / « Course précédente » : un toucher dans le widget (iOS 17).
struct SlideStepIntent: AppIntent {
    static let title: LocalizedStringResource = "Corde : changer de course"
    static let description: IntentDescription? = IntentDescription("Montre la course suivante ou précédente du widget « Courses en images ».")
    static var isDiscoverable: Bool { false }

    @Parameter(title: "Pas", default: 1)
    var step: Int

    init() {}

    init(step: Int) {
        self.step = step
    }

    func perform() async throws -> some IntentResult {
        SlideClock.step(step < 0 ? -1 : 1)
        return .result()
    }
}

struct SlidesEntry: TimelineEntry {
    let date: Date
    let load: WidgetLoad
    /// La page est `position` modulo le nombre de pages à `date`.
    let position: Int
}

struct SlidesProvider: TimelineProvider {
    /// Entrées aux départs au plus (en plus des entrées régulières).
    static let maxStartEntries = 40

    func placeholder(in context: Context) -> SlidesEntry {
        SlidesEntry(date: Date(), load: .ready(SampleData.payload(now: Date(), lang: nil)), position: 0)
    }

    func getSnapshot(in context: Context, completion: @escaping (SlidesEntry) -> Void) {
        let now = Date()
        var load = WidgetLoad.load()
        // Galerie des widgets : un exemple tant que Corde n'a rien écrit.
        if context.isPreview, load.payload == nil {
            load = .ready(SampleData.payload(now: now, lang: nil))
        }
        completion(SlidesEntry(date: now, load: load, position: SlideClock.current(at: now) + SlideClock.shift))
    }

    func getTimeline(in context: Context, completion: @escaping (Timeline<SlidesEntry>) -> Void) {
        let now = Date()
        let load = WidgetLoad.load()
        let anchor = SlideClock.reanchor(now: now)
        let shift = SlideClock.shift
        var dates = SlideClock.ticks(from: now, anchor: anchor.date)
        if let payload = load.payload {
            let end = now.addingTimeInterval(SlideClock.span)
            dates += payload.changeDates(after: now, limit: SlidesProvider.maxStartEntries).filter { $0 < end }
        }
        let entries = Array(Set(dates)).sorted().map {
            SlidesEntry(date: $0, load: load,
                        position: SlideClock.position(at: $0, anchor: anchor.date, base: anchor.base) + shift)
        }
        completion(Timeline(entries: entries, policy: .after(now.addingTimeInterval(3600))))
    }
}

struct SlidesWidget: Widget {
    var body: some WidgetConfiguration {
        StaticConfiguration(kind: "CordeSlides", provider: SlidesProvider()) { entry in
            SlidesEntryView(entry: entry)
        }
        .configurationDisplayName(L10n.device.slidesTitle)
        .description(L10n.device.slidesDescription)
        .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    }
}

struct SlidesEntryView: View {
    @Environment(\.widgetFamily) private var family
    let entry: SlidesEntry

    var body: some View {
        let small = family == .systemSmall
        let page = SlideDeck.page(entry.load, at: entry.date, small: small, position: entry.position)
        // La photo est lue ici, au dessin, et réduite pour le petit widget.
        let image = SlideImages.image(key: page?.slide.race.img, maxSide: small ? 480 : 720)
        SlidesWidgetView(family: family, date: entry.date, load: entry.load, page: page) { step in
            Button(intent: SlideStepIntent(step: step)) {
                SlideChevron(step: step)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(step < 0 ? L10n(entry.load.payload?.lang).previousRace : L10n(entry.load.payload?.lang).nextRace)
        }
        .widgetURL(racesURL)
        .containerBackground(for: .widget) {
            SlideBackdrop(page: page, image: image, small: small)
        }
    }
}
