// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le bloc de données que l'application donne à ses widgets et à son activité en direct.
// Compilé dans les deux cibles (application et extension CordeWidgets).
//
// Où : un élément du trousseau iOS (`kSecClassGenericPassword`) dans un groupe de trousseau
// partagé par les deux cibles (`keychain-access-groups` de leurs fichiers .entitlements). Le nom
// réel du groupe commence par le préfixe de l'équipe Apple, que le code ne connaît pas : Xcode
// l'écrit dans la clé `CordeKeychainGroup` de l'Info.plist de chaque cible, lue ici.
//
// Pourquoi pas un « App Group » (UserDefaults partagés) : un App Group se crée à la main sur le
// portail d'Apple (l'API App Store Connect ne sait pas le faire) ; le partage de trousseau ne
// demande rien, les profils de provisionnement l'autorisent déjà (`<équipe>.*`). ADR 0012.
// Passer à un App Group plus tard ne toucherait que ce fichier.
//
// Le bloc ne contient aucun secret (planning public, choix du pilote, langue) : jamais d'adresse
// de serveur ni de jeton d'appairage. Accessible après le premier déverrouillage (les widgets se
// redessinent téléphone verrouillé), jamais synchronisé avec iCloud.

import Foundation
import Security

enum SharedStore {
    static let service = "io.github.dultra123.corde.widgets"
    static let account = "widget-data.v1"
    /// Au-delà, l'écriture est refusée (le TypeScript s'arrête à 48 Kio).
    static let maxBytes = 64 * 1024

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

    private static func query(group: String?) -> [String: Any] {
        var q: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: service,
            kSecAttrAccount as String: account,
            kSecAttrSynchronizable as String: kCFBooleanFalse as Any,
        ]
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

    /// Écrit le bloc (remplace le précédent). Renvoie l'`OSStatus` (0 = réussi).
    @discardableResult
    static func write(_ data: Data) -> OSStatus {
        if data.count > maxBytes { return errSecParam }
        return withFallback { group in
            let base = query(group: group)
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

    /// Lit le bloc ; `data` est `nil` quand rien n'a été écrit ou en cas d'erreur.
    static func read() -> ReadResult {
        var found: Data?
        let status = withFallback { group in
            var q = query(group: group)
            q[kSecReturnData as String] = true
            q[kSecMatchLimit as String] = kSecMatchLimitOne
            var out: CFTypeRef?
            let s = SecItemCopyMatching(q as CFDictionary, &out)
            found = s == errSecSuccess ? out as? Data : nil
            return s
        }
        return ReadResult(data: found, status: status)
    }
}
