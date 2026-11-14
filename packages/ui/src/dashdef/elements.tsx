import {
  Component,
  type ComponentType,
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  CarStripBlock,
  CurrentLapBlock,
  DeltaBlock,
  GapsBlock,
  GearBlock,
  LapRowsBlock,
  MiniTyres,
  PositionBlock,
  RevBlock,
  SectorsBlock,
} from "../dash/blocks/course";
import {
  FuelPanel,
  HybridPanel,
  SessionPanel,
  VirtualEnergyPanel,
  WeatherPanel,
} from "../dash/blocks/energy";
import { AroundMe, LastLaps, SectorTable } from "../dash/blocks/timing";
import { BrakesBlock, CORNERS, TyreCard, TyresCentre } from "../dash/blocks/tyres";
import { CoursePage } from "../dash/CoursePage";
import { EnergyPage } from "../dash/EnergyPage";
import { setData, setText } from "../dash/live";
import { Lab, type PageProps } from "../dash/parts";
import { TimingPage } from "../dash/TimingPage";
import { TyresPage } from "../dash/TyresPage";
import { useT } from "../i18n";
import { fmtClock, fmtTimeOfDay } from "../lib/format";
import type { Lang } from "../store/settings";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { getWidget, listWidgets } from "../widgets";
import { useFrameDraw } from "../widgets/shared/hooks";
import {
  boolProp,
  type ElementSpec,
  getElementSpec,
  numProp,
  registerElementSpec,
  strProp,
} from "./catalog";
import { type FieldSpec, formatValue, getField, label, unitOf } from "./fields";
import { validColor } from "./model";

/**
 * The drawing of each element type. Every string of a definition goes into the page as text
 * (React escapes it), every colour through a style property after `validColor`: nothing of a
 * definition is ever read as markup or code.
 */

export interface ElementProps {
  /** The type of the element (`widget.map`…). */
  type: string;
  props: Record<string, unknown>;
  data: PageProps;
  lang: Lang;
}

/** A colour of the definition, or the fallback (`auto`, anything not `#rrggbb`). */
function colorOr(v: unknown, fallback: string | undefined): string | undefined {
  return validColor(v) ? v : fallback;
}

function Framed({
  on,
  children,
  className,
}: {
  on: boolean;
  children: ReactNode;
  className?: string;
}) {
  return on ? (
    <section className={`dpanel del-frame${className ? ` ${className}` : ""}`}>{children}</section>
  ) : (
    <div className={`del-bare${className ? ` ${className}` : ""}`}>{children}</div>
  );
}

// ---------------------------------------------------------------- generic

function fieldLabel(f: FieldSpec | undefined, props: Record<string, unknown>, lang: Lang): string {
  const own = strProp(props, "label", "").trim();
  if (own) return own;
  if (!f) return lang === "en" ? "Unknown data" : "Donnée inconnue";
  return label(f.short ?? f.label, lang);
}

function readCtx() {
  const s = useTelemetry.getState();
  return { frame: s.frame, laps: s.laps };
}

/**
 * `draw` on each new telemetry frame, and after every render of the element: a changed setting
 * (colour, data, decimals) shows at once, even while no frame arrives — the game closed, or the
 * Studio without the demonstration race. Without this, a new colour only showed after the
 * dashboard was opened again.
 */
function useElementDraw(draw: () => void): void {
  useFrameDraw(draw);
  useLayoutEffect(() => {
    draw();
  });
}

function ValueEl({ props, data, lang }: ElementProps) {
  const f = getField(props.field);
  const ref = useRef<HTMLSpanElement | null>(null);
  const fixed = colorOr(props.color, undefined);
  const decimals = numProp(props, "decimals", -1, -1, 3);
  // The colour last written: a browser gives `style.color` back in another form (`rgb(…)`).
  const shown = useRef<string | null>(null);
  useElementDraw(() => {
    if (!data.active || !ref.current) return;
    const ctx = readCtx();
    const v = f ? f.read(ctx) : null;
    setText(ref.current, f ? formatValue(f, v, decimals < 0 ? null : decimals, lang) : "—");
    const c = fixed ?? (f?.color && typeof v === "number" ? f.color(v, ctx) : null) ?? "";
    if (shown.current !== c) {
      ref.current.style.color = c;
      shown.current = c;
    }
  });
  const unit = f && boolProp(props, "unit", true) ? unitOf(f, lang) : "";
  const align = strProp(props, "align", "left");
  return (
    <Framed on={boolProp(props, "framed", true)} className={`del-value is-${align}`}>
      <Lab>{fieldLabel(f, props, lang)}</Lab>
      <span className="num del-v">
        <span ref={ref}>—</span>
        {unit && <small>{unit}</small>}
      </span>
    </Framed>
  );
}

