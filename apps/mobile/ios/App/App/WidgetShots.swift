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
//                              journal. Widget « Courses en images » : aller-retour du magasin
//                              des photos (trousseau), réduction d'une grande image fabriquée,
//                              trois planches avec des photos d'exemple dessinées par le code
//                              (aucune photo dans le dépôt, aucun téléchargement).
//   -CordeDemoActivity         démarre une activité en direct d'exemple (la vraie extension la
//                              dessine ; l'essai de fumée la photographie dans la Dynamic Island).
//   -CordeOpenURL <lien>       l'application ouvre elle-même ce lien après -CordeOpenDelay
//   -CordeOpenDelay <s>        secondes (15 par défaut) : même chemin qu'un toucher de widget
//                              (scene(_:openURLContexts:)), sans la fenêtre « Ouvrir dans
//                              Corde ? » que `simctl openurl` fait apparaître.
//
// Rien de ceci n'existe dans le build Release (TestFlight).

#if DEBUG
import ActivityKit
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
        if args.contains("-CordeRenderShots") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) {
                Task { @MainActor in
                    WidgetShots.renderAll()
                }
            }
        }
        if args.contains("-CordeDemoActivity") {
            DispatchQueue.main.asyncAfter(deadline: .now() + 3) {
                Task { @MainActor in
                    startDemoActivity()
                }
            }
        }
    }

    /// Une activité d'exemple (départ dans 25 min), dessinée par la vraie extension : l'essai de
    /// fumée met ensuite une autre application au premier plan pour voir la Dynamic Island.
    @MainActor
    private static func startDemoActivity() {
        let lang = L10n.device.fr ? "fr" : "en"
        let start = Date().addingTimeInterval(25 * 60)
        let state = CordeRaceAttributes.ContentState(startMs: start.timeIntervalSince1970 * 1000)
        let allowed = ActivityAuthorizationInfo().areActivitiesEnabled
        do {
            _ = try Activity.request(attributes: SampleData.activity(lang: lang),
                                     content: ActivityContent(state: state, staleDate: start),
                                     pushType: nil)
            NSLog("[corde-native] %@", "demo activity: started (allowed \(allowed), \(Activity<CordeRaceAttributes>.activities.count) running)")
        } catch {
            NSLog("[corde-native] %@", "demo activity: refused (allowed \(allowed)): \(error.localizedDescription)")
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

// MARK: - « Courses en images »

/// Photos d'exemple fabriquées par le code (dégradés, formes) : aucune photo dans le dépôt.
enum SamplePictures {
    private static func draw(_ size: CGSize, _ body: (CGContext, CGSize) -> Void) -> UIImage {
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        return UIGraphicsImageRenderer(size: size, format: format).image { ctx in
            body(ctx.cgContext, size)
        }
    }

    private static func gradient(_ c: CGContext, _ colors: [UIColor], from: CGPoint, to: CGPoint) {
        let cg = colors.map(\.cgColor) as CFArray
        if let g = CGGradient(colorsSpace: CGColorSpaceCreateDeviceRGB(), colors: cg, locations: nil) {
            c.drawLinearGradient(g, start: from, end: to, options: [.drawsBeforeStartLocation, .drawsAfterEndLocation])
        }
    }

    private static func color(_ hex: UInt32, _ alpha: CGFloat = 1) -> UIColor {
        UIColor(red: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
                blue: CGFloat(hex & 0xFF) / 255, alpha: alpha)
    }

    /// Une piste en courbe, avec ses vibreurs rouge et blanc.
    private static func track(_ c: CGContext, _ s: CGSize, asphalt: UIColor, line: UIColor) {
        let path = UIBezierPath()
        path.move(to: CGPoint(x: -40, y: s.height * 0.95))
        path.addCurve(to: CGPoint(x: s.width + 40, y: s.height * 0.62),
                      controlPoint1: CGPoint(x: s.width * 0.35, y: s.height * 0.55),
                      controlPoint2: CGPoint(x: s.width * 0.6, y: s.height * 0.9))
        c.setLineCap(.butt)
        c.addPath(path.cgPath)
        c.setStrokeColor(color(0xD8D8D8).cgColor)
        c.setLineWidth(s.height * 0.26)
        c.strokePath()
        c.addPath(path.cgPath)
        c.setStrokeColor(color(0xD7262E).cgColor)
        c.setLineDash(phase: 0, lengths: [26, 26])
        c.setLineWidth(s.height * 0.26)
        c.strokePath()
        c.setLineDash(phase: 0, lengths: [])
        c.addPath(path.cgPath)
        c.setStrokeColor(asphalt.cgColor)
        c.setLineWidth(s.height * 0.22)
        c.strokePath()
        c.addPath(path.cgPath)
        c.setStrokeColor(line.cgColor)
        c.setLineDash(phase: 0, lengths: [40, 30])
        c.setLineWidth(4)
        c.strokePath()
        c.setLineDash(phase: 0, lengths: [])
    }

    /// Coucher de soleil sur une piste.
    static func sunset() -> UIImage {
        draw(CGSize(width: 1280, height: 720)) { c, s in
            gradient(c, [color(0x3B1F5C), color(0xC2456A), color(0xFF9A3D)], from: .zero, to: CGPoint(x: 0, y: s.height * 0.55))
            c.setFillColor(color(0xFFD27A).cgColor)
            c.fillEllipse(in: CGRect(x: s.width * 0.62, y: s.height * 0.3, width: 150, height: 150))
            c.setFillColor(color(0x1B1622).cgColor)
            c.fill(CGRect(x: 0, y: s.height * 0.5, width: s.width, height: s.height * 0.5))
            for i in 0 ..< 9 {
                c.setFillColor(color(0x2A2233).cgColor)
                c.fill(CGRect(x: CGFloat(i) * 150 + 20, y: s.height * 0.42, width: 120, height: s.height * 0.1))
            }
            track(c, s, asphalt: color(0x2C2C30), line: color(0xFFFFFF, 0.7))
        }
    }

    /// Photo très claire (ciel blanc, herbe pâle) : le cas difficile pour la lisibilité.
    static func bright() -> UIImage {
        draw(CGSize(width: 1280, height: 720)) { c, s in
            gradient(c, [color(0xCFE4F7), color(0xFFFFFF)], from: .zero, to: CGPoint(x: 0, y: s.height * 0.5))
            c.setFillColor(color(0xD9EBC0).cgColor)
            c.fill(CGRect(x: 0, y: s.height * 0.48, width: s.width, height: s.height * 0.52))
            c.setFillColor(color(0xF4F4F4).cgColor)
            c.fill(CGRect(x: s.width * 0.05, y: s.height * 0.3, width: s.width * 0.5, height: s.height * 0.2))
            track(c, s, asphalt: color(0xBFC3C8), line: color(0xFFFFFF))
        }
    }

    /// Nuit, projecteurs et traînées de feux.
    static func night() -> UIImage {
        draw(CGSize(width: 1280, height: 720)) { c, s in
            gradient(c, [color(0x050814), color(0x13204A)], from: .zero, to: CGPoint(x: 0, y: s.height * 0.6))
            c.setFillColor(color(0x07090F).cgColor)
            c.fill(CGRect(x: 0, y: s.height * 0.55, width: s.width, height: s.height * 0.45))
            for i in 0 ..< 6 {
                let x = CGFloat(i) * 230 + 60
                c.setFillColor(color(0xFFE9A8, 0.25).cgColor)
                c.fillEllipse(in: CGRect(x: x - 60, y: s.height * 0.18 - 60, width: 120, height: 120))
                c.setFillColor(color(0xFFF6D8).cgColor)
                c.fillEllipse(in: CGRect(x: x - 12, y: s.height * 0.18 - 12, width: 24, height: 24))
            }
            track(c, s, asphalt: color(0x1E2026), line: color(0xFFFFFF, 0.4))
            for (i, hex) in [0xFF3B30, 0xFFFFFF, 0xFF3B30].enumerated() {
                c.setStrokeColor(color(UInt32(hex), 0.85).cgColor)
                c.setLineWidth(3)
                c.move(to: CGPoint(x: 100, y: s.height * 0.8 + CGFloat(i) * 10))
                c.addLine(to: CGPoint(x: s.width * 0.7, y: s.height * 0.68 + CGFloat(i) * 8))
                c.strokePath()
            }
        }
    }

    /// Une grande image difficile à comprimer (4000 × 3000, des milliers de rectangles).
    static func huge() -> UIImage {
        draw(CGSize(width: 4000, height: 3000)) { c, s in
            var rng = SystemRandomNumberGenerator()
            for _ in 0 ..< 4000 {
                let color = UIColor(red: .random(in: 0 ... 1, using: &rng), green: .random(in: 0 ... 1, using: &rng),
                                    blue: .random(in: 0 ... 1, using: &rng), alpha: 1)
                c.setFillColor(color.cgColor)
                c.fill(CGRect(x: .random(in: 0 ... s.width, using: &rng), y: .random(in: 0 ... s.height, using: &rng),
                              width: .random(in: 8 ... 220, using: &rng), height: .random(in: 8 ... 220, using: &rng)))
            }
        }
    }
}

/// Une vignette du widget en images : la photo (ou le fond dessiné) en fond, le contenu dessus.
private struct SlideTile<Content: View>: View {
    let label: String
    let width: CGFloat
    let height: CGFloat
    let page: SlidePage?
    let image: UIImage?
    let small: Bool
    @ViewBuilder let content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            ZStack {
                SlideBackdrop(page: page, image: image, small: small)
                    .frame(width: width, height: height)
                content
                    .padding(16)
                    .frame(width: width, height: height)
                    .foregroundStyle(Color.primary)
            }
            .frame(width: width, height: height)
            .clipShape(RoundedRectangle(cornerRadius: 22, style: .continuous))
            Text(label)
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(.secondary)
        }
    }
}

