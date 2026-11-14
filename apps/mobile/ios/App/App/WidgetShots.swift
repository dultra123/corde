// SPDX-License-Identifier: GPL-3.0-or-later
//
// Build Debug seulement : ce qui permet à la CI de « voir » les widgets et l'activité en direct
// sans Mac ni iPhone (personne dans le projet n'en a).
//
//   -CordeRenderShots          dessine chaque vue des widgets et de l'activité (données
//                              d'exemple inventées, SampleData.swift) avec ImageRenderer, en fr
//                              et en, clair et sombre, sur quatre planches PNG dans
//                              Documents/corde-shots/ ; l'essai de fumée de ios.yml les imprime
//                              en base64 dans le journal. Lit aussi le bloc écrit par
//                              l'application (même code que l'extension) et le dit dans le
//                              journal.
//   -CordeOpenURL <lien>       l'application ouvre elle-même ce lien après -CordeOpenDelay
//   -CordeOpenDelay <s>        secondes (15 par défaut) : même chemin qu'un toucher de widget
//                              (scene(_:openURLContexts:)), sans la fenêtre « Ouvrir dans
//                              Corde ? » que `simctl openurl` fait apparaître.
//
// Rien de ceci n'existe dans le build Release (TestFlight).

#if DEBUG
import SwiftUI
import UIKit
import WidgetKit

enum CordeDebug {
    private static var args: [String] { ProcessInfo.processInfo.arguments }

    private static func value(_ name: String) -> String? {
        guard let i = args.firstIndex(of: name), i + 1 < args.count else { return nil }
        return args[i + 1]
    }

    static func didFinishLaunching() {
        guard args.contains("-CordeRenderShots") else { return }
        DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
            Task { @MainActor in
                WidgetShots.renderAll()
            }
        }
    }

    static func sceneDidConnect() {
        guard let link = value("-CordeOpenURL"), let url = URL(string: link) else { return }
        let delay = Double(value("-CordeOpenDelay") ?? "") ?? 15
        DispatchQueue.main.asyncAfter(deadline: .now() + delay) {
            Task { @MainActor in
                NSLog("[corde-native] debug: opening %@", link)
                UIApplication.shared.open(url) { ok in
                    NSLog("[corde-native] debug: open %@", ok ? "accepted" : "refused")
                }
            }
        }
    }
}

/// Une vignette : la vue à la taille d'un widget, avec son fond, et son nom dessous.
private struct Tile<Content: View>: View {
    let label: String
    let width: CGFloat
    let height: CGFloat
    var padding: CGFloat = 16
    var radius: CGFloat = 22
    var lockScreen = false
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ZStack {
                if lockScreen {
                    LinearGradient(colors: [Color(hex: 0x2B3A55), Color(hex: 0x10151F)], startPoint: .topLeading, endPoint: .bottomTrailing)
                } else {
                    CordeBackground()
                }
                content
                    .padding(padding)
                    .frame(width: width, height: height)
                    .foregroundStyle(lockScreen ? Color.white : Color.primary)
            }
            .frame(width: width, height: height)
            .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
            Text(label)
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(.secondary)
        }
    }
}

/// Une planche : toutes les vues, pour une langue et un mode (clair / sombre).
private struct ShotSheet: View {
    let lang: String
    let dark: Bool
    let now: Date

