import type { PlayerTelemetry } from "@corde/protocol";
import { useRef } from "react";
import { useT } from "../../i18n";
import { classColor, tempColor, tyreScale } from "../../lib/color";
import { fmtClock, fmtGear, fmtLapTime, fmtNum } from "../../lib/format";
import { gameTraits } from "../../lib/games";
import { lapNo } from "../../lib/laps";
import { useTelemetry } from "../../store/telemetry";
import { useFrameDraw } from "../../widgets/shared/hooks";
import { currentSectors } from "../../widgets/shared/sectors";
import { setData, setText } from "../live";
import {
  classCount,
  deltaView,
  energyAutonomy,
  flagTone,
  frontBias,
  fuelAutonomy,
  gapView,
  isTimed,
  LED_COLORS,
  lapLimit,
  playerDelta,
  revState,
  setting,
} from "../model";
import { Lab, Level, type PageProps, Row } from "../parts";

/**
 * The blocks of the « Course » page (`CoursePage` lays them out; the Studio places them one by
 * one, `dashdef/`). Each block draws its fast values itself (`useFrameDraw`), only while its
 * page is the one on screen.
 */

type BlockProps = Pick<PageProps, "frame" | "laps" | "standings" | "flags" | "active" | "idle">;

function player(): PlayerTelemetry | null {
  return useTelemetry.getState().frame?.player ?? null;
}

function Flags({ side, tone }: { side: "left" | "right"; tone: string | null }) {
  return (
    <span className={`dflags is-${side}`} data-tone={tone ?? "none"} aria-hidden="true">
      <i />
      <i />
      <i />
    </span>
  );
}

/** The rev lights between the two flag panels. */
export function RevBlock({
  flags,
  active,
  showFlags = true,
  className,
}: Pick<BlockProps, "flags" | "active"> & { showFlags?: boolean; className?: string }) {
  const rev = useRef<HTMLDivElement | null>(null);
  const leds = useRef<HTMLElement[]>([]);
  useFrameDraw(() => {
    if (!active) return;
    const p = player();
    const r = p
      ? revState(p.engine.rpm, p.engine.max_rpm, p.engine.speed_limiter)
      : revState(0, 0, false);
    if (!rev.current) return;
    setData(rev.current, "state", r.limiter ? "limiter" : r.shift ? "shift" : "run");
    for (let i = 0; i < leds.current.length; i += 1) {
      setData(leds.current[i] as HTMLElement, "on", i < r.lit ? "1" : "0");
    }
  });
  const tone = flagTone(flags);
  return (
    <div className={`dc-rev${className ? ` ${className}` : ""}`}>
      {showFlags && <Flags side="left" tone={tone} />}
      <div className="drev" data-state="run" aria-hidden="true" ref={rev}>
        {LED_COLORS.map((c, i) => (
          <i
            key={i}
            className={`is-${c}`}
            data-on="0"
            ref={(n) => {
              if (n) leds.current[i] = n;
            }}
          />
        ))}
      </div>
      {showFlags && <Flags side="right" tone={tone} />}
    </div>
  );
}

/** Gear, speed, the pit limiter banner and the idle message. */
export function GearBlock({
  active,
  idle,
  showSpeed = true,
}: Pick<BlockProps, "active" | "idle"> & { showSpeed?: boolean }) {
  const { t } = useT();
  const box = useRef<HTMLDivElement | null>(null);
  const gear = useRef<HTMLDivElement | null>(null);
  const speed = useRef<HTMLElement | null>(null);
  useFrameDraw(() => {
    if (!active) return;
    const p = player();
    setText(gear.current, p ? fmtGear(p.engine.gear) : "—");
    setText(speed.current, p ? fmtNum(p.motion.speed_kmh, 0) : "—");
    setData(box.current, "limiter", p?.engine.speed_limiter ? "1" : "0");
  });
  return (
    <div className="dgear" data-limiter="0" ref={box}>
      <div className="dgear-n num" ref={gear}>
        —
      </div>
      {showSpeed && (
        <div className="dspeed">
          <b className="num" ref={speed}>
            —
          </b>
          <span>km/h</span>
        </div>
      )}
      <div className="dlimiter">{t("dash.limiter")}</div>
      {idle && <div className="didle">{idle}</div>}
    </div>
  );
}