function BarEl({ props, data, lang }: ElementProps) {
  const f = getField(props.field);
  const fill = useRef<HTMLElement | null>(null);
  const text = useRef<HTMLSpanElement | null>(null);
  const vertical = boolProp(props, "vertical", false);
  const fixed = colorOr(props.color, undefined);
  const shown = useRef<string | null>(null);
  useElementDraw(() => {
    if (!data.active) return;
    const ctx = readCtx();
    const raw = f?.fraction ? f.read(ctx) : null;
    const v = typeof raw === "number" && Number.isFinite(raw) ? Math.max(0, Math.min(1, raw)) : 0;
    if (fill.current) {
      const tr = vertical ? `scaleY(${v.toFixed(3)})` : `scaleX(${v.toFixed(3)})`;
      if (fill.current.style.transform !== tr) fill.current.style.transform = tr;
      const c =
        fixed ??
        (f?.color && typeof raw === "number" ? f.color(raw, ctx) : null) ??
        "var(--d-text)";
      if (shown.current !== c) {
        fill.current.style.background = c;
        shown.current = c;
      }
    }
    if (text.current && f)
      setText(
        text.current,
        `${formatValue(f, typeof raw === "number" ? raw : null, null, lang)} %`,
      );
  });
  const show = boolProp(props, "showValue", true);
  const lab = strProp(props, "label", "").trim() || (f ? label(f.short ?? f.label, lang) : "—");
  return (
    <Framed
      on={boolProp(props, "framed", true)}
      className={`del-bar${vertical ? " is-vertical" : ""}`}
    >
      {(show || strProp(props, "label", "").trim()) && (
        <div className="del-bar-head">
          <Lab>{lab}</Lab>
          {show && <span className="num del-bar-v" ref={text} />}
        </div>
      )}
      <span className="del-track" aria-hidden="true">
        <i ref={fill} />
      </span>
    </Framed>
  );
}

function TextEl({ props }: ElementProps) {
  const align = strProp(props, "align", "left");
  const cls = [
    "del-text",
    `is-${align}`,
    boolProp(props, "bold", true) ? "is-bold" : "",
    boolProp(props, "upper", true) ? "is-upper" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <Framed on={boolProp(props, "framed", false)} className={cls}>
      <span style={{ color: colorOr(props.color, undefined) }}>{strProp(props, "text", "")}</span>
    </Framed>
  );
}

function PanelEl({ props }: ElementProps) {
  const style = strProp(props, "style", "panel");
  const border = colorOr(props.color, undefined);
  if (style === "line-h" || style === "line-v") {
    return (
      <div className={`del-line is-${style === "line-h" ? "h" : "v"}`} aria-hidden="true">
        <i style={{ background: border }} />
      </div>
    );
  }
  const title = strProp(props, "title", "").trim();
  return (
    <section
      className="dpanel del-panel"
      style={{ borderColor: border, background: colorOr(props.fillColor, undefined) }}
    >
      {title && <Lab>{title}</Lab>}
    </section>
  );
}

