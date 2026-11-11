import { useEffect, useState } from "react";
import { fetchPicks, type RacePick } from "../net/races-api";
import { errorMessage } from "../sessions/shared";
import { useRaces } from "../store/races";
import { useTelemetry } from "../store/telemetry";

/** `Date.now()`, refreshed every `everyMs` while the page is visible. */
export function useNow(everyMs: number): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") setNow(Date.now());
    }, everyMs);
    return () => clearInterval(id);
  }, [everyMs]);
  return now;
}

/**
 * The driver's picks, shared by the PC and the phone through the server: read when a page needs
 * them, again every 30 s and whenever something changed (a pick toggled, a reminder received).
 */
export function usePicks(): { picks: RacePick[] | null; error: string | null } {
  const online = useTelemetry((s) => s.socket === "open");
  const revision = useRaces((s) => s.picksRevision);
  const picks = useRaces((s) => s.picks);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  useEffect(() => {
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") setTick((n) => n + 1);
    }, 30_000);
    return () => clearInterval(id);
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: revision and tick are reload triggers
  useEffect(() => {
    if (!online) return;
    const ctrl = new AbortController();
    fetchPicks(ctrl.signal)
      .then((p) => {
        if (ctrl.signal.aborted) return;
        useRaces.getState().setPicks(p);
        setError(null);
      })
      .catch((e: unknown) => {
        if (!ctrl.signal.aborted) setError(errorMessage(e));
      });
    return () => ctrl.abort();
  }, [online, revision, tick]);
  return { picks, error };
}
