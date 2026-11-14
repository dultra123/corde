// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le widget « Courses en images » : une course par page, sa photo en fond (ou le fond dessiné
// aux couleurs de sa catégorie avec le nom de la piste, comme le remplaçant du carrousel de
// l'Accueil, packages/ui/src/home/RaceCarousel.tsx), le badge de catégorie / niveau, le SR / DR
// requis (marqué quand le profil du pilote est en dessous), « Ma course », le nom, piste · durée,
// le prochain départ avec son compte à rebours et les suivants, « 2/6 ».
//
// Comme WidgetViews.swift : des vues sans WidgetKit autour, compilées dans les deux cibles.
// L'extension (CordeWidgets/SlidesWidget.swift) les enveloppe — fond par `containerBackground`,
// boutons « précédent » / « suivant » par `Button(intent:)` — et l'application en build Debug
// les dessine en images pour la CI (WidgetShots.swift), avec des boutons inertes.
//
// La photo n'est jamais téléchargée ici : l'application l'a réduite (≤ 720 px, ≤ 48 Kio) et
// rangée dans le trousseau partagé (SharedStore) ; elle est lue au moment du dessin, réduite
// encore pour le petit widget (mémoire de l'extension limitée, taille maximale des images d'un
// widget).

import ImageIO
import SwiftUI
import UIKit
import WidgetKit

// MARK: - Photos

enum SlideImages {
    /// La photo gardée sous `key`, réduite à `maxSide` pixels au plus ; `nil` sans photo.
    static func image(key: String?, maxSide: Int) -> UIImage? {
        guard let key, let data = SharedStore.readImage(key: key).data else { return nil }
        return downsample(data, maxSide: maxSide)
    }

    /// Décodage réduit (ImageIO) : l'image entière n'est jamais décodée.
    static func downsample(_ data: Data, maxSide: Int) -> UIImage? {
        let sourceOptions = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let source = CGImageSourceCreateWithData(data as CFData, sourceOptions),
              CGImageSourceGetCount(source) > 0 else { return nil }
        let options: [CFString: Any] = [
            kCGImageSourceCreateThumbnailFromImageAlways: true,
            kCGImageSourceCreateThumbnailWithTransform: true,
            kCGImageSourceShouldCacheImmediately: true,
            kCGImageSourceThumbnailMaxPixelSize: maxSide,
        ]
        guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
            return nil
        }
        return UIImage(cgImage: image)
    }
}

/// Le mot écrit en grand sur le fond dessiné : la piste sans ses parenthèses (`trackMark`,
/// races/slides.ts).
func trackMark(_ track: String) -> String {
    track.replacingOccurrences(of: "\\(.*?\\)", with: "", options: .regularExpression)
        .trimmingCharacters(in: .whitespaces)
}

// MARK: - Fond

/// Une bande de livrée en biais, de `from` à `to` (fractions de la largeur).
private struct LiveryStripe: Shape {
    let from: CGFloat
    let to: CGFloat

    func path(in r: CGRect) -> Path {
        let slant = r.height * 0.4
        var p = Path()
        p.move(to: CGPoint(x: r.minX + r.width * from + slant / 2, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + r.width * to + slant / 2, y: r.minY))
        p.addLine(to: CGPoint(x: r.minX + r.width * to - slant / 2, y: r.maxY))
        p.addLine(to: CGPoint(x: r.minX + r.width * from - slant / 2, y: r.maxY))
        p.closeSubpath()
        return p
    }
}

/// Sans photo : le fond dessiné aux couleurs de la catégorie, avec la marque de la piste.
struct SlideStandIn: View {
    let race: WidgetRace

    var body: some View {
        let tint = CordeColor.cat(race.cat, .dark)
        ZStack(alignment: .leading) {
            Color(hex: 0x10151C)
            LinearGradient(colors: [tint.opacity(0.42), tint.opacity(0.04)], startPoint: .top, endPoint: .bottom)
            LiveryStripe(from: 0.56, to: 0.64).fill(tint.opacity(0.8))
            LiveryStripe(from: 0.67, to: 0.685).fill(Color.white.opacity(0.3))
            Text(trackMark(race.track).uppercased())
                .font(.system(size: 64, weight: .heavy).width(.condensed))
                .foregroundStyle(Color.white.opacity(0.09))
                .lineLimit(1)
                .fixedSize()
                .padding(.leading, 12)
                .offset(y: -6)
        }
        .clipped()
    }
}

/// Le fond d'une page : la photo cadrée en remplissage (ou le fond dessiné), sous un voile
/// sombre qui garde le texte lisible sur une photo claire comme sur une photo sombre.
struct SlideBackdrop: View {
    let page: SlidePage?
    let image: UIImage?
    var small = false

