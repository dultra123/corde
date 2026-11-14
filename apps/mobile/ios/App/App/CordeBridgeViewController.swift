// SPDX-License-Identifier: GPL-3.0-or-later
//
// Le contrôleur de la vue web de Capacitor, avec le module natif propre à Corde
// (CordeNativePlugin) enregistré à son chargement : un module local, sans paquet npm.
// Utilisé par SceneDelegate.swift et Main.storyboard.

import Capacitor
import UIKit

class CordeBridgeViewController: CAPBridgeViewController {
    override func capacitorDidLoad() {
        bridge?.registerPluginInstance(CordeNativePlugin())
    }
}
