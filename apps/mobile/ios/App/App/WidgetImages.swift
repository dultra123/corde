// SPDX-License-Identifier: GPL-3.0-or-later
//
// Les photos du widget « Courses en images », côté application (jamais dans l'extension, qui ne
// fait aucune requête réseau). La partie JavaScript donne la liste des photos des pages
// (packages/ui/src/mobile/widget-sync.ts, au plus 8, seulement quand le planning est activé) ;
// pour chacune qui manque :
//   - téléchargement par une session `URLSession` éphémère : ni cookies ni cache, pas de
//     `Referer`, `Accept: image/*`, 15 s au plus, 4 Mo au plus, `https` seulement, jamais vers
//     une adresse IP écrite telle quelle, `localhost` ou un nom en `.local` (redirections
//     comprises) ;
//   - réduction avec ImageIO (vignette, plus grand côté ≤ 720 px, orientation appliquée, aucune
//     métadonnée gardée), JPEG de qualité 0,6, puis moins (et plus petit) jusqu'à tenir sous
//     48 Kio ;
//   - écriture dans le trousseau partagé (SharedStore, un élément par photo).
// Les photos qui ne sont plus dans la liste sont supprimées. ADR 0012, docs/IOS.md.
//
// L'hébergeur d'une photo voit l'adresse IP du téléphone, comme quand l'interface l'affiche
// (PRIVACY.md).

import Foundation
import ImageIO
import WidgetKit

enum WidgetImages {
    static let maxDownloadBytes = 4 * 1024 * 1024
    static let maxStoredBytes = 48 * 1024
    static let maxSide = 720
    static let timeout: TimeInterval = 15
    /// Au-delà, l'image n'est pas décodée (bombe de décompression).
    static let maxPixels = 60_000_000

    struct Item: Sendable {
        let key: String
        let url: URL
    }

    struct Outcome {
        var written = 0
        var kept = 0
        var failed = 0
        var removed = 0
        var bytes = 0
        var count = 0
        /// Dernier `OSStatus` du trousseau autre qu'un succès (0 sinon).
        var status: OSStatus = errSecSuccess
    }

    enum Failure: Error {
        case refusedAddress
        case http(Int)
        case tooLarge
        case undecodable
    }

    /// Une adresse acceptée : `https`, un nom d'hôte (pas une adresse IP), ni `localhost` ni
    /// `.local`, sans identifiants.
    static func isAllowed(_ url: URL) -> Bool {
        guard url.scheme?.lowercased() == "https", url.user == nil, url.password == nil,
              var host = url.host?.lowercased(), !host.isEmpty else { return false }
        while host.hasSuffix(".") { host.removeLast() }
        if host.contains(":") || host.contains("%") || host.contains("[") { return false } // IPv6
        let labels = host.split(separator: ".", omittingEmptySubsequences: false)
        guard labels.count >= 2, labels.allSatisfy({ !$0.isEmpty }) else { return false }
        if host == "localhost" || host.hasSuffix(".localhost") || host.hasSuffix(".local") { return false }
        // IPv4 (y compris les formes courtes, décimales ou hexadécimales) : le dernier libellé
        // d'un vrai nom de domaine n'est jamais numérique.
        let last = labels[labels.count - 1]
        if last.allSatisfy(\.isNumber) || last.hasPrefix("0x") { return false }
        return true
    }

    /// Refuse une redirection vers une adresse non acceptée, et n'envoie pas de `Referer`.
    private final class RedirectGuard: NSObject, URLSessionTaskDelegate {
        func urlSession(_ session: URLSession, task: URLSessionTask,
                        willPerformHTTPRedirection response: HTTPURLResponse,
                        newRequest request: URLRequest,
                        completionHandler: @escaping (URLRequest?) -> Void) {
            guard let url = request.url, WidgetImages.isAllowed(url) else {
                completionHandler(nil)
                return
            }
            var next = request
            next.setValue(nil, forHTTPHeaderField: "Referer")
            next.setValue(nil, forHTTPHeaderField: "Cookie")
            completionHandler(next)
        }
    }

    private static func makeSession() -> URLSession {
        let config = URLSessionConfiguration.ephemeral
        config.httpCookieStorage = nil
        config.httpShouldSetCookies = false
        config.httpCookieAcceptPolicy = .never
        config.urlCache = nil
        config.urlCredentialStorage = nil
        config.requestCachePolicy = .reloadIgnoringLocalAndRemoteCacheData
        config.timeoutIntervalForRequest = timeout
        config.timeoutIntervalForResource = timeout
        config.waitsForConnectivity = false
        config.httpMaximumConnectionsPerHost = 2
        let version = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "0"
        config.httpAdditionalHeaders = [
            "Accept": "image/*",
            // Même forme que pour la lecture du planning (packages/ui/src/mobile/env.ts).
            "User-Agent": "Corde/\(version) (+https://github.com/dultra123/corde)",
        ]
        return URLSession(configuration: config)
    }