    var body: some View {
        if let page {
            ZStack {
                Group {
                    if let image {
                        Color.black.overlay(
                            Image(uiImage: image)
                                .resizable()
                                .scaledToFill()
                        )
                    } else {
                        SlideStandIn(race: page.slide.race)
                    }
                }
                .id(page.slide.race.id)
                .transition(.opacity)
                LinearGradient(
                    stops: small ? [
                        .init(color: Color(hex: 0x0B0E12).opacity(0.62), location: 0),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.18), location: 0.32),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.55), location: 0.5),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.95), location: 1),
                    ] : [
                        .init(color: Color(hex: 0x0B0E12).opacity(0.6), location: 0),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.05), location: 0.32),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.45), location: 0.55),
                        .init(color: Color(hex: 0x0B0E12).opacity(0.94), location: 1),
                    ],
                    startPoint: .top,
                    endPoint: .bottom
                )
            }
            .clipped()
        } else {
            CordeBackground()
        }
    }
}

// MARK: - Morceaux

/// `● QUOTIDIENNE · AVANCÉ` sur fond sombre (lisible sur une photo).
private struct SlideKind: View {
    let race: WidgetRace
    let l10n: L10n
    let small: Bool

    var body: some View {
        HStack(spacing: 4) {
            Circle()
                .fill(CordeColor.mark(race.cat, race.lvl, .dark))
                .frame(width: 6, height: 6)
            Text(small && race.cat == "daily" && race.lvl != nil
                 ? l10n.level(race.lvl).uppercased(with: l10n.locale)
                 : l10n.kind(race.cat, race.lvl))
                .font(.system(size: small ? 9 : 10, weight: .heavy))
                .lineLimit(1)
                .minimumScaleFactor(0.7)
        }
        .padding(.horizontal, 6)
        .padding(.vertical, 3)
        .background(Color.black.opacity(0.5), in: Capsule())
        .overlay(Capsule().strokeBorder(CordeColor.cat(race.cat, .dark).opacity(0.55), lineWidth: 1))
        .foregroundStyle(CordeColor.cat(race.cat, .dark))
    }
}

/// « SR S2 » : le rang requis ; cadenas et bord rouge quand le profil du pilote est en dessous.
private struct RatingChip: View {
    let kind: String
    let value: String
    let locked: Bool
    let l10n: L10n
    var compact = false

    var body: some View {
        HStack(spacing: compact ? 2 : 3) {
            if locked {
                Image(systemName: "lock.fill")
                    .font(.system(size: compact ? 7 : 8, weight: .bold))
                    .foregroundStyle(CordeColor.urgent)
            }
            Text(kind)
                .font(.system(size: compact ? 8 : 9, weight: .semibold))
                .foregroundStyle(Color.white.opacity(0.65))
            Text(value)
                .font(.system(size: compact ? 10 : 11, weight: .heavy))
                .monospacedDigit()
                .foregroundStyle(CordeColor.rating(value))
        }
        .lineLimit(1)
        .fixedSize()
        .padding(.horizontal, compact ? 4 : 6)
        .padding(.vertical, 2.5)
        .background(Color.black.opacity(0.55), in: RoundedRectangle(cornerRadius: 5, style: .continuous))
        .overlay(
            RoundedRectangle(cornerRadius: 5, style: .continuous)
                .strokeBorder(locked ? CordeColor.urgent : Color.white.opacity(0.22), lineWidth: locked ? 1.2 : 1)
        )
        .accessibilityLabel(locked ? "\(kind) \(value), \(l10n.locked)" : "\(kind) \(value)")
    }
}

/// « MA COURSE » (petit widget : l'icône seule, faute de place).
private struct MinePill: View {
    let l10n: L10n
    var compact = false

    var body: some View {
        HStack(spacing: 3) {
            Image(systemName: "timer")
                .font(.system(size: compact ? 9 : 8, weight: .heavy))
            if !compact {
                Text(l10n.myRace)
                    .font(.system(size: 9.5, weight: .heavy))
                    .lineLimit(1)
            }
        }
        .fixedSize()
        .accessibilityLabel(l10n.myRace)
        .padding(.horizontal, compact ? 5 : 6)
        .padding(.vertical, 3)
        .background(CordeColor.accent, in: Capsule())
        .foregroundStyle(Color(hex: 0x14110A))
    }
}

/// L'aspect du bouton « précédent » / « suivant » (le bouton lui-même vient de l'extension).
struct SlideChevron: View {
    /// `1` suivant, `-1` précédent.
    let step: Int
    var size: CGFloat = 24