/** « P3 /24 » and the class chip when several classes are on track. */
export function PositionBlock({ frame, standings }: Pick<BlockProps, "frame" | "standings">) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const session = frame?.session ?? null;
  const me = standings.find((s) => s.is_player) ?? null;
  const total = session?.num_vehicles || standings.length || null;
  const showClass = me !== null && classCount(standings) >= 2;
  return (
    <div className="dc-posbox">
      <Lab>{t("dash.position")}</Lab>
      <div className="dc-pos">
        <b className="num">{p ? `P${p.place}` : "—"}</b>
        {p && total !== null && <span className="num dc-of">/{total}</span>}
        {showClass && me && (
          <span
            className="dchip num"
            style={{ color: classColor(me.vehicle_class), borderColor: "currentColor" }}
          >
            {me.class_name} · P{me.class_place}
          </span>
        )}
      </div>
    </div>
  );
}

/** Lap (with the limit or the time left), last lap, best lap. */
export function LapRowsBlock({ frame }: Pick<BlockProps, "frame">) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const session = frame?.session ?? null;
  const limit = lapLimit(session);
  const timed = isTimed(session) && limit === null;
  return (
    <>
      <Row label={timed ? t("dash.lapLeft") : t("dash.lap")}>
        {p ? lapNo(p.lap.lap_number) : "—"}
        {p && limit !== null && <small>/{limit}</small>}
        {p && timed && session && <small> {fmtClock(session.time_remaining_s)}</small>}
      </Row>
      <Row label={t("laps.last")}>{fmtLapTime(p?.lap.last_lap_s)}</Row>
      <Row label={t("laps.best")} className="is-best">
        {fmtLapTime(p?.lap.best_lap_s)}
      </Row>
    </>
  );
}

/** The live delta to the best lap and its bar. */
export function DeltaBlock({ active }: Pick<BlockProps, "active">) {
  const { t } = useT();
  const delta = useRef<HTMLDivElement | null>(null);
  const bar = useRef<HTMLElement | null>(null);
  useFrameDraw(() => {
    if (!active) return;
    const d = deltaView(playerDelta(player()));
    setText(delta.current, d.text);
    setData(delta.current, "tone", d.tone);
    if (bar.current) {
      setData(bar.current, "tone", d.tone);
      // `scaleX` from the centre line: no layout work, only the compositor.
      const tr = `scaleX(${Math.abs(d.frac).toFixed(3)})`;
      if (bar.current.style.transform !== tr) bar.current.style.transform = tr;
      setData(bar.current, "side", d.frac < 0 ? "left" : "right");
    }
  });
  return (
    <div className="dc-deltabox">
      <Lab>{t("dash.delta")}</Lab>
      <div className="ddelta num" data-tone="none" ref={delta}>
        —
      </div>
      <span className="dbar" aria-hidden="true">
        <i data-tone="none" ref={bar} />
      </span>
    </div>
  );
}

/** The time of the lap in progress (red when invalidated, « Stands » in the pit lane). */
export function CurrentLapBlock({ frame, active }: Pick<BlockProps, "frame" | "active">) {
  const { t } = useT();
  const lap = useRef<HTMLElement | null>(null);
  useFrameDraw(() => {
    if (!active) return;
    const p = player();
    setText(lap.current, p ? fmtLapTime(p.lap.current_lap_s, { ms: 3, minutes: true }) : "—");
  });
  const p = frame?.player ?? null;
  return (
    <div className={`drow${p?.lap.lap_invalidated ? " is-invalid" : ""}`}>
      <Lab>
        {p?.lap.in_pit_lane
          ? t("laps.inPit")
          : p?.lap.lap_invalidated
            ? t("laps.invalid")
            : t("laps.current")}
      </Lab>
      <span className="num dval" ref={lap}>
        —
      </span>
    </div>
  );
}

/** S1 S2 S3, coloured like the timing screens (best, faster, slower), the running one framed. */
export function SectorsBlock({ frame }: Pick<BlockProps, "frame">) {
  const p = frame?.player ?? null;
  const sectors = p ? currentSectors(p.lap) : null;
  return (
    <div className="dsect">
      {[0, 1, 2].map((i) => {
        const s = sectors?.[i];
        const running = p !== null && p.lap.current_sector === i + 1;
        return (
          <span
            key={i}
            className={`num${running ? " is-running" : ""}`}
            data-tone={s && s.tone !== "running" ? s.tone : "none"}
          >
            S{i + 1}
          </span>
        );
      })}
    </div>
  );
}