function clockText(mode: string): string {
  const s = useTelemetry.getState().frame?.session ?? null;
  if (mode === "left") return s && s.end_s > 0 ? fmtClock(Math.max(0, s.time_remaining_s)) : "—";
  if (mode === "elapsed") return s ? fmtClock(s.elapsed_s) : "—";
  if (mode === "game") return s ? fmtTimeOfDay(s.weather.time_of_day_s) : "—";
  const d = new Date();
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

const LOCAL_LABEL = { fr: "Heure", en: "Time" };

const CLOCK_LABEL: Record<string, { fr: string; en: string }> = {
  local: { fr: "Heure", en: "Time" },
  left: { fr: "Temps restant", en: "Time left" },
  elapsed: { fr: "Temps écoulé", en: "Elapsed" },
  game: { fr: "Heure en jeu", en: "Game time" },
};

function ClockEl({ props, data, lang }: ElementProps) {
  const mode = strProp(props, "mode", "local");
  const ref = useRef<HTMLSpanElement | null>(null);
  const [, tick] = useState(0);
  useFrameDraw(() => {
    if (data.active) setText(ref.current, clockText(mode));
  });
  // The local time also moves without telemetry.
  useEffect(() => {
    if (mode !== "local") return;
    const id = setInterval(() => {
      setText(ref.current, clockText(mode));
      tick((n) => n + 1);
    }, 10_000);
    return () => clearInterval(id);
  }, [mode]);
  const own = strProp(props, "label", "").trim();
  return (
    <Framed
      on={boolProp(props, "framed", true)}
      className={`del-value is-${strProp(props, "align", "left")}`}
    >
      <Lab>{own || label(CLOCK_LABEL[mode] ?? LOCAL_LABEL, lang)}</Lab>
      <span className="num del-v">
        <span ref={ref} style={{ color: colorOr(props.color, undefined) }}>
          {clockText(mode)}
        </span>
      </span>
    </Framed>
  );
}

// ---------------------------------------------------------------- blocks of Corde

const CORNER_INDEX: Record<string, number> = { fl: 0, fr: 1, rl: 2, rr: 3 };

const BLOCKS: Record<string, ComponentType<ElementProps>> = {
  "corde.rev": ({ props, data }) => (
    <RevBlock
      flags={data.flags}
      active={data.active}
      showFlags={boolProp(props, "flags", true)}
      className="is-solo"
    />
  ),
  "corde.gear": ({ props, data }) => (
    <GearBlock active={data.active} idle={data.idle} showSpeed={boolProp(props, "speed", true)} />
  ),
  "corde.position": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)}>
      <PositionBlock frame={data.frame} standings={data.standings} />
    </Framed>
  ),
  "corde.laps": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)}>
      <LapRowsBlock frame={data.frame} />
    </Framed>
  ),
  "corde.delta": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)}>
      <DeltaBlock active={data.active} />
    </Framed>
  ),
  "corde.lap-current": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)} className="is-row">
      <CurrentLapBlock frame={data.frame} active={data.active} />
    </Framed>
  ),
  "corde.sectors": ({ data }) => (
    <div className="del-bare is-row">
      <SectorsBlock frame={data.frame} />
    </div>
  ),
  "corde.gaps": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)} className="is-row">
      <GapsBlock frame={data.frame} />
    </Framed>
  ),
  "corde.car-strip": ({ data }) => <CarStripBlock frame={data.frame} laps={data.laps} />,
  "corde.mini-tyres": ({ props, data, lang }) => (
    <Framed on={boolProp(props, "framed", true)} className="del-minityres">
      <Lab>{lang === "en" ? "Tyres" : "Pneus"}</Lab>
      <MiniTyres frame={data.frame} />
    </Framed>
  ),
  "corde.tyre": ({ props, data }) => {
    const i = CORNER_INDEX[strProp(props, "corner", "fl")] ?? 0;
    const corner = CORNERS[i];
    return corner ? (
      <TyreCard wheel={data.frame?.player?.wheels[i] ?? null} corner={corner} />
    ) : null;
  },
  "corde.tyres-centre": ({ data }) => <TyresCentre frame={data.frame} />,
  "corde.brakes": ({ props, data }) => (
    <Framed on={boolProp(props, "framed", true)}>
      <BrakesBlock frame={data.frame} />
    </Framed>
  ),
  "corde.fuel": ({ data }) => <FuelPanel frame={data.frame} laps={data.laps} />,
  "corde.energy": ({ data }) => <VirtualEnergyPanel frame={data.frame} laps={data.laps} />,
  "corde.hybrid": ({ data }) => <HybridPanel frame={data.frame} />,
  "corde.session": ({ data }) => <SessionPanel frame={data.frame} laps={data.laps} />,
  "corde.weather": ({ data }) => <WeatherPanel frame={data.frame} />,
  "corde.sector-table": ({ data }) => <SectorTable frame={data.frame} />,
  "corde.last-laps": ({ props, data }) => (
    <LastLaps frame={data.frame} laps={data.laps} count={numProp(props, "count", 5, 1, 10)} />
  ),
  "corde.around": ({ props, data }) => (
    <AroundMe standings={data.standings} span={numProp(props, "span", 2, 1, 4)} />
  ),
};

const PAGES: Record<string, ComponentType<PageProps>> = {
  "corde.page.course": CoursePage,
  "corde.page.tyres": TyresPage,
  "corde.page.energy": EnergyPage,
  "corde.page.timing": TimingPage,
};