    var body: some View {
        Image(systemName: step < 0 ? "chevron.left" : "chevron.right")
            .font(.system(size: size * 0.45, weight: .bold))
            .foregroundStyle(Color.white)
            .frame(width: size, height: size)
            .background(Color.black.opacity(0.45), in: Circle())
            .overlay(Circle().strokeBorder(Color.white.opacity(0.28), lineWidth: 1))
            .contentShape(Circle())
    }
}

/// Un départ suivant : `21:55`, `≈ 22:25` ; mis en avant quand il est choisi.
private struct StartChip: View {
    let start: SlideStart
    let now: Date
    let l10n: L10n

    var body: some View {
        Text((start.estimated ? "≈ " : "") + l10n.when(start.date, now: now))
            .font(.system(size: 11.5, weight: .bold))
            .monospacedDigit()
            .lineLimit(1)
            .padding(.horizontal, 6)
            .padding(.vertical, 3)
            .foregroundStyle(start.picked ? Color(hex: 0x14110A) : Color.white)
            .background(start.picked ? CordeColor.accent : Color.black.opacity(0.45),
                        in: RoundedRectangle(cornerRadius: 5, style: .continuous))
            .overlay(
                RoundedRectangle(cornerRadius: 5, style: .continuous)
                    .strokeBorder(Color.white.opacity(start.picked ? 0 : 0.22), lineWidth: 1)
            )
    }
}

// MARK: - Le widget

/// La page à afficher (`nil` : un message à la place — planning éteint, bloc périmé, rien à
/// venir).
enum SlideDeck {
    static func page(_ load: WidgetLoad, at date: Date, small: Bool, position: Int) -> SlidePage? {
        guard let p = load.payload, p.sched != "off", !p.isStale(at: date) else { return nil }
        return p.slidePage(at: date, small: small, position: position)
    }
}

/// Le contenu du widget « Courses en images » (sans son fond, `SlideBackdrop`). `nav` construit
/// le bouton « précédent » (-1) ou « suivant » (1).
struct SlidesWidgetView<Nav: View>: View {
    let family: WidgetFamily
    let date: Date
    let load: WidgetLoad
    let page: SlidePage?
    let nav: (Int) -> Nav

    init(family: WidgetFamily, date: Date, load: WidgetLoad, page: SlidePage?,
         @ViewBuilder nav: @escaping (Int) -> Nav) {
        self.family = family
        self.date = date
        self.load = load
        self.page = page
        self.nav = nav
    }

    private var l10n: L10n { L10n(load.payload?.lang) }

    var body: some View {
        content
            .environment(\.locale, l10n.locale)
    }

    @ViewBuilder
    private var content: some View {
        if let page {
            SlideContent(family: family, page: page, now: date, l10n: l10n, nav: nav)
                .id(page.slide.id)
                .transition(.push(from: .trailing))
                .environment(\.colorScheme, .dark)
        } else {
            WidgetMessage(title: l10n.slidesTitle, message: message, symbol: "photo.on.rectangle")
        }
    }

    private var message: String {
        guard let p = load.payload else { return l10n.openCorde }
        if p.sched == "off" { return l10n.scheduleOff }
        if p.isStale(at: date) || p.sched == "none" { return l10n.openCorde }
        return l10n.noRace
    }
}

private struct SlideContent<Nav: View>: View {
    let family: WidgetFamily
    let page: SlidePage
    let now: Date
    let l10n: L10n
    let nav: (Int) -> Nav