/// La planche du widget « Courses en images » : trois tailles, avec photo (sombre, claire),
/// sans photo, verrouillé, « Ma course », événement spécial, messages.
private struct SlidesSheet: View {
    let lang: String
    let dark: Bool
    let now: Date
    /// Clé → photo réduite (passée par la vraie réduction, WidgetImages.reduce).
    let pictures: [String: UIImage]

    private func tile(_ label: String, _ family: WidgetFamily, race: String?, load: WidgetLoad) -> some View {
        let small = family == .systemSmall
        let size: CGSize = small ? CGSize(width: 170, height: 170)
            : family == .systemMedium ? CGSize(width: 364, height: 170) : CGSize(width: 364, height: 382)
        var position = 0
        if let race, let p = load.payload {
            position = p.slides(at: now, small: small).firstIndex { $0.race.id == race } ?? 0
        }
        let page = SlideDeck.page(load, at: now, small: small, position: position)
        let image = page?.slide.race.img.flatMap { pictures[$0] }
        return SlideTile(label: label, width: size.width, height: size.height, page: page, image: image, small: small) {
            SlidesWidgetView(family: family, date: now, load: load, page: page) { step in
                SlideChevron(step: step)
            }
        }
    }

    var body: some View {
        let full = WidgetLoad.ready(SampleData.payload(now: now, lang: lang))
        let off = WidgetLoad.ready(WidgetPayload(at: now.timeIntervalSince1970 * 1000, lang: lang, sched: "off",
                                                 until: 0, races: [], mine: []))
        VStack(alignment: .leading, spacing: 14) {
            Text("Corde · Courses en images · \(lang) · \(dark ? "dark" : "light")")
                .font(.system(size: 14, weight: .bold))
            HStack(alignment: .top, spacing: 14) {
                tile("small · Ma course (photo claire)", .systemSmall, race: nil, load: full)
                tile("small · photo de nuit, SR / DR verrouillés", .systemSmall, race: "hyper-advanced", load: full)
                tile("small · sans photo", .systemSmall, race: "weekly-endurance", load: full)
                tile("small · rien d'écrit", .systemSmall, race: nil, load: .empty)
            }
            HStack(alignment: .top, spacing: 14) {
                tile("medium · photo (coucher de soleil)", .systemMedium, race: "sprint-gt3-beginner", load: full)
                tile("medium · Ma course, départs choisis en avant", .systemMedium, race: "lmp2-cup-intermediate", load: full)
            }
            HStack(alignment: .top, spacing: 14) {
                tile("medium · événement spécial, sans photo", .systemMedium, race: "special-6h", load: full)
                tile("medium · planning éteint", .systemMedium, race: nil, load: off)
            }
            HStack(alignment: .top, spacing: 14) {
                tile("large · Ma course (photo claire)", .systemLarge, race: "lmp2-cup-intermediate", load: full)
                tile("large · photo de nuit, verrouillé", .systemLarge, race: "hyper-advanced", load: full)
                tile("large · sans photo", .systemLarge, race: "weekly-endurance", load: full)
            }
        }
        .padding(16)
        .background(dark ? Color(hex: 0x000000) : Color(hex: 0xD9DEE5))
        .environment(\.colorScheme, dark ? .dark : .light)
    }
}

