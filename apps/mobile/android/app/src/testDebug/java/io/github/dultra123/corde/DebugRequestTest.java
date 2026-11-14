package io.github.dultra123.corde;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import org.junit.Test;

/** The smoke test's requests become the events packages/ui/src/mobile/android.ts reads. */
public class DebugRequestTest {

    @Test
    public void reminderDelayIsBounded() {
        assertEquals("{\"action\":\"reminder\",\"delaySec\":60}", DebugRequest.eventData("reminder", 60, null));
        assertEquals("{\"action\":\"reminder\",\"delaySec\":10}", DebugRequest.eventData("reminder", 1, null));
        assertEquals("{\"action\":\"reminder\",\"delaySec\":600}", DebugRequest.eventData("reminder", 99999, null));
    }

    @Test
    public void fetchTakesOnlyClearTextAddresses() {
        assertEquals(
            "{\"action\":\"fetch\",\"url\":\"http://10.0.2.2:8471/api/health\"}",
            DebugRequest.eventData("fetch", 0, "http://10.0.2.2:8471/api/health")
        );
        assertNull(DebugRequest.eventData("fetch", 0, "https://example.com/"));
        assertNull(DebugRequest.eventData("fetch", 0, null));
    }

    @Test
    public void unknownActionsAreRefused() {
        assertEquals("{\"action\":\"scan\"}", DebugRequest.eventData("scan", 0, null));
        assertNull(DebugRequest.eventData("pick", 0, null));
        assertNull(DebugRequest.eventData("", 0, null));
    }

    @Test
    public void quotedStringsCannotLeaveTheLiteral() {
        assertEquals("\"a\\\"b\\\\c\"", DebugRequest.quote("a\"b\\c"));
        assertEquals("\"\\u003c/script\\u003e\\u2028\"", DebugRequest.quote("</script>\u2028"));
    }
}
