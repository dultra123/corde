// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le bloc de données que l'application donne à ses widgets et à son activité en direct, et les
// photos réduites du widget « Courses en images ». Compilé dans les deux cibles (application et
// extension CordeWidgets).
//
// Où : des éléments du trousseau iOS (`kSecClassGenericPassword`) dans un groupe de trousseau
// partagé par les deux cibles (`keychain-access-groups` de leurs fichiers .entitlements). Le nom
// réel du groupe commence par le préfixe de l'équipe Apple, que le code ne connaît pas : Xcode
// l'écrit dans la clé `CordeKeychainGroup` de l'Info.plist de chaque cible, lue ici.
//   - le bloc : service `io.github.dultra123.corde.widgets`, compte `widget-data.v1` ;
//   - les photos : service `io.github.dultra123.corde.widgets.img`, un élément par photo, compte =
//     la clé de son adresse (`imageKey` de widgets.ts : `i` + 16 caractères hexadécimaux), au
//     plus 8, chacune un JPEG de 48 Kio au plus (réduit par l'application, WidgetImages.swift).
//
// Pourquoi pas un « App Group » (UserDefaults ou dossier partagés) : un App Group se crée à la
// main sur le portail d'Apple (l'API App Store Connect ne sait pas le faire) ; le partage de
// trousseau ne demande rien, les profils de provisionnement l'autorisent déjà (`<équipe>.*`).
// ADR 0012. Passer à un App Group plus tard ne toucherait que ce fichier. Limite assumée pour les
// photos : le trousseau n'est pas fait pour des images (aucune taille maximale documentée, les
// ennuis connus commencent vers le mégaoctet ; on reste à 8 × 48 Kio) et ses éléments survivent
// à la désinstallation de l'application.
//
// Dans les droits de l'application, le groupe partagé vient en second, après le groupe propre à
// l'application (`<équipe>.io.github.dultra123.corde`) : iOS range dans le PREMIER groupe de la
// liste tout élément écrit sans groupe. Le groupe partagé ne reçoit donc que ce qui est écrit ici
// avec son groupe nommé — jamais, par défaut, autre chose que l'application rangerait un jour dans
// le trousseau.
//
// Rien de secret ici (planning public, choix du pilote, langue, photos publiques des courses) :
// jamais d'adresse de serveur ni de jeton d'appairage. Accessible après le premier déverrouillage
// (les widgets se redessinent téléphone verrouillé), jamais synchronisé avec iCloud.

import Foundation
import Security

enum SharedStore {
    static let service = "io.github.dultra123.corde.widgets"
    static let account = "widget-data.v1"
    /// Les photos du widget « Courses en images » (un élément par photo, compte = sa clé).
    static let imageService = "io.github.dultra123.corde.widgets.img"
    /// Au-delà, l'écriture est refusée (le TypeScript s'arrête à 48 Kio).
    static let maxBytes = 64 * 1024
    /// Au-delà, une photo est refusée (la réduction s'arrête à 48 Kio).
    static let maxImageBytes = 64 * 1024
    /// Photos gardées au plus.
    static let maxImages = 8

    struct ReadResult {
        let data: Data?
        /// `errSecSuccess`, `errSecItemNotFound` (rien d'écrit), ou une erreur.
        let status: OSStatus
    }

    /// Le groupe de trousseau partagé, lu dans l'Info.plist ; `nil` s'il manque.
    static var accessGroup: String? {
        guard let value = Bundle.main.object(forInfoDictionaryKey: "CordeKeychainGroup") as? String else {
            return nil
        }
        let group = value.trimmingCharacters(in: .whitespaces)
        if group.isEmpty || group.contains("$(") { return nil }
        // Sans équipe (simulateur sans signature), le préfixe est vide : « .io.github… ».
        return group.hasPrefix(".") ? String(group.dropFirst()) : group
    }

    /// Une clé de photo valable : `i` suivi de 16 caractères hexadécimaux minuscules.
    static func isImageKey(_ key: String) -> Bool {
        key.count == 17 && key.hasPrefix("i")
            && key.dropFirst().allSatisfy { ("0" ... "9").contains($0) || ("a" ... "f").contains($0) }
    }

