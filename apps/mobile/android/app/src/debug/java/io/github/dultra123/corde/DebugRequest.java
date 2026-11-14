package io.github.dultra123.corde;

import java.util.Locale;

/**
 * The request of the CI's smoke test, as the event the page receives (`cordeDebug`, read by
 * {@code parseDebugRequest} in packages/ui/src/mobile/android.ts). Debug build only.
 */
final class DebugRequest {

    private DebugRequest() {}

    /**
     * The event's data (a JavaScript object literal, JSON) for {@code action}, or {@code null}
     * for an unknown action. {@code reminder}: in {@code delaySec} seconds (10 to 600);
     * {@code fetch}: {@code url}, {@code http://} only; {@code scan}.
     */
    static String eventData(String action, int delaySec, String url) {
        if ("reminder".equals(action)) {
            int d = Math.max(10, Math.min(600, delaySec));
            return "{\"action\":\"reminder\",\"delaySec\":" + d + "}";
        }
        if ("fetch".equals(action)) {
            if (url == null || !url.startsWith("http://")) return null;
            return "{\"action\":\"fetch\",\"url\":" + quote(url) + "}";
        }
        if ("scan".equals(action)) {
            return "{\"action\":\"scan\"}";
        }
        return null;
    }

    /** A JSON string (also a safe JavaScript string: `<`, `>`, `&` and the line separators escaped). */
    static String quote(String s) {
        StringBuilder b = new StringBuilder(s.length() + 2).append('"');
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            switch (c) {
                case '"':
                    b.append("\\\"");
                    break;
                case '\\':
                    b.append("\\\\");
                    break;
                default:
                    if (c < 0x20 || c == '<' || c == '>' || c == '&' || c == ' ' || c == ' ') {
                        b.append(String.format(Locale.ROOT, "\\u%04x", (int) c));
                    } else {
                        b.append(c);
                    }
            }
        }
        return b.append('"').toString();
    }
}
