// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le module natif propre à Corde (« CordeNative » côté JavaScript, packages/ui/src/mobile/
// platform.ts) : écrit le bloc des widgets dans le trousseau partagé, y garde les photos réduites
// du widget « Courses en images », démarre, termine et liste l'activité en direct, et donne un
// diagnostic. Enregistré par CordeBridgeViewController.
// Les règles (quand démarrer, quand terminer, quelles photos) sont dans le TypeScript : ici, on
// exécute. Seule lecture du réseau : le téléchargement des photos que la partie JavaScript
// demande (WidgetImages.swift) ; rien n'est envoyé nulle part.

import ActivityKit
import Capacitor
import Foundation
import Security
import WidgetKit

@objc(CordeNativePlugin)
public class CordeNativePlugin: CAPPlugin, CAPBridgedPlugin {
    public let identifier = "CordeNativePlugin"
    public let jsName = "CordeNative"
    public let pluginMethods: [CAPPluginMethod] = [
        CAPPluginMethod(name: "setWidgetData", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "setWidgetImages", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "activities", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "startActivity", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "endActivities", returnType: CAPPluginReturnPromise),
        CAPPluginMethod(name: "diagnostics", returnType: CAPPluginReturnPromise),
    ]

    /// Une ligne du journal de l'appareil (sortie d'erreur et journal système).
    static func log(_ line: String) {
        NSLog("[corde-native] %@", line)
    }

    @objc func setWidgetData(_ call: CAPPluginCall) {
        guard let json = call.getString("json"), let data = json.data(using: .utf8) else {
            call.reject("json manquant")
            return
        }
        let status = SharedStore.write(data)
        if status == errSecSuccess {
            WidgetCenter.shared.reloadAllTimelines()
        }
        CordeNativePlugin.log("widgets: \(data.count) bytes, OSStatus \(status)")
        call.resolve([
            "ok": status == errSecSuccess,
            "status": Int(status),
            "bytes": data.count,
        ])
    }

    /// Dernier `OSStatus` du magasin des photos autre qu'un succès (diagnostic).
    private static var lastImageStatus: OSStatus = errSecSuccess

    /// Les photos du widget « Courses en images » : `items` = [{ key, url }], au plus 8. Les
    /// téléchargements se font ici, dans l'application (WidgetImages.swift), jamais dans
    /// l'extension.
    @objc func setWidgetImages(_ call: CAPPluginCall) {
        let raw = call.getArray("items", JSObject.self) ?? []
        var items: [WidgetImages.Item] = []
        var refused = 0
        for entry in raw.prefix(SharedStore.maxImages) {
            guard let key = entry["key"] as? String, SharedStore.isImageKey(key),
                  let link = entry["url"] as? String, link.count <= 500,
                  let url = URL(string: link), WidgetImages.isAllowed(url),
                  !items.contains(where: { $0.key == key }) else {
                refused += 1
                continue
            }
            items.append(WidgetImages.Item(key: key, url: url))
        }
        Task {
            var outcome = await WidgetImages.sync(items) { line in CordeNativePlugin.log(line) }
            outcome.failed += refused
            CordeNativePlugin.lastImageStatus = outcome.status
            CordeNativePlugin.log("widget pictures: \(outcome.written) written, \(outcome.kept) kept, \(outcome.failed) failed, \(outcome.removed) removed, \(outcome.count) held (\(outcome.bytes) bytes), OSStatus \(outcome.status)")
            call.resolve([
                "written": outcome.written,
                "kept": outcome.kept,
                "failed": outcome.failed,
                "removed": outcome.removed,
                "bytes": outcome.bytes,
                "status": Int(outcome.status),
            ])
        }
    }

    private static func running() -> [Activity<CordeRaceAttributes>] {
        Activity<CordeRaceAttributes>.activities.filter {
            $0.activityState == .active || $0.activityState == .stale
        }
    }

    @objc func activities(_ call: CAPPluginCall) {
        var list: JSArray = []
        for activity in CordeNativePlugin.running() {
            let item: JSObject = [
                "id": activity.id,
                "pickId": activity.attributes.pickId,
                "startMs": activity.content.state.startMs,
            ]
            list.append(item)
        }
        call.resolve([
            "allowed": ActivityAuthorizationInfo().areActivitiesEnabled,
            "running": list,
        ])
    }

    @objc func startActivity(_ call: CAPPluginCall) {
        guard ActivityAuthorizationInfo().areActivitiesEnabled else {
            call.reject("activités en direct non autorisées", "not_allowed")
            return
        }
        guard let pickId = call.getString("pickId"), let startMs = call.getDouble("startMs") else {
            call.reject("pickId ou startMs manquant")
            return
        }
        let attributes = CordeRaceAttributes(
            pickId: pickId,
            cat: call.getString("cat") ?? "daily",
            level: call.getString("level"),
            name: String((call.getString("name") ?? "").prefix(120)),
            track: String((call.getString("track") ?? "").prefix(120)),
            classes: String((call.getString("classes") ?? "").prefix(80)),
            lang: call.getString("lang") ?? "fr"
        )
        let state = CordeRaceAttributes.ContentState(startMs: startMs)
        // `staleDate` : à l'heure du départ, iOS marque l'activité périmée et la redessine
        // (« Départ donné ») sans que l'application ait à tourner.
        let content = ActivityContent(state: state, staleDate: state.start)
        do {
            let activity = try Activity.request(attributes: attributes, content: content, pushType: nil)
            CordeNativePlugin.log("live activity started")
            call.resolve(["id": activity.id])
        } catch {
            CordeNativePlugin.log("live activity refused: \(error.localizedDescription)")
            call.reject("activité refusée par iOS : \(error.localizedDescription)")
        }
    }

    @objc func endActivities(_ call: CAPPluginCall) {
        let ids = call.getArray("ids", String.self)
        let targets = CordeNativePlugin.running().filter { ids == nil || ids!.contains($0.id) }
        Task {
            for activity in targets {
                await activity.end(nil, dismissalPolicy: .immediate)
            }
            CordeNativePlugin.log("live activity: \(targets.count) ended")
            call.resolve()
        }
    }

    @objc func diagnostics(_ call: CAPPluginCall) {
        let read = SharedStore.read()
        var version = 0
        var writtenAt = 0.0
        struct Head: Decodable {
            let v: Int?
            let at: Double?
        }
        if let data = read.data, let head = try? JSONDecoder().decode(Head.self, from: data) {
            version = head.v ?? 0
            writtenAt = head.at ?? 0
        }
        let store: String
        if read.data != nil {
            store = "ok"
        } else if read.status == errSecItemNotFound {
            store = "empty"
        } else {
            store = "error"
        }
        let os = ProcessInfo.processInfo.operatingSystemVersion
        let images = WidgetImages.inventory()
        let imageStatus = images.status != errSecSuccess ? images.status : CordeNativePlugin.lastImageStatus
        call.resolve([
            "images": images.keys.count,
            "imageBytes": images.bytes,
            "imageStatus": Int(imageStatus),
            "store": store,
            "status": Int(read.status),
            // Seulement « trouvé ou non » : le nom du groupe contient l'identifiant d'équipe.
            "group": SharedStore.accessGroup != nil,
            "bytes": read.data?.count ?? 0,
            "writtenAt": writtenAt,
            "version": version,
            "activitiesAllowed": ActivityAuthorizationInfo().areActivitiesEnabled,
            "activities": CordeNativePlugin.running().count,
            "ios": "\(os.majorVersion).\(os.minorVersion).\(os.patchVersion)",
        ])
    }
}