    var body: some View {
        let full = WidgetLoad.ready(SampleData.payload(now: now, lang: lang))
        let noPicks = WidgetLoad.ready(SampleData.payload(now: now, lang: lang, picks: false))
        let off = WidgetLoad.ready(WidgetPayload(at: now.timeIntervalSince1970 * 1000, lang: lang, sched: "off",
                                                 until: now.timeIntervalSince1970 * 1000, races: [], mine: []))
        let model = ActivityModel(attributes: SampleData.activity(lang: lang), start: now.addingTimeInterval(27 * 60 + 41), stale: false)
        let stale = ActivityModel(attributes: SampleData.activity(lang: lang), start: now.addingTimeInterval(-60), stale: true)

        VStack(alignment: .leading, spacing: 14) {
            Text("Corde · \(lang) · \(dark ? "dark" : "light")")
                .font(.system(size: 14, weight: .bold))
            HStack(alignment: .top, spacing: 14) {
                Tile(label: "Courses · small", width: 170, height: 170) {
                    RacesWidgetView(family: .systemSmall, date: now, load: full)
                }
                Tile(label: "Courses · medium", width: 364, height: 170) {
                    RacesWidgetView(family: .systemMedium, date: now, load: full)
                }
                Tile(label: "Courses · small · off", width: 170, height: 170) {
                    RacesWidgetView(family: .systemSmall, date: now, load: off)
                }
            }
            HStack(alignment: .top, spacing: 14) {
                Tile(label: "Courses · large", width: 364, height: 382) {
                    RacesWidgetView(family: .systemLarge, date: now, load: full)
                }
                VStack(alignment: .leading, spacing: 14) {
                    Tile(label: "Mes courses · medium", width: 364, height: 170) {
                        MyRacesWidgetView(family: .systemMedium, date: now, load: full)
                    }
                    HStack(alignment: .top, spacing: 14) {
                        Tile(label: "Mes courses · small", width: 170, height: 170) {
                            MyRacesWidgetView(family: .systemSmall, date: now, load: full)
                        }
                        Tile(label: "Mes courses · small · vide", width: 170, height: 170) {
                            MyRacesWidgetView(family: .systemSmall, date: now, load: noPicks)
                        }
                    }
                }
            }
            HStack(alignment: .top, spacing: 14) {
                Tile(label: "Courses · small · rien d'écrit", width: 170, height: 170) {
                    RacesWidgetView(family: .systemSmall, date: now, load: .empty)
                }
                VStack(alignment: .leading, spacing: 10) {
                    Tile(label: "Écran verrouillé · rectangulaire", width: 172, height: 76, padding: 6, radius: 12, lockScreen: true) {
                        LockRectangularView(date: now, load: full)
                    }
                    Tile(label: "Écran verrouillé · en ligne", width: 260, height: 26, padding: 2, radius: 6, lockScreen: true) {
                        LockInlineView(date: now, load: full)
                            .font(.system(size: 14, weight: .semibold))
                    }
                }
                Tile(label: "Écran verrouillé · rond", width: 76, height: 76, padding: 4, radius: 38, lockScreen: true) {
                    LockCircularView(date: now, load: full)
                }
                Tile(label: "Rectangulaire · rien", width: 172, height: 76, padding: 6, radius: 12, lockScreen: true) {
                    LockRectangularView(date: now, load: .empty)
                }
            }
            Text("Activité en direct")
                .font(.system(size: 12, weight: .bold))
            HStack(alignment: .top, spacing: 14) {
                Tile(label: "Écran verrouillé", width: 364, height: 110, padding: 0, lockScreen: true) {
                    ActivityLockView(model: model, now: now)
                        .background(Color(hex: 0x0B0E12).opacity(0.86))
                        .environment(\.colorScheme, .dark)
                }
                Tile(label: "Écran verrouillé · départ donné", width: 364, height: 110, padding: 0, lockScreen: true) {
                    ActivityLockView(model: stale, now: now)
                        .background(Color(hex: 0x0B0E12).opacity(0.86))
                        .environment(\.colorScheme, .dark)
                }
            }
            HStack(alignment: .top, spacing: 14) {
                Tile(label: "Dynamic Island · déployée", width: 370, height: 150, padding: 14, radius: 44, lockScreen: true) {
                    VStack(spacing: 10) {
                        HStack(alignment: .center) {
                            IslandLeading(model: model)
                            Spacer(minLength: 4)
                            IslandCenter(model: model)
                            Spacer(minLength: 4)
                            IslandTrailing(model: model, now: now)
                        }
                        IslandBottom(model: model)
                        Spacer(minLength: 0)
                    }
                    .padding(.top, 8)
                    .background(Color.black)
                    .environment(\.colorScheme, .dark)
                }
                VStack(alignment: .leading, spacing: 10) {
                    Tile(label: "Dynamic Island · compacte", width: 250, height: 37, padding: 0, radius: 19, lockScreen: true) {
                        HStack {
                            IslandCompactLeading(model: model)
                            Spacer()
                            IslandCompactTrailing(model: model, now: now)
                        }
                        .padding(.horizontal, 12)
                        .frame(width: 250, height: 37)
                        .background(Color.black)
                    }
                    Tile(label: "compacte · départ donné", width: 250, height: 37, padding: 0, radius: 19, lockScreen: true) {
                        HStack {
                            IslandCompactLeading(model: stale)
                            Spacer()
                            IslandCompactTrailing(model: stale, now: now)
                        }
                        .padding(.horizontal, 12)
                        .frame(width: 250, height: 37)
                        .background(Color.black)
                    }
                    Tile(label: "minimale", width: 37, height: 37, padding: 0, radius: 19, lockScreen: true) {
                        IslandMinimal(model: model, now: now)
                            .frame(width: 37, height: 37)
                            .background(Color.black)
                    }
                }
            }
        }
        .padding(16)
        .background(dark ? Color(hex: 0x000000) : Color(hex: 0xD9DEE5))
        .environment(\.colorScheme, dark ? .dark : .light)
    }
}

@MainActor
enum WidgetShots {
    static func renderAll() {
        // Le bloc écrit par la partie JavaScript au lancement précédent, relu comme l'extension.
        let read = SharedStore.read()
        let load = read.data.map(WidgetLoad.decode)
        let state: String
        switch load {
        case let .ready(p)?: state = "version \(p.v), schedule \(p.sched), \(p.races.count) races, \(p.mine.count) picks"
        case .unreadable?: state = "unreadable"
        default: state = "none"
        }
        let group = SharedStore.accessGroup == nil ? "missing" : "found"
        NSLog("[corde-native] %@", "store read: OSStatus \(read.status), \(read.data?.count ?? 0) bytes, \(state), group \(group)")

        let fm = FileManager.default
        guard let docs = fm.urls(for: .documentDirectory, in: .userDomainMask).first else { return }
        let dir = docs.appendingPathComponent("corde-shots", isDirectory: true)
        try? fm.removeItem(at: dir)
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true)
        let now = Date()
        var count = 0
        for lang in ["fr", "en"] {
            for dark in [false, true] {
                let renderer = ImageRenderer(content: ShotSheet(lang: lang, dark: dark, now: now))
                renderer.scale = 2
                guard let png = renderer.uiImage?.pngData() else {
                    NSLog("[corde-native] shots: %@ %@ not rendered", lang, dark ? "dark" : "light")
                    continue
                }
                let file = dir.appendingPathComponent("widgets-\(lang)-\(dark ? "dark" : "light").png")
                if (try? png.write(to: file)) != nil { count += 1 }
            }
        }
        try? Data("\(count)\n".utf8).write(to: dir.appendingPathComponent("done.txt"))
        NSLog("[corde-native] %@", "shots: \(count) images in Documents/corde-shots")
    }
}
#endif
