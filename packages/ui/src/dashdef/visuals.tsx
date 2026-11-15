import { type CSSProperties, useEffect, useRef, useState } from "react";
import { setData, setText } from "../dash/live";
import { LED_FULL, LED_START, SHIFT_AT } from "../dash/model";
import { Lab } from "../dash/parts";
import { boolProp, numProp, strProp } from "./catalog";
import {
  colorOr,
  type ElementProps,
  readCtx,
  useElementDraw,
  useMediaMotion,
  usePageVisible,
  useReducedMotion,
} from "./draw";
import { formatValue, getField, label, unitOf } from "./fields";
import { kindOfId, useMediaSrc } from "./media";
import { FITS, type Fit, validAssetId } from "./model";
import { thresholdColor } from "./rules";

/**
 * The elements of ADR 0016: a picture, a video, a round gauge, a shape, an alert banner and a
 * rev ramp of its own. Every colour of a setting is written at render (it shows at once, game
 * closed and demonstration stopped); only what follows the data is drawn on each frame.
 */

function fitOf(v: unknown): Fit {
  return typeof v === "string" && (FITS as readonly string[]).includes(v) ? (v as Fit) : "contain";
}

/** The place of a media that is not there (PC off, file removed): discreet, never an error. */
function MediaMissing({ cited }: { cited: boolean }) {
  return (
    <div className={`del-media-missing${cited ? " is-cited" : ""}`} aria-hidden="true">
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path
          d="M4 6h16v12H4zM4 15l5-5 4 4 2-2 5 5"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.4"
        />
      </svg>
    </div>
  );
}

/**
 * A picture or a video of the PC, by its id. A video: muted, looped, without controls, played
 * only while its page is on screen and the page itself is (a tab or the application in the
 * background pauses it); `prefers-reduced-motion`, or a small preview: its first image only.
 */
export function MediaView({
  id,
  fit,
  opacity,
  active,
  className,
}: {
  id: unknown;
  fit: Fit;
  opacity?: number;
  active: boolean;
  className?: string;
}) {
  const src = useMediaSrc(id);
  const [failed, setFailed] = useState<string | null>(null);
  const kind = kindOfId(id);
  const motion = useMediaMotion();
  const reduced = useReducedMotion();
  const visible = usePageVisible();
  const video = useRef<HTMLVideoElement>(null);
  const still = !motion || reduced;
  const play = kind === "video" && !still && active && visible;
  // biome-ignore lint/correctness/useExhaustiveDependencies: a new address is a new video element to start
  useEffect(() => {
    const v = video.current;
    if (!v) return;
    // `muted` as a property too: a browser only lets a muted video start by itself.
    v.muted = true;
    v.defaultMuted = true;
    if (play) {
      const p = v.play?.();
      if (p && typeof p.catch === "function") p.catch(() => {});
    } else {
      v.pause?.();
    }
  }, [play, src]);
  const style: CSSProperties = {
    objectFit: fit,
    ...(opacity !== undefined && opacity < 1 ? { opacity } : {}),
  };
  if (!src || !kind || failed === src) return <MediaMissing cited={validAssetId(id)} />;
  const cls = `del-media${className ? ` ${className}` : ""}`;
  if (kind === "image") {
    return (
      <img
        className={cls}
        src={src}
        alt=""
        draggable={false}
        decoding="async"
        style={style}
        onError={() => setFailed(src)}
      />
    );
  }
  return (
    <video
      ref={video}
      className={cls}
      // The first image only: a fragment the browser understands, the server never sees it.
      src={still ? `${src}#t=0.001` : src}
      style={style}
      muted
      loop
      playsInline
      autoPlay={play}
      preload={still ? "metadata" : "auto"}
      controls={false}
      disablePictureInPicture
      disableRemotePlayback
      onError={() => setFailed(src)}
    />
  );
}

export function ImageEl({ props, data }: ElementProps) {
  return (
    <div className="del-mediabox">
      <MediaView id={props.asset} fit={fitOf(props.fit)} active={data.active} />
    </div>
  );
}

// ---------------------------------------------------------------- round gauge

/** A 270° arc, open at the bottom; `pathLength` 100 so that the value is a dash length. */
const ARC = "M 21.716 78.284 A 40 40 0 1 1 78.284 78.284";