    private static func query(service: String, account: String?, group: String?) -> [String: Any] {
        var q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]
        if let account { q[kSecAttrAccount as String] = account }
        if let group { q[kSecAttrAccessGroup as String] = group }
        return q
    }

    /// Sur le simulateur compilé sans droits, le groupe partagé est refusé
    /// (`errSecMissingEntitlement`) : on retombe sur le trousseau propre à l'application, pour
    /// que l'essai de fumée exerce quand même écriture et lecture. Jamais sur un iPhone.
    private static func withFallback(_ run: (String?) -> OSStatus) -> OSStatus {
        let status = run(accessGroup)
        #if targetEnvironment(simulator)
        if status == errSecMissingEntitlement {
            return run(nil)
        }
        #endif
        return status
    }

    private static func writeItem(service: String, account: String, data: Data) -> OSStatus {
        withFallback { group in
            let base = query(service: service, account: account, group: group)
            let attributes: [String: Any] = [
                kSecValueData as String: data,
                kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlock,
            ]
            var status = SecItemUpdate(base as CFDictionary, attributes as CFDictionary)
            if status == errSecItemNotFound {
                var add = base
                for (key, value) in attributes { add[key] = value }
                status = SecItemAdd(add as CFDictionary, nil)
            }
            return status
        }
    }

    private static func readItem(service: String, account: String) -> ReadResult {
        var found: Data?
        let status = withFallback { group in
            var q = query(service: service, account: account, group: group)
            q[kSecReturnData as String] = true
            q[kSecMatchLimit as String] = kSecMatchLimitOne
            var out: CFTypeRef?
            let s = SecItemCopyMatching(q as CFDictionary, &out)
            found = s == errSecSuccess ? out as? Data : nil
            return s
        }
        return ReadResult(data: found, status: status)
    }

    // MARK: - Le bloc

    /// Écrit le bloc (remplace le précédent). Renvoie l'`OSStatus` (0 = réussi).
    @discardableResult
    static func write(_ data: Data) -> OSStatus {
        if data.count > maxBytes { return errSecParam }
        return writeItem(service: service, account: account, data: data)
    }

    /// Lit le bloc ; `data` est `nil` quand rien n'a été écrit ou en cas d'erreur.
    static func read() -> ReadResult {
        readItem(service: service, account: account)
    }

    // MARK: - Les photos du widget « Courses en images »

    /// Écrit une photo réduite sous sa clé (remplace la précédente).
    @discardableResult
    static func writeImage(_ data: Data, key: String) -> OSStatus {
        guard isImageKey(key), data.count <= maxImageBytes else { return errSecParam }
        return writeItem(service: imageService, account: key, data: data)
    }

    /// Lit une photo ; `data` est `nil` quand elle n'existe pas ou en cas d'erreur.
    static func readImage(key: String) -> ReadResult {
        guard isImageKey(key) else { return ReadResult(data: nil, status: errSecParam) }
        return readItem(service: imageService, account: key)
    }

    /// Supprime une photo (`errSecItemNotFound` si elle n'existait pas).
    @discardableResult
    static func deleteImage(key: String) -> OSStatus {
        withFallback { group in
            SecItemDelete(query(service: imageService, account: key, group: group) as CFDictionary)
        }
    }

    /// Les clés des photos gardées (sans leurs données : iOS refuse de rendre les données de
    /// plusieurs éléments à la fois). `status` vaut `errSecItemNotFound` quand il n'y en a aucune.
    static func imageKeys() -> (keys: [String], status: OSStatus) {
        var keys: [String] = []
        let status = withFallback { group in
            var q = query(service: imageService, account: nil, group: group)
            q[kSecReturnAttributes as String] = true
            q[kSecMatchLimit as String] = kSecMatchLimitAll
            var out: CFTypeRef?
            let s = SecItemCopyMatching(q as CFDictionary, &out)
            if s == errSecSuccess, let items = out as? [[String: Any]] {
                keys = items.compactMap { $0[kSecAttrAccount as String] as? String }.sorted()
            }
            return s
        }
        return (keys, status)
    }
}
