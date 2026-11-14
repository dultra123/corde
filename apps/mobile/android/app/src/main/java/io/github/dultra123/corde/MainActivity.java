package io.github.dultra123.corde;

import android.content.Intent;
import android.graphics.Color;
import android.os.Bundle;
import androidx.activity.EdgeToEdge;
import androidx.activity.SystemBarStyle;
import androidx.core.splashscreen.SplashScreen;
import com.getcapacitor.BridgeActivity;

/**
 * The application: Capacitor's activity, with
 * - the launch screen of Android 12 (androidx.core:core-splashscreen also draws it before);
 * - edge to edge on every version (Android 15 imposes it): transparent system bars with light
 *   icons; the interface keeps its safe margins (SystemBars of Capacitor gives the insets to
 *   the page, `env(safe-area-inset-*)`);
 * - in the Debug build only, the entries of the CI's smoke test ({@code DebugHooks}; the
 *   Release build has an empty one).
 */
public class MainActivity extends BridgeActivity {

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        SplashScreen.installSplashScreen(this);
        EdgeToEdge.enable(this, SystemBarStyle.dark(Color.TRANSPARENT), SystemBarStyle.dark(Color.TRANSPARENT));
        super.onCreate(savedInstanceState);
    }

    /** Also called by {@code BridgeActivity.load()} with the launch intent. */
    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        DebugHooks.onIntent(this, intent);
    }
}
