import { type CSSProperties, useCallback, useEffect, useRef, useState } from "react";
import { SettingsDialog } from "../dashboard/SettingsDialog";
import { isBuiltin } from "../dashdef/builtin";
import { DashPage, themeStyle } from "../dashdef/Renderer";
import { useActiveDash } from "../dashdef/store";
import { type TranslationKey, useT } from "../i18n";
import { useRoute } from "../nav/routes";
import { useSettings } from "../store/settings";
import { connectionPhase, type TelemetryState } from "../store/telemetry";
import { DashMenu, pageTitle } from "./DashMenu";
import { useThrottled } from "./live";
import { type DashFlags, flagsOf, flagTone } from "./model";
import { useDashFull, useDashPage } from "./store";
import { useFullToggle } from "./view";
import { useKeepAwake } from "./wake";

const IDLE: Record<string, TranslationKey | null> = {
  connecting: "status.connecting",
  offline: "status.offline",
  "waiting-game": "status.waitingGame",
  monitor: "status.monitor",
  live: null,
  demo: null,
  "demo-auto": null,
};

interface Snapshot {
  frame: TelemetryState["frame"];
  laps: TelemetryState["laps"];
  demo: TelemetryState["demo"];
  idle: TranslationKey | null;
  flags: DashFlags;
}

function snapshot(s: TelemetryState): Snapshot {
  const player = s.frame?.player ?? null;
  return {
    frame: s.frame,
    laps: s.laps,
    demo: s.demo,
    idle: player ? null : (IDLE[connectionPhase(s)] ?? null),
    flags: flagsOf(s.frame?.session ?? null, player, s.frame?.standings ?? []),
  };
}

function prefersReducedMotion(): boolean {
  return window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
}

/**
 * The four pages side by side in a strip that scrolls horizontally and snaps on each page: the
 * finger drags it (native scrolling, momentum included), the dots, the arrows of the keyboard
 * and the menu move it. The page shown is kept in `useDashPage` (remembered on the device).
 */
function usePager(count: number) {
  const ref = useRef<HTMLDivElement>(null);
  const index = useDashPage((s) => s.index);
  const show = useDashPage((s) => s.show);
  const settled = useRef(index);

  // The store changed (dot, key, menu, debug request): scroll there.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const left = index * el.clientWidth;
    if (Math.abs(el.scrollLeft - left) < 2) return;
    const smooth = settled.current !== index && !prefersReducedMotion();
    settled.current = index;
    el.scrollTo({ left, behavior: smooth ? "smooth" : "auto" });
  }, [index]);

  // The finger moved the strip: the page it settled on becomes the current one.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let timer: ReturnType<typeof setTimeout> | null = null;
    const settle = () => {
      timer = null;
      const w = el.clientWidth;
      if (w <= 0) return;
      const i = Math.max(0, Math.min(count - 1, Math.round(el.scrollLeft / w)));
      settled.current = i;
      show(i);
    };
    const onScroll = () => {
      if (timer) clearTimeout(timer);
      timer = setTimeout(settle, 120);
    };
    el.addEventListener("scroll", onScroll, { passive: true });
    // A turn of the phone or a resized window: stay on the same page.
    const ro =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => {
            el.scrollLeft = useDashPage.getState().index * el.clientWidth;
          });
    ro?.observe(el);
    return () => {
      el.removeEventListener("scroll", onScroll);
      ro?.disconnect();
      if (timer) clearTimeout(timer);
    };
  }, [count, show]);

  return { ref, index, show };
}

/**
 * The race dashboard: a race display like the screen of a racing wheel, four pages swiped
 * with the finger. Full screen on a phone and at `#/dash`; on a PC or a tablet inside the Direct
 * page (`embedded`, under the top bar), full screen on request (the button, Esc to leave).
 */
