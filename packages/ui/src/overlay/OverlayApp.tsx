import type { DashboardLayout } from "@corde/protocol";
import { type CSSProperties, useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { DEFAULT_OVERLAY_ID, defaultOverlayLayout } from "../dashboard/defaults";
import { WidgetBoundary } from "../dashboard/GridItem";
import { useT } from "../i18n";
import { connection } from "../net/connection";
import { isLayout } from "../net/layouts-api";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { getWidget } from "../widgets";
import { BOOT_OPTIONS } from "./boot";
import {
  fitFactor,
  isOnTrack,
  layoutCanvas,
  OFFTRACK_GRACE_MS,
  type OverlayOptions,
  overlayShown,
  parseOverlayOptions,
  parseOverlayTarget,
} from "./model";

/** How often an overlay layout is read again (the streamer may be editing it in Corde). */
const LAYOUT_REFRESH_MS = 10_000;
/** Inset of a widget in its cells, like the dashboard grid (gridstack margin). */
const ITEM_INSET = 3;

function subscribeHash(cb: () => void): () => void {
  window.addEventListener("hashchange", cb);
  return () => window.removeEventListener("hashchange", cb);
}

function useHash(): string {
  return useSyncExternalStore(subscribeHash, () => window.location.hash);
}

function useViewport(): { width: number; height: number } {
  const [size, setSize] = useState(() => ({
    width: window.innerWidth,
    height: window.innerHeight,
  }));
  useEffect(() => {
    const on = () => setSize({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  return size;
}

/**
 * `hide=offtrack`: shown while the driver is on track, faded out {@link OFFTRACK_GRACE_MS} after
 * they left it. Re-renders only when the answer changes, not on every frame.
 */
function useShown(hideOffTrack: boolean): boolean {
  const onTrack = useTelemetry(isOnTrack);
  const [offSince, setOffSince] = useState<number | null>(null);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (onTrack) return () => setOffSince(Date.now());
    return undefined;
  }, [onTrack]);
  useEffect(() => {
    if (!hideOffTrack || onTrack || offSince === null) return;
    const left = OFFTRACK_GRACE_MS - (Date.now() - offSince);
    if (left <= 0) return;
    const timer = setTimeout(() => setNow(Date.now()), left + 20);
    return () => clearTimeout(timer);
  }, [hideOffTrack, onTrack, offSince]);
  return overlayShown(hideOffTrack, onTrack, offSince, Math.max(now, Date.now()));
}

/** A widget's settings: its defaults, then the `w.<key>` options it actually offers. */
function widgetConfig(
  kind: string,
  base: Record<string, unknown>,
  fromUrl: Record<string, string>,
): Record<string, unknown> {
  const def = getWidget(kind);
  const out: Record<string, unknown> = { ...(def?.defaultConfig ?? {}), ...base };
  for (const field of def?.configFields ?? []) {
    const v = fromUrl[field.key];
    if (v !== undefined && field.options.some((o) => o.value === v)) out[field.key] = v;
  }
  return out;
}

const noop = () => {};

/** One widget without the dashboard chrome: no handle, no button, scaled by `scale`. */
function OverlayPanel({
  id,
  kind,
  config,
  options,
}: {
  id: string;
  kind: string;
  config: Record<string, unknown>;
  options: OverlayOptions;
}) {
  const { t } = useT();
  const def = getWidget(kind);
  if (!def) return null;
  const Comp = def.component;
  const title = t(def.titleKey);
  const s = options.scale;
  const style: CSSProperties =
    s === 1 ? {} : { width: `${100 / s}%`, height: `${100 / s}%`, transform: `scale(${s})` };
  return (
    <div className="ov-scale" style={style}>
      <section className={`widget is-${kind}`} aria-label={title}>
        {options.title && (
          <header className="widget-head">
            <span className="widget-title">{title}</span>
          </header>
        )}
        <div className="widget-body">
          <WidgetBoundary label={title}>
            <Comp id={id} config={config} setConfig={noop} editing={false} />
          </WidgetBoundary>
        </div>
      </section>
    </div>
  );
}

function OverlayWidget({ kind, options }: { kind: string; options: OverlayOptions }) {
  const config = useMemo(() => widgetConfig(kind, {}, options.widget), [kind, options.widget]);
  return (
    <div className="ov-fill">
      <OverlayPanel id={`overlay-${kind}`} kind={kind} config={config} options={options} />
    </div>
  );
}

/**
 * One layout of the server, `"missing"` when it has none of that id, `null` when the server
 * does not answer. Reads the list (`GET /api/layouts`, a few kilobytes) rather than the layout
 * itself: an unknown id then does not log a 404 in the console of OBS every ten seconds.
 */
async function fetchLayout(id: string): Promise<DashboardLayout | "missing" | null> {
  try {
    const res = await fetch(`${connection.endpoint.http}/api/layouts`, {
      headers: { Accept: "application/json" },
    });
    if (!res.ok) return null;
    const json: unknown = await res.json();
    if (!Array.isArray(json)) return null;
    const found = json.find((l: unknown) => isLayout(l) && l.id === id);
    return found && isLayout(found) ? found : "missing";
  } catch {
    return null;
  }
}

/**
 * Reads the layout from the server (read only: an overlay never writes) when it opens, on every
 * reconnection and every {@link LAYOUT_REFRESH_MS}; keeps the last copy when the server is away.
 * The default overlay layout is built in, so its address works before the first sync.
 */
function useLayout(id: string): DashboardLayout | null {
  const [layout, setLayout] = useState<DashboardLayout | null>(null);
  const hello = useTelemetry((s) => s.hello);
  // biome-ignore lint/correctness/useExhaustiveDependencies: `hello` re-reads after a reconnection
  useEffect(() => {
    let alive = true;
    const load = async () => {
      const got = await fetchLayout(id);
      if (!alive) return;
      if (got === "missing") setLayout(id === DEFAULT_OVERLAY_ID ? defaultOverlayLayout() : null);
      else if (got) {
        setLayout((prev) =>
          prev && prev.id === got.id && prev.updated_ms === got.updated_ms ? prev : got,
        );
      }
    };
    void load();
    const timer = setInterval(load, LAYOUT_REFRESH_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, [id, hello]);
  return layout;
}

function OverlayLayout({ id, options }: { id: string; options: OverlayOptions }) {
  const layout = useLayout(id);
  const vp = useViewport();
  if (!layout) return null;
  const canvas = layoutCanvas(layout);
  const fit = fitFactor(canvas, vp.width, vp.height);
  return (
    <div
      className="ov-canvas"
      style={{ width: canvas.width, height: canvas.height, transform: `scale(${fit})` }}
    >
      {layout.widgets.map((w) => (
        <div
          key={w.id}
          className="ov-item"
          style={{
            left: w.x * canvas.cellW + ITEM_INSET,
            top: w.y * canvas.cellH + ITEM_INSET,
            width: w.w * canvas.cellW - 2 * ITEM_INSET,
            height: w.h * canvas.cellH - 2 * ITEM_INSET,
          }}
        >
          <OverlayPanel
            id={w.id}
            kind={w.kind}
            config={widgetConfig(w.kind, w.config, {})}
            options={options}
          />
        </div>
      ))}
    </div>
  );
}

/**
 * The page of an OBS browser source: transparent, no navigation, no banner, no control, no
 * cursor, no scroll bar — the widgets alone. See `overlay/model.ts` for the addresses and
 * options, `styles/overlay.css` for the looks.
 */
export function OverlayApp() {
  const hash = useHash();
  const target = parseOverlayTarget(hash);
  const options = useMemo(() => parseOverlayOptions(window.location.search, hash), [hash]);
  const shown = useShown(options.hideOffTrack);

  // The pixel ratio follows the scale the page was opened with (`boot.ts`): reload on a change.
  useEffect(() => {
    if (BOOT_OPTIONS && options.scale !== BOOT_OPTIONS.scale) window.location.reload();
  }, [options.scale]);

  // `lang=` applies to this page only: not saved, the app keeps its own language.
  useEffect(() => {
    if (options.lang) useSettings.setState({ lang: options.lang });
  }, [options.lang]);

  useEffect(() => {
    document.title = target ? `Corde · ${target.id}` : "Corde";
  }, [target]);

  return (
    <div
      className={`ov is-bg-${options.bg}${shown ? "" : " is-hidden"}`}
      style={{ "--ov-opacity": String(options.opacity) } as CSSProperties}
    >
      {target?.kind === "widget" && <OverlayWidget kind={target.id} options={options} />}
      {target?.kind === "layout" && <OverlayLayout id={target.id} options={options} />}
    </div>
  );
}
