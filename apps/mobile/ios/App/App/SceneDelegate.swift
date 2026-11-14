import Capacitor
import UIKit

class SceneDelegate: UIResponder, UIWindowSceneDelegate {
    var window: UIWindow?

    func scene(_ scene: UIScene, willConnectTo session: UISceneSession, options connectionOptions: UIScene.ConnectionOptions) {
        guard let windowScene = scene as? UIWindowScene else { return }

        window = UIWindow(windowScene: windowScene)
        // Contrôleur de Capacitor avec le module natif de Corde (widgets, activité en direct).
        window?.rootViewController = CordeBridgeViewController()
        window?.makeKeyAndVisible()

        // Lancement à froid par un lien (`corde://races`, toucher d'un widget) : Capacitor garde
        // l'adresse et la transmet quand les modules sont chargés (événement `appUrlOpen` et
        // `App.getLaunchUrl()`).
        SceneDelegateProxy.shared.scene(scene, willConnectTo: session, options: connectionOptions)

        #if DEBUG
        CordeDebug.sceneDidConnect()
        #endif
    }

    // Lien reçu application ouverte (à chaud).
    func scene(_ scene: UIScene, openURLContexts URLContexts: Set<UIOpenURLContext>) {
        SceneDelegateProxy.shared.scene(scene, openURLContexts: URLContexts)
    }

    func scene(_ scene: UIScene, continue userActivity: NSUserActivity) {
        SceneDelegateProxy.shared.scene(scene, continue: userActivity)
    }
}