/** Gap to the car ahead and behind in the race. */
export function GapsBlock({ frame }: Pick<BlockProps, "frame">) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const ahead = gapView(p?.gaps.place_ahead_s ?? null, "ahead");
  const behind = gapView(p?.gaps.place_behind_s ?? null, "behind");
  return (
    <div className="dc-gaps">
      <span className="dc-gap">
        <Lab>{t("gaps.ahead")}</Lab>
        <b className="num" data-tone={ahead.tone}>
          {ahead.text}
        </b>
      </span>
      <span className="dc-gap">
        <Lab>{t("gaps.behind")}</Lab>
        <b className="num" data-tone={behind.tone}>
          {behind.text}
        </b>
      </span>
    </div>
  );
}

/** The four tyres as coloured squares (average surface temperature). */
export function MiniTyres({ frame }: Pick<BlockProps, "frame">) {
  const p = frame?.player ?? null;
  const scale = tyreScale(p?.wheels[0].optimal_temp_c ?? null);
  return (
    <span className="dminityres" aria-hidden="true">
      {[0, 1, 2, 3].map((i) => {
        const w = p?.wheels[i];
        const avg = w ? (w.temp_c[0] + w.temp_c[1] + w.temp_c[2]) / 3 : Number.NaN;
        return <i key={i} style={{ background: tempColor(avg, scale) }} />;
      })}
    </span>
  );
}

/** The bottom strip: fuel, energy, TC, ABS, bias, map, tyres — only what this car has. */
export function CarStripBlock({ frame, laps }: Pick<BlockProps, "frame" | "laps">) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const e = p?.energy ?? null;
  const el = p?.electronics ?? null;
  const fuel = p ? fuelAutonomy(p.energy, laps) : null;
  const ve = e?.virtual_energy ?? null;
  const veAuto = p ? energyAutonomy(p.energy, laps) : null;
  const tc = el ? setting(el.tc, el.tc_max) : null;
  const abs = el ? setting(el.abs, el.abs_max) : null;
  const map = el ? setting(el.motor_map, el.motor_map_max) : null;
  const bias = el ? frontBias(el) : null;
  return (
    <div className="dc-bot">
      <div className="dcell is-wide">
        <Lab>{t("dash.fuel")}</Lab>
        <span className="num dcell-v">
          {e ? fmtNum(e.fuel_l, 1) : "—"}
          <small>L{fuel ? ` · ${t("dash.lapsN", { n: fmtNum(fuel.laps, 1) })}` : ""}</small>
        </span>
      </div>
      {ve !== null && (
        <div className="dcell is-wide">
          <Lab>{t("dash.energy")}</Lab>
          <span className="num dcell-v">
            {fmtNum(ve * 100, 0)}
            <small>%{veAuto ? ` · ${t("dash.lapsN", { n: fmtNum(veAuto.laps, 1) })}` : ""}</small>
          </span>
          <Level fraction={ve} tone="cold" />
        </div>
      )}
      {tc !== null && (
        <div className={`dcell${el?.tc_active ? " is-lit" : ""}`}>
          <Lab>{t("elec.tc")}</Lab>
          <span className="num dcell-v">{tc}</span>
        </div>
      )}
      {abs !== null && (
        <div className={`dcell${el?.abs_active ? " is-lit" : ""}`}>
          <Lab>{t("elec.abs")}</Lab>
          <span className="num dcell-v">{abs}</span>
        </div>
      )}
      <div className="dcell is-mid">
        <Lab>{t("dash.biasFront")}</Lab>
        <span className="num dcell-v">{fmtNum(bias, 1)}</span>
      </div>
      {map !== null && (
        <div className="dcell">
          <Lab>{t("dash.map")}</Lab>
          <span className="num dcell-v">{map}</span>
        </div>
      )}
      {gameTraits(frame?.session.game).tyres && (
        <div className="dcell is-mid">
          <Lab>{t("dash.tyres")}</Lab>
          <MiniTyres frame={frame} />
        </div>
      )}
    </div>
  );
}
