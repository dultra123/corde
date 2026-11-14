package io.github.dultra123.corde;

import android.content.Intent;
import com.getcapacitor.BridgeActivity;

/**
 * Release build: no test entry (the Debug build's one is in src/debug). An intent's extras
 * change nothing here.
 */
final class DebugHooks {

    private DebugHooks() {}

    static void onIntent(BridgeActivity activity, Intent intent) {}
}
