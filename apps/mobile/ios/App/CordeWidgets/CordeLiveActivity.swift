// SPDX-License-Identifier: GPL-3.0-or-later
//
// L'activité en direct de Corde : compte à rebours du prochain départ choisi, sur l'écran
// verrouillé et dans la Dynamic Island. Démarrée et terminée par l'application seulement
// (règles dans packages/ui/src/mobile/widgets.ts, `planActivity`) ; aucune mise à jour à
// distance. Vues dans Shared/ActivityViews.swift.

import ActivityKit
import SwiftUI
import WidgetKit

struct CordeRaceLiveActivity: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for: CordeRaceAttributes.self) { context in
            let model = ActivityModel(attributes: context.attributes, start: context.state.start, stale: context.isStale)
            ActivityLockView(model: model, now: Date())
                .activityBackgroundTint(Color(hex: 0x0B0E12).opacity(0.86))
                .activitySystemActionForegroundColor(.white)
                .environment(\.colorScheme, .dark)
                .widgetURL(URL(string: "corde://races"))
        } dynamicIsland: { context in
            let model = ActivityModel(attributes: context.attributes, start: context.state.start, stale: context.isStale)
            return DynamicIsland {
                DynamicIslandExpandedRegion(.leading) {
                    IslandLeading(model: model)
                }
                DynamicIslandExpandedRegion(.trailing) {
                    IslandTrailing(model: model, now: Date())
                }
                DynamicIslandExpandedRegion(.center) {
                    IslandCenter(model: model)
                }
                DynamicIslandExpandedRegion(.bottom) {
                    IslandBottom(model: model)
                }
            } compactLeading: {
                IslandCompactLeading(model: model)
            } compactTrailing: {
                IslandCompactTrailing(model: model, now: Date())
            } minimal: {
                IslandMinimal(model: model, now: Date())
            }
            .widgetURL(URL(string: "corde://races"))
            .keylineTint(model.color)
        }
    }
}
