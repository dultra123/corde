import type { PlayerTelemetry } from "@corde/protocol";
import { useRef } from "react";
import { useT } from "../i18n";
import { classColor, tempColor, tyreScale } from "../lib/color";
import { fmtClock, fmtGear, fmtLapTime, fmtNum } from "../lib/format";
import { lapNo } from "../lib/laps";
import { useTelemetry } from "../store/telemetry";
import { useFrameDraw } from "../widgets/shared/hooks";
import { currentSectors } from "../widgets/shared/sectors";
import { setData, setText } from "./live";
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
} from "./model";
import { Lab, Level, type PageProps, Row } from "./parts";

/** The fast part of the page: written straight into these elements (`useFrameDraw`). */
interface FastRefs {
  rev: HTMLDivElement | null;
  leds: HTMLElement[];
  gear: HTMLDivElement | null;
  speed: HTMLElement | null;
  box: HTMLDivElement | null;
  delta: HTMLDivElement | null;
  bar: HTMLElement | null;
  lap: HTMLElement | null;
}

function drawFast(r: FastRefs, p: PlayerTelemetry | null): void {
  const rev = p
    ? revState(p.engine.rpm, p.engine.max_rpm, p.engine.speed_limiter)
    : revState(0, 0, false);
  if (r.rev) {
    setData(r.rev, "state", rev.limiter ? "limiter" : rev.shift ? "shift" : "run");
    for (let i = 0; i < r.leds.length; i += 1) {
      const led = r.leds[i] as HTMLElement;
      setData(led, "on", i < rev.lit ? "1" : "0");
    }
  }
  setText(r.gear, p ? fmtGear(p.engine.gear) : "—");
  setText(r.speed, p ? fmtNum(p.motion.speed_kmh, 0) : "—");
  setData(r.box, "limiter", rev.limiter ? "1" : "0");
  const d = deltaView(playerDelta(p));
  setText(r.delta, d.text);
  setData(r.delta, "tone", d.tone);
  if (r.bar) {
    setData(r.bar, "tone", d.tone);
    // `scaleX` from the centre line: no layout work, only the compositor.
    const t = `scaleX(${Math.abs(d.frac).toFixed(3)})`;
    if (r.bar.style.transform !== t) r.bar.style.transform = t;
    setData(r.bar, "side", d.frac < 0 ? "left" : "right");
  }
  setText(r.lap, p ? fmtLapTime(p.lap.current_lap_s, { ms: 3, minutes: true }) : "—");
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

/** Page 1 — « Course »: revs, position, laps, gear and speed, delta, sectors, gaps, the car. */
export function CoursePage({ frame, laps, standings, flags, active, idle }: PageProps) {
  const { t } = useT();
  const refs = useRef<FastRefs>({
    rev: null,
    leds: [],
    gear: null,
    speed: null,
    box: null,
    delta: null,
    bar: null,
    lap: null,
  });
  useFrameDraw(() => {
    if (!active) return;
    drawFast(refs.current, useTelemetry.getState().frame?.player ?? null);
  });

  const p = frame?.player ?? null;
  const session = frame?.session ?? null;
  const me = standings.find((s) => s.is_player) ?? null;
  const total = session?.num_vehicles || standings.length || null;
  const limit = lapLimit(session);
  const timed = isTimed(session) && limit === null;
  const showClass = me !== null && classCount(standings) >= 2;
  const sectors = p ? currentSectors(p.lap) : null;
  const ahead = gapView(p?.gaps.place_ahead_s ?? null, "ahead");
  const behind = gapView(p?.gaps.place_behind_s ?? null, "behind");
  const tone = flagTone(flags);

  // The bottom strip: only what this car has.
  const e = p?.energy ?? null;
  const el = p?.electronics ?? null;
  const fuel = p ? fuelAutonomy(p.energy, laps) : null;
  const ve = e?.virtual_energy ?? null;
  const veAuto = p ? energyAutonomy(p.energy, laps) : null;
  const tc = el ? setting(el.tc, el.tc_max) : null;
  const abs = el ? setting(el.abs, el.abs_max) : null;
  const map = el ? setting(el.motor_map, el.motor_map_max) : null;
  const bias = el ? frontBias(el) : null;
  const opt = p?.wheels[0].optimal_temp_c ?? null;
  const scale = tyreScale(opt);

  return (
    <div className="dc">
      <div className="dc-rev">
        <Flags side="left" tone={tone} />
        <div
          className="drev"
          data-state="run"
          aria-hidden="true"
          ref={(n) => {
            refs.current.rev = n;
          }}
        >
          {LED_COLORS.map((c, i) => (
            <i
              key={i}
              className={`is-${c}`}
              data-on="0"
              ref={(n) => {
                if (n) refs.current.leds[i] = n;
              }}
            />
          ))}
        </div>
        <Flags side="right" tone={tone} />
      </div>

      <section className="dpanel dc-left">
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
        <Row label={timed ? t("dash.lapLeft") : t("dash.lap")}>
          {p ? lapNo(p.lap.lap_number) : "—"}
          {p && limit !== null && <small>/{limit}</small>}
          {p && timed && session && <small> {fmtClock(session.time_remaining_s)}</small>}
        </Row>
        <Row label={t("laps.last")}>{fmtLapTime(p?.lap.last_lap_s)}</Row>
        <Row label={t("laps.best")} className="is-best">
          {fmtLapTime(p?.lap.best_lap_s)}
        </Row>
      </section>

      <div
        className="dgear"
        data-limiter="0"
        ref={(n) => {
          refs.current.box = n;
        }}
      >
        <div
          className="dgear-n num"
          ref={(n) => {
            refs.current.gear = n;
          }}
        >
          —
        </div>
        <div className="dspeed">
          <b
            className="num"
            ref={(n) => {
              refs.current.speed = n;
            }}
          >
            —
          </b>
          <span>km/h</span>
        </div>
        <div className="dlimiter">{t("dash.limiter")}</div>
        {idle && <div className="didle">{idle}</div>}
      </div>

      <section className="dpanel dc-right">
        <div className="dc-deltabox">
          <Lab>{t("dash.delta")}</Lab>
          <div
            className="ddelta num"
            data-tone="none"
            ref={(n) => {
              refs.current.delta = n;
            }}
          >
            —
          </div>
          <span className="dbar" aria-hidden="true">
            <i
              data-tone="none"
              ref={(n) => {
                refs.current.bar = n;
              }}
            />
          </span>
        </div>
        <div className={`drow${p?.lap.lap_invalidated ? " is-invalid" : ""}`}>
          <Lab>
            {p?.lap.in_pit_lane
              ? t("laps.inPit")
              : p?.lap.lap_invalidated
                ? t("laps.invalid")
                : t("laps.current")}
          </Lab>
          <span
            className="num dval"
            ref={(n) => {
              refs.current.lap = n;
            }}
          >
            —
          </span>
        </div>
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
      </section>

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
        <div className="dcell is-mid">
          <Lab>{t("dash.tyres")}</Lab>
          <span className="dminityres" aria-hidden="true">
            {[0, 1, 2, 3].map((i) => {
              const w = p?.wheels[i];
              const avg = w ? (w.temp_c[0] + w.temp_c[1] + w.temp_c[2]) / 3 : Number.NaN;
              return <i key={i} style={{ background: tempColor(avg, scale) }} />;
            })}
          </span>
        </div>
      </div>
    </div>
  );
}