@MainActor
enum WidgetShots {
    /// Aller-retour du magasin des photos (le vrai trousseau du simulateur) : écrire ~40 Kio,
    /// relire, lister, supprimer, relire ; chaque OSStatus dans le journal.
    static func imageStoreRoundTrip() {
        let key = "i00000000000000aa"
        var rng = SystemRandomNumberGenerator()
        let data = Data((0 ..< 40 * 1024).map { _ in UInt8.random(in: 0 ... 255, using: &rng) })
        let write = SharedStore.writeImage(data, key: key)
        let read = SharedStore.readImage(key: key)
        let list = SharedStore.imageKeys()
        let delete = SharedStore.deleteImage(key: key)
        let after = SharedStore.readImage(key: key)
        let same = read.data == data ? "same" : "different"
        let found = list.keys.contains(key) ? "found" : "missing"
        NSLog("[corde-native] %@", "image store: write OSStatus \(write), read OSStatus \(read.status) (\(read.data?.count ?? 0) bytes, \(same)), list OSStatus \(list.status) (\(list.keys.count) keys, \(found)), delete OSStatus \(delete), read after delete OSStatus \(after.status)")
    }

    /// Réduction d'une grande image fabriquée : JPEG de 48 Kio au plus.
    static func reductionTest() {
        guard let source = SamplePictures.huge().jpegData(compressionQuality: 0.95) else {
            NSLog("[corde-native] %@", "image reduce: source not encoded")
            return
        }
        let start = Date()
        let out = WidgetImages.reduce(source)
        let ms = Int(Date().timeIntervalSince(start) * 1000)
        let size = out.flatMap { SlideImages.downsample($0, maxSide: 4000) }.map { "\(Int($0.size.width))x\(Int($0.size.height))" } ?? "?"
        let verdict = (out?.count ?? Int.max) <= WidgetImages.maxStoredBytes ? "ok" : "FAILED"
        NSLog("[corde-native] %@", "image reduce: 4000x3000 source \(source.count) bytes → JPEG \(out?.count ?? 0) bytes, \(size), \(ms) ms, \(verdict) (limit \(WidgetImages.maxStoredBytes))")
    }