export function ArcEl({ props, data, lang, thresholds }: ElementProps) {
  const f = getField(props.field);
  const fill = useRef<SVGPathElement | null>(null);
  const text = useRef<HTMLSpanElement | null>(null);
  const fixed = colorOr(props.color, undefined);
  const min = numProp(props, "min", 0);
  const max = numProp(props, "max", 300);
  const decimals = numProp(props, "decimals", -1, -1, 3);
  const showValue = boolProp(props, "showValue", true);
  const shown = useRef<string | null>(null);
  useElementDraw(() => {
    if (!data.active) return;
    const ctx = readCtx();
    const raw = f ? f.read(ctx) : null;
    const v = typeof raw === "number" && Number.isFinite(raw) ? raw : null;
    // A fraction (throttle, fuel…) is 0 to 100 %; the revs go to the car's own maximum when
    // the maximum is left at 0.
    let lo = min;
    let hi = max;
    if (f?.fraction) {
      lo = 0;
      hi = 1;
    } else if (f?.id === "rpm" && hi <= lo) {
      hi = ctx.frame?.player?.engine.max_rpm ?? 0;
    }
    const frac = v === null || !(hi > lo) ? 0 : Math.max(0, Math.min(1, (v - lo) / (hi - lo)));
    if (fill.current) {
      const dash = `${(frac * 100).toFixed(2)} 100`;
      if (fill.current.getAttribute("stroke-dasharray") !== dash)
        fill.current.setAttribute("stroke-dasharray", dash);
      const c =
        thresholdColor(thresholds, v) ??
        fixed ??
        (f?.color && v !== null ? f.color(v, ctx) : null) ??
        "var(--accent)";
      if (shown.current !== c) {
        fill.current.style.stroke = c;
        shown.current = c;
      }
    }
    if (text.current)
      setText(text.current, f ? formatValue(f, v, decimals < 0 ? null : decimals, lang) : "—");
  });
  const lab = strProp(props, "label", "").trim() || (f ? label(f.short ?? f.label, lang) : "—");
  const unit = f && boolProp(props, "unit", true) ? unitOf(f, lang) : "";
  return (
    <div className="del-arc">
      <svg viewBox="0 0 100 100" aria-hidden="true" preserveAspectRatio="xMidYMid meet">
        <path className="del-arc-track" d={ARC} pathLength={100} />
        <path
          className="del-arc-fill"
          ref={fill}
          d={ARC}
          pathLength={100}
          strokeDasharray="0 100"
          style={{ stroke: fixed ?? "var(--accent)" }}
        />
      </svg>
      <div className="del-arc-centre">
        {showValue && (
          <span className="num del-arc-v">
            <span ref={text}>—</span>
            {unit && <small>{unit}</small>}
          </span>
        )}
        <Lab>{lab}</Lab>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------- shape

export function ShapeEl({ props }: ElementProps) {
  const shape = strProp(props, "shape", "rect");
  const color = colorOr(props.fillColor, "var(--accent)");
  const thick = numProp(props, "thickness", 3, 1, 20);
  if (shape === "line-h" || shape === "line-v") {
    const size = `max(1px, calc(var(--su) * ${thick * 0.25}))`;
    return (
      <div className={`del-shape-line is-${shape === "line-h" ? "h" : "v"}`} aria-hidden="true">
        <i
          style={{
            background: color,
            ...(shape === "line-h" ? { height: size } : { width: size }),
          }}
        />
      </div>
    );
  }
  return (
    <div
      className={`del-shape is-${shape === "circle" ? "circle" : "rect"}`}
      aria-hidden="true"
      style={{ background: color }}
    />
  );
}

// ---------------------------------------------------------------- alert banner

export function AlertEl({ props }: ElementProps) {
  const bg = colorOr(props.fillColor, "#e5484d");
  const fg = colorOr(props.color, "#ffffff");
  const blink = boolProp(props, "blink", true);
  return (
    <div
      className={`del-alert${blink ? " is-blink" : ""}`}
      role="status"
      style={{ background: bg, color: fg }}
    >
      <span>{strProp(props, "text", "")}</span>
    </div>
  );
}

// ---------------------------------------------------------------- rev ramp of its own

export function rampLit(rpm: number, maxRpm: number, segments: number, start: number): number {
  if (!(maxRpm > 0) || !Number.isFinite(rpm)) return 0;
  const f = rpm / maxRpm;
  if (f <= start) return 0;
  if (f >= LED_FULL) return segments;
  return Math.min(segments, Math.ceil(((f - start) / (LED_FULL - start)) * segments));
}

export function RpmEl({ props, data }: ElementProps) {
  const n = Math.round(numProp(props, "segments", 15, 3, 30));
  const start = numProp(props, "start", LED_START, 0.3, 0.9);
  const colors = [
    colorOr(props.lowColor, "#2fe07f") as string,
    colorOr(props.midColor, "#ffc93c") as string,
    colorOr(props.highColor, "#ff4b3e") as string,
  ];
  const shift = colorOr(props.shiftColor, "#3d8bff") as string;
  const box = useRef<HTMLDivElement | null>(null);
  const leds = useRef<HTMLElement[]>([]);
  leds.current.length = Math.min(leds.current.length, n);
  useElementDraw(() => {
    if (!data.active || !box.current) return;
    const p = readCtx().frame?.player ?? null;
    const limiter = p?.engine.speed_limiter === true;
    const f = p && p.engine.max_rpm > 0 ? p.engine.rpm / p.engine.max_rpm : 0;
    const lit = limiter || !p ? 0 : rampLit(p.engine.rpm, p.engine.max_rpm, n, start);
    setData(box.current, "state", limiter ? "limiter" : f >= SHIFT_AT ? "shift" : "run");
    for (let i = 0; i < leds.current.length; i += 1) {
      const el = leds.current[i];
      if (el) setData(el, "on", i < lit ? "1" : "0");
    }
  });
  return (
    <div
      className="del-rpm"
      data-state="run"
      ref={box}
      aria-hidden="true"
      style={{ "--rpm-shift": shift } as CSSProperties}
    >
      {Array.from({ length: n }, (_, i) => (
        <i
          key={i}
          data-on="0"
          style={{ "--led": colors[Math.min(2, Math.floor((i * 3) / n))] } as CSSProperties}
          ref={(el) => {
            if (el) leds.current[i] = el;
          }}
        />
      ))}
    </div>
  );
}