    /// Télécharge une photo (≤ 4 Mo).
    static func download(_ url: URL, session: URLSession) async throws -> Data {
        guard isAllowed(url) else { throw Failure.refusedAddress }
        var request = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalAndRemoteCacheData, timeoutInterval: timeout)
        request.httpShouldHandleCookies = false
        let (bytes, response) = try await session.bytes(for: request, delegate: RedirectGuard())
        guard let http = response as? HTTPURLResponse, (200 ..< 300).contains(http.statusCode) else {
            bytes.task.cancel()
            throw Failure.http((response as? HTTPURLResponse)?.statusCode ?? 0)
        }
        if http.expectedContentLength > Int64(maxDownloadBytes) {
            bytes.task.cancel()
            throw Failure.tooLarge
        }
        var data = Data()
        data.reserveCapacity(Int(min(max(http.expectedContentLength, 0), Int64(maxDownloadBytes))))
        for try await byte in bytes {
            data.append(byte)
            if data.count > maxDownloadBytes {
                bytes.task.cancel()
                throw Failure.tooLarge
            }
        }
        return data
    }

    /// Une image JPEG encodée depuis `image` (sans métadonnées).
    static func jpeg(_ image: CGImage, quality: Double) -> Data? {
        let out = NSMutableData()
        guard let dest = CGImageDestinationCreateWithData(out, "public.jpeg" as CFString, 1, nil) else { return nil }
        CGImageDestinationAddImage(dest, image, [kCGImageDestinationLossyCompressionQuality: quality] as CFDictionary)
        guard CGImageDestinationFinalize(dest) else { return nil }
        return out as Data
    }

    /// La photo réduite : plus grand côté ≤ `maxSide`, JPEG de `limit` octets au plus (qualité
    /// 0,6, puis moins, puis plus petite) ; `nil` si ce n'est pas une image lisible.
    static func reduce(_ data: Data, maxSide: Int = maxSide, limit: Int = maxStoredBytes) -> Data? {
        let sourceOptions = [kCGImageSourceShouldCache: false] as CFDictionary
        guard let source = CGImageSourceCreateWithData(data as CFData, sourceOptions),
              CGImageSourceGetCount(source) > 0 else { return nil }
        if let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
           let w = props[kCGImagePropertyPixelWidth] as? Int, let h = props[kCGImagePropertyPixelHeight] as? Int,
           w * h > maxPixels {
            return nil
        }
        for side in [maxSide, maxSide * 3 / 4, maxSide / 2, maxSide * 3 / 8] {
            let options: [CFString: Any] = [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceShouldCacheImmediately: true,
                kCGImageSourceThumbnailMaxPixelSize: side,
            ]
            guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
                return nil
            }
            for quality in [0.6, 0.5, 0.4, 0.3] {
                if let out = jpeg(image, quality: quality), out.count <= limit { return out }
            }
        }
        return nil
    }

    /// Ce qui est gardé : clés et poids total (au plus 8 éléments, lus un par un).
    static func inventory() -> (keys: [String], bytes: Int, status: OSStatus) {
        let list = SharedStore.imageKeys()
        var bytes = 0
        var status = list.status == errSecItemNotFound ? errSecSuccess : list.status
        for key in list.keys {
            let r = SharedStore.readImage(key: key)
            bytes += r.data?.count ?? 0
            if r.status != errSecSuccess { status = r.status }
        }
        return (list.keys, bytes, status)
    }

    /// Met le magasin en accord avec `items` (au plus 8) : supprime, télécharge, réduit, écrit.
    static func sync(_ items: [Item], log: @escaping @Sendable (String) -> Void) async -> Outcome {
        var outcome = Outcome()
        let wanted = Array(items.prefix(SharedStore.maxImages))
        let wantedKeys = Set(wanted.map(\.key))
        let existing = SharedStore.imageKeys()
        if existing.status != errSecSuccess, existing.status != errSecItemNotFound {
            outcome.status = existing.status
        }
        for key in existing.keys where !wantedKeys.contains(key) {
            let s = SharedStore.deleteImage(key: key)
            if s == errSecSuccess || s == errSecItemNotFound { outcome.removed += 1 } else { outcome.status = s }
        }
        let have = Set(existing.keys)
        let missing = wanted.filter { !have.contains($0.key) }
        outcome.kept = wanted.count - missing.count
        if !missing.isEmpty {
            let session = makeSession()
            defer { session.invalidateAndCancel() }
            let reduced: [(Item, Data?)] = await withTaskGroup(of: (Item, Data?).self) { group in
                for item in missing {
                    group.addTask {
                        do {
                            let raw = try await download(item.url, session: session)
                            guard let small = reduce(raw) else { throw Failure.undecodable }
                            return (item, small)
                        } catch {
                            // L'adresse n'est pas écrite dans le journal (seulement l'hôte et
                            // un code : la description d'une URLError contient l'adresse).
                            let why = (error as? URLError).map { "URLError \($0.code.rawValue)" } ?? "\(error)"
                            log("widget picture \(item.key) from \(item.url.host ?? "?"): \(why)")
                            return (item, nil)
                        }
                    }
                }
                var all: [(Item, Data?)] = []
                for await r in group { all.append(r) }
                return all
            }
            for (item, data) in reduced {
                guard let data else {
                    outcome.failed += 1
                    continue
                }
                let s = SharedStore.writeImage(data, key: item.key)
                if s == errSecSuccess {
                    outcome.written += 1
                } else {
                    outcome.failed += 1
                    outcome.status = s
                }
            }
        }
        let after = inventory()
        outcome.bytes = after.bytes
        outcome.count = after.keys.count
        if after.status != errSecSuccess { outcome.status = after.status }
        if outcome.written > 0 || outcome.removed > 0 {
            WidgetCenter.shared.reloadTimelines(ofKind: "CordeSlides")
        }
        return outcome
    }
}