    /// Les photos d'exemple, passées par la vraie réduction puis relues comme dans l'extension.
    static func samplePictures() -> [String: UIImage] {
        var out: [String: UIImage] = [:]
        let made: [(String, UIImage)] = [
            (SampleData.sunsetKey, SamplePictures.sunset()),
            (SampleData.brightKey, SamplePictures.bright()),
            (SampleData.nightKey, SamplePictures.night()),
        ]
        for (key, image) in made {
            guard let png = image.pngData(), let jpeg = WidgetImages.reduce(png),
                  let back = SlideImages.downsample(jpeg, maxSide: 720) else {
                NSLog("[corde-native] %@", "sample picture \(key): not reduced")
                continue
            }
            NSLog("[corde-native] %@", "sample picture \(key): \(png.count) → \(jpeg.count) bytes, \(Int(back.size.width))x\(Int(back.size.height))")
            out[key] = back
        }
        return out
    }

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

        // « Courses en images » : magasin des photos, réduction, puis les planches.
        imageStoreRoundTrip()
        reductionTest()
        let pictures = samplePictures()
        for (lang, dark) in [("fr", true), ("fr", false), ("en", true)] {
            let renderer = ImageRenderer(content: SlidesSheet(lang: lang, dark: dark, now: now, pictures: pictures))
            renderer.scale = 2
            guard let png = renderer.uiImage?.pngData() else {
                NSLog("[corde-native] shots: slides %@ %@ not rendered", lang, dark ? "dark" : "light")
                continue
            }
            let file = dir.appendingPathComponent("slides-\(lang)-\(dark ? "dark" : "light").png")
            if (try? png.write(to: file)) != nil { count += 1 }
        }
        try? Data("\(count)\n".utf8).write(to: dir.appendingPathComponent("done.txt"))
        NSLog("[corde-native] %@", "shots: \(count) images in Documents/corde-shots")
    }
}
#endif
