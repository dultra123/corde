package io.github.dultra123.corde;

import android.content.Intent;
import android.os.Handler;
import android.os.Looper;
import android.util.Log;
import android.webkit.WebView;
import com.getcapacitor.Bridge;
import com.getcapacitor.BridgeActivity;
import com.getcapacitor.WebViewListener;

/**
 * Debug build only (src/release has an empty one): the entries of the CI's smoke test
 * (.github/workflows/android.yml, apps/mobile/scripts/android-smoke.sh), given as extras of the
 * launch intent:
 *
 * <pre>
 * am start -n io.github.dultra123.corde/.MainActivity --es cordeDebug reminder --ei cordeDelay 60
 * am start -n io.github.dultra123.corde/.MainActivity --es cordeDebug fetch --es cordeUrl http://10.0.2.2:8471/api/health
 * am start -n io.github.dultra123.corde/.MainActivity --es cordeDebug scan
 * </pre>
 *
 * Each one becomes a {@code cordeDebug} event of the page, once the page is loaded and the
 * interface started (it listens from boot.ts); the page writes what happened to the log.
 */
final class DebugHooks {

    static final String EXTRA_ACTION = "cordeDebug";
    static final String EXTRA_DELAY = "cordeDelay";
    static final String EXTRA_URL = "cordeUrl";
    private static final String TAG = "CordeDebug";
    /** After the page is loaded, the interface still starts (boot.ts): leave it the time. */
    private static final long AFTER_LOAD_MS = 4000;

    private DebugHooks() {}

    static void onIntent(BridgeActivity activity, Intent intent) {
        if (intent == null) return;
        String action = intent.getStringExtra(EXTRA_ACTION);
        if (action == null) return;
        // Once: not again when the activity is created again with the same intent.
        intent.removeExtra(EXTRA_ACTION);
        String data = DebugRequest.eventData(action, intent.getIntExtra(EXTRA_DELAY, 60), intent.getStringExtra(EXTRA_URL));
        Bridge bridge = activity.getBridge();
        if (data == null || bridge == null) {
            Log.w(TAG, "request refused: " + action);
            return;
        }
        Handler main = new Handler(Looper.getMainLooper());
        Runnable send = () -> {
            Log.i(TAG, "request " + action);
            bridge.triggerWindowJSEvent("cordeDebug", data);
        };
        WebView webView = bridge.getWebView();
        if (webView != null && webView.getProgress() >= 100 && webView.getUrl() != null) {
            // The application was already running (singleTask): the page is there.
            main.postDelayed(send, AFTER_LOAD_MS);
            return;
        }
        bridge.addWebViewListener(
            new WebViewListener() {
                private boolean sent = false;

                @Override
                public void onPageLoaded(WebView view) {
                    if (sent) return;
                    sent = true;
                    main.postDelayed(send, AFTER_LOAD_MS);
                }
            }
        );
    }
}