export function DashView({ embedded = false }: { embedded?: boolean }) {
  const { t, lang } = useT();
  const def = useActiveDash();
  const pageIds = def.pages.map((p) => p.id).join("\n");
  useEffect(() => {
    useDashPage.getState().setPages(pageIds.split("\n"));
  }, [pageIds]);
  const route = useRoute();
  const canToggle = useFullToggle(route);
  const full = useDashFull((s) => s.full);
  const setFull = useDashFull((s) => s.setFull);
  // The browser left its full screen (Esc, F11): the dashboard goes back into the page.
  useEffect(() => {
    if (!full) return;
    let entered = !!document.fullscreenElement;
    const on = () => {
      if (document.fullscreenElement) entered = true;
      else if (entered) useDashFull.getState().setFull(false);
    };
    document.addEventListener("fullscreenchange", on);
    return () => document.removeEventListener("fullscreenchange", on);
  }, [full]);
  const snap = useThrottled(snapshot);
  const keepAwake = useSettings((s) => s.keepAwake);
  const [menuOpen, setMenuOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { ref, index, show } = usePager(def.pages.length);
  useKeepAwake(keepAwake);

  const onKey = useCallback(
    (e: KeyboardEvent) => {
      if (menuOpen || settingsOpen || e.altKey || e.ctrlKey || e.metaKey) return;
      if (e.key === "Escape" && useDashFull.getState().full) {
        e.preventDefault();
        useDashFull.getState().setFull(false);
        return;
      }
      const target = e.target;
      if (target instanceof Element && target.closest("input, select, textarea")) return;
      if (e.key === "ArrowRight" || e.key === "PageDown") {
        e.preventDefault();
        useDashPage.getState().step(1);
      } else if (e.key === "ArrowLeft" || e.key === "PageUp") {
        e.preventDefault();
        useDashPage.getState().step(-1);
      }
    },
    [menuOpen, settingsOpen],
  );
  useEffect(() => {
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onKey]);

  const standings = snap.frame?.standings ?? [];
  const tone = flagTone(snap.flags);
  const idle = snap.idle ? t(snap.idle) : null;
  return (
    <div
      className={`dash${embedded ? " is-embedded" : ""}`}
      data-flag={tone ?? "none"}
      style={isBuiltin(def) ? undefined : (themeStyle(def) as CSSProperties)}
    >
      <div className="dash-pages" ref={ref}>
        {def.pages.map((page, i) => (
          <section
            key={`${def.id}/${page.id}`}
            className={`dash-page is-${page.id}`}
            aria-label={pageTitle(def, page, t)}
            aria-hidden={i !== index}
            inert={i !== index ? true : undefined}
          >
            <DashPage
              def={def}
              page={page}
              lang={lang}
              data={{
                frame: snap.frame,
                laps: snap.laps,
                standings,
                flags: snap.flags,
                active: i === index,
                idle,
              }}
            />
          </section>
        ))}
      </div>
      <button
        type="button"
        className="dash-menu"
        aria-label={t("dash.menu")}
        aria-haspopup="dialog"
        onClick={() => setMenuOpen(true)}
      >
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M3 5h14M3 10h14M3 15h14" stroke="currentColor" strokeWidth="1.8" />
        </svg>
      </button>
      {canToggle && (
        <button
          type="button"
          className="dash-full"
          aria-pressed={full}
          aria-label={full ? t("dash.exitFullscreen") : t("dash.fullscreen")}
          title={full ? t("dash.exitFullscreen") : t("dash.fullscreen")}
          onClick={() => setFull(!full)}
        >
          <svg viewBox="0 0 20 20" aria-hidden="true">
            {full ? (
              <path
                d="M8 3v5H3M12 3v5h5M8 17v-5H3M12 17v-5h5"
                stroke="currentColor"
                strokeWidth="1.8"
                fill="none"
              />
            ) : (
              <path
                d="M3 8V3h5M17 8V3h-5M3 12v5h5M17 12v5h-5"
                stroke="currentColor"
                strokeWidth="1.8"
                fill="none"
              />
            )}
          </svg>
        </button>
      )}
      <div className="dash-foot">
        {snap.demo !== "off" && <span className="dash-demo">{t("dash.demo")}</span>}
        <div className="dash-dots" role="tablist" aria-label={t("dash.pages")}>
          {def.pages.map((page, i) => (
            <button
              key={page.id}
              type="button"
              role="tab"
              aria-selected={i === index}
              aria-label={pageTitle(def, page, t)}
              className={i === index ? "is-on" : undefined}
              onClick={() => show(i)}
            >
              <i />
            </button>
          ))}
        </div>
      </div>
      <DashMenu
        def={def}
        open={menuOpen}
        onClose={() => setMenuOpen(false)}
        onSettings={() => setSettingsOpen(true)}
      />
      <SettingsDialog open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </div>
  );
}