    private var slide: Slide { page.slide }
    private var race: WidgetRace { page.slide.race }
    private var small: Bool { family == .systemSmall }
    private var large: Bool { family == .systemLarge }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .center, spacing: 5) {
                SlideKind(race: race, l10n: l10n, small: small)
                Spacer(minLength: 4)
                pager
            }
            badges
                .padding(.top, small ? 4 : 5)
            Spacer(minLength: 4)
            bottom
                .shadow(color: Color.black.opacity(0.55), radius: 3, x: 0, y: 1)
        }
        .foregroundStyle(Color.white)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
    }

    /// `‹ 2/6 ›` (petit : `2/6 ›`).
    private var pager: some View {
        HStack(spacing: small ? 4 : 6) {
            if !small, page.count > 1 { nav(-1) }
            Text("\(page.index + 1)/\(page.count)")
                .font(.system(size: 11, weight: .bold))
                .monospacedDigit()
                .foregroundStyle(Color.white.opacity(0.9))
                .shadow(color: Color.black.opacity(0.6), radius: 2)
                .fixedSize()
            if page.count > 1 { nav(1) }
        }
    }

    @ViewBuilder
    private var badges: some View {
        let req = race.req
        HStack(spacing: small ? 3 : 4) {
            if let req, !req.sr.isEmpty {
                RatingChip(kind: "SR", value: req.sr, locked: req.srLocked, l10n: l10n, compact: small)
            }
            if let req, !req.dr.isEmpty {
                RatingChip(kind: "DR", value: req.dr, locked: req.drLocked, l10n: l10n, compact: small)
            }
            if slide.picked { MinePill(l10n: l10n, compact: small) }
        }
    }

    private var meta: String {
        [race.track, race.dur].filter { !$0.isEmpty }.joined(separator: " · ")
    }

    @ViewBuilder
    private var bottom: some View {
        VStack(alignment: .leading, spacing: small ? 1 : 2) {
            Text(race.name)
                .font(.system(size: small ? 14 : large ? 22 : 18, weight: .heavy))
                .lineLimit(small || large ? 2 : 1)
                .minimumScaleFactor(0.75)
            if !meta.isEmpty {
                Text(meta)
                    .font(.system(size: small ? 10.5 : 12, weight: .semibold))
                    .foregroundStyle(Color.white.opacity(0.85))
                    .lineLimit(1)
                    .minimumScaleFactor(0.8)
            }
            if large, !race.cls.isEmpty {
                Text(race.cls.prefix(4).joined(separator: " · "))
                    .font(.system(size: 11.5, weight: .medium))
                    .foregroundStyle(Color.white.opacity(0.7))
                    .lineLimit(1)
            }
            when
                .padding(.top, small ? 3 : large ? 8 : 4)
            if large, !slide.next.isEmpty {
                Text(l10n.nextStarts.uppercased(with: l10n.locale))
                    .font(.system(size: 9.5, weight: .heavy))
                    .foregroundStyle(Color.white.opacity(0.7))
                    .padding(.top, 8)
                HStack(spacing: 5) {
                    ForEach(slide.next.prefix(4), id: \.self) { s in
                        StartChip(start: s, now: now, l10n: l10n)
                    }
                }
                .padding(.top, 3)
            }
        }
    }

    /// Le départ affiché et le temps restant (événement spécial : ses dates).
    @ViewBuilder
    private var when: some View {
        if race.cat == "special" {
            VStack(alignment: .leading, spacing: 1) {
                Text(slide.ongoing ? l10n.ongoing : l10n.dateRange(slide.start, dateOf(race.to ?? slide.start.timeIntervalSince1970 * 1000)))
                    .font(.system(size: small ? 13 : 15, weight: .heavy))
                    .foregroundStyle(slide.ongoing ? CordeColor.cat(race.cat, .dark) : Color.white)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
                if slide.ongoing, let to = race.to {
                    Text(l10n.dateRange(slide.start, dateOf(to)))
                        .font(.system(size: 10.5, weight: .semibold))
                        .foregroundStyle(Color.white.opacity(0.8))
                        .lineLimit(1)
                } else if !slide.ongoing {
                    TimeLeft(start: slide.start, now: now, l10n: l10n)
                        .font(.system(size: small ? 10.5 : 11.5, weight: .semibold))
                        .foregroundStyle(CordeColor.accent)
                        .lineLimit(1)
                        .minimumScaleFactor(0.7)
                }
            }
        } else if small {
            VStack(alignment: .leading, spacing: 0) {
                startLine(size: 19)
                TimeLeft(start: slide.start, now: now, l10n: l10n)
                    .font(.system(size: 10.5, weight: .semibold))
                    .foregroundStyle(soon ? CordeColor.urgent : CordeColor.accent)
                    .lineLimit(1)
                    .minimumScaleFactor(0.7)
            }
        } else {
            HStack(alignment: .bottom, spacing: 8) {
                VStack(alignment: .leading, spacing: 0) {
                    startLine(size: large ? 30 : 20)
                    TimeLeft(start: slide.start, now: now, l10n: l10n)
                        .font(.system(size: large ? 13 : 11, weight: .semibold))
                        .foregroundStyle(soon ? CordeColor.urgent : CordeColor.accent)
                        .lineLimit(1)
                        .minimumScaleFactor(0.7)
                }
                .layoutPriority(1)
                Spacer(minLength: 0)
                if !large, !slide.next.isEmpty {
                    HStack(spacing: 4) {
                        ForEach(slide.next.prefix(3), id: \.self) { s in
                            StartChip(start: s, now: now, l10n: l10n)
                        }
                    }
                }
            }
        }
    }

    private var soon: Bool { slide.start.timeIntervalSince(now) <= 300 }

    private func startLine(size: CGFloat) -> some View {
        Text((slide.estimated ? "≈ " : "") + l10n.when(slide.start, now: now))
            .font(.system(size: size, weight: .heavy))
            .monospacedDigit()
            .lineLimit(1)
            .minimumScaleFactor(0.6)
    }
}