export function pageComponent(type: string): ComponentType<PageProps> | undefined {
  return PAGES[type];
}

// ---------------------------------------------------------------- widgets of the PC

/** A widget of the grid (`widgets/`) as an element: it reads the telemetry itself. */
function WidgetEl({ type, props }: ElementProps) {
  const kind = type.slice("widget.".length);
  {
    const { t } = useT();
    const def = getWidget(kind);
    if (!def) return null;
    const Comp = def.component;
    const config: Record<string, unknown> = { ...def.defaultConfig };
    for (const f of def.configFields ?? []) {
      const v = props[f.key];
      if (typeof v === "string" && f.options.some((o) => o.value === v)) config[f.key] = v;
    }
    return (
      <section className={`widget is-${kind} del-widget`} aria-label={t(def.titleKey)}>
        <header className="widget-head">
          <span className="widget-title">{t(def.titleKey)}</span>
        </header>
        <div className="widget-body">
          <Comp id={`dash-${kind}`} config={config} setConfig={() => {}} editing={false} />
        </div>
      </section>
    );
  }
}

const WIDGETS = new Map<string, ComponentType<ElementProps>>();

function registerWidgets(): void {
  const lang = useSettings.getState().lang;
  void lang;
  for (const w of listWidgets()) {
    const type = `widget.${w.id}`;
    if (WIDGETS.has(type)) continue;
    WIDGETS.set(type, WidgetEl);
    const spec: ElementSpec = {
      type,
      group: "widgets",
      name: { fr: w.titleKey, en: w.titleKey },
      size: {
        w: Math.max(4, Math.min(24, w.defaultSize.w * 2)),
        h: Math.max(3, Math.min(12, w.defaultSize.h)),
      },
      min: { w: Math.min(24, w.minW * 2), h: Math.min(12, w.minH) },
      props: (w.configFields ?? []).map((f) => ({
        key: f.key,
        label: { fr: f.labelKey, en: f.labelKey },
        type: {
          kind: "choice" as const,
          options: f.options.map((o) => ({
            value: o.value,
            label: { fr: o.labelKey, en: o.labelKey },
          })),
        },
        default: String(w.defaultConfig?.[f.key] ?? f.options[0]?.value ?? ""),
      })),
    };
    registerElementSpec(spec);
  }
}

registerWidgets();

/** The names of the widget elements are translation keys (`isKeyLabel`). */
export function isKeyLabel(spec: ElementSpec): boolean {
  return spec.group === "widgets";
}

// ---------------------------------------------------------------- lookup

const GENERIC: Record<string, ComponentType<ElementProps>> = {
  value: ValueEl,
  bar: BarEl,
  text: TextEl,
  panel: PanelEl,
  clock: ClockEl,
};

export function elementComponent(type: string): ComponentType<ElementProps> | undefined {
  return GENERIC[type] ?? BLOCKS[type] ?? WIDGETS.get(type);
}

/** A type this version does not know (a definition made by a later one). */
export function UnknownElement({ type, lang }: { type: string; lang: Lang }) {
  return (
    <div className="del-unknown">
      <span>{lang === "en" ? "Unknown element" : "Élément inconnu"}</span>
      <small>{type}</small>
    </div>
  );
}

/** Keeps a broken element from taking the whole dashboard down. */
export class ElementBoundary extends Component<
  { children: ReactNode; type: string; lang: Lang },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? (
      <UnknownElement type={this.props.type} lang={this.props.lang} />
    ) : (
      this.props.children
    );
  }
}

export function knownType(type: string): boolean {
  return (
    getElementSpec(type) !== undefined && (elementComponent(type) !== undefined || type in PAGES)
  );
}

/** A data attribute to style an element by its context (the page it comes from). */
export function contextOf(type: string): string {
  if (type.startsWith("corde.page.")) return "page";
  if (
    ["corde.fuel", "corde.energy", "corde.hybrid", "corde.session", "corde.weather"].includes(type)
  )
    return "energy";
  if (["corde.sector-table", "corde.last-laps", "corde.around"].includes(type)) return "timing";
  if (["corde.tyre", "corde.tyres-centre", "corde.brakes"].includes(type)) return "tyres";
  if (type.startsWith("corde.")) return "course";
  if (type.startsWith("widget.")) return "widget";
  return "generic";
}

export { setData };
