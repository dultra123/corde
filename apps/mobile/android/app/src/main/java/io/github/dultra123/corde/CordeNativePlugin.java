package io.github.dultra123.corde;

import android.app.Activity;
import android.view.WindowManager;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;

/**
 * The application's own module on Android, « CordeNative » like the iPhone's
 * (packages/ui/src/mobile/platform.ts): only {@code setKeepAwake}, which keeps the screen on while
 * the phone dashboard is shown ({@code FLAG_KEEP_SCREEN_ON} on the window: no permission, and
 * Android only applies it while the window is visible). Registered by {@link MainActivity}.
 */
@CapacitorPlugin(name = "CordeNative")
public class CordeNativePlugin extends Plugin {

    @PluginMethod
    public void setKeepAwake(PluginCall call) {
        boolean on = Boolean.TRUE.equals(call.getBoolean("on", false));
        Activity activity = getActivity();
        if (activity == null) {
            call.reject("no activity");
            return;
        }
        activity.runOnUiThread(() -> {
            if (on) {
                activity.getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            } else {
                activity.getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
            }
            call.resolve();
        });
    }
}
