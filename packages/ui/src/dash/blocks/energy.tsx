import type { LapRecord, MotorState, PitState, SessionType, TelemetryFrame } from "@corde/protocol";
import { type TranslationKey, useT } from "../../i18n";
import { fmtClock, fmtNum } from "../../lib/format";
import {
  energyAutonomy,
  fuelAutonomy,
  fuelToFinish,
  hasHybrid,
  isTimed,
  lapLimit,
  lapsLeft,
  lapsLeftTimed,
} from "../model";
import { Lab, Level, Row } from "../parts";

/** The blocks of the « Carburant & énergie » page (`EnergyPage`), also offered by the Studio. */

interface Data {
  frame: TelemetryFrame | null;
  laps: LapRecord[];
}

const PIT: Record<PitState, TranslationKey> = {
  none: "dash.pit.none",
  request: "dash.pit.request",
  entering: "dash.pit.entering",
  stopped: "dash.pit.stopped",
  exiting: "dash.pit.exiting",
};

const MOTOR: Record<MotorState, TranslationKey> = {
  unavailable: "motor.unavailable",
  inactive: "motor.inactive",
  propulsion: "motor.propulsion",
  regeneration: "motor.regeneration",
};

export const SESSION: Record<SessionType, TranslationKey> = {
  test_day: "session.type.test_day",
  practice: "session.type.practice",
  qualify: "session.type.qualify",
  warmup: "session.type.warmup",
  race: "session.type.race",
  unknown: "session.type.unknown",
};

/** Low levels turn amber then red, like the Fuel widget. */
export function levelTone(fraction: number | null): string {
  if (fraction === null) return "text";
  if (fraction < 0.1) return "hot";
  if (fraction < 0.2) return "warm";
  return "text";
}

/** Laps still to run: the limit, or an estimate (≈) in a timed session. */
function toGo(frame: TelemetryFrame | null, laps: LapRecord[]) {
  const s = frame?.session ?? null;
  const p = frame?.player ?? null;
  const limit = lapLimit(s);
  const left = lapsLeft(s, p);
  const leftTimed = limit === null ? lapsLeftTimed(s, laps) : null;
  return { value: left ?? leftTimed, estimate: leftTimed !== null };
}

/** Fuel: litres, tank, autonomy, use per lap, missing or spare at the finish. */
export function FuelPanel({ frame, laps, className }: Data & { className?: string }) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const e = p?.energy ?? null;
  const fuel = p ? fuelAutonomy(p.energy, laps) : null;
  const go = toGo(frame, laps);
  const need = e ? fuelToFinish(e.fuel_l, fuel?.perLap ?? null, go.value) : null;
  const fuelTone = levelTone(e?.fuel_fraction ?? null);
  return (
    <section className={`dpanel de-fuel${className ? ` ${className}` : ""}`}>
      <div>
        <Lab>{t("dash.fuel")}</Lab>
        <div className={`num dhuge is-${fuelTone}`}>
          {e ? fmtNum(e.fuel_l, 1) : "—"}
          <small>L</small>
        </div>
        <Level fraction={e?.fuel_fraction ?? null} tone={fuelTone} />
      </div>
      <Row label={t("dash.tank")}>
        {e ? fmtNum(e.fuel_fraction * 100, 0) : "—"}
        <small>%</small>
      </Row>
      <Row label={t("dash.autonomy")}>
        {fuel ? fmtNum(fuel.laps, 1) : "—"}
        <small>{t("dash.laps")}</small>
      </Row>
      <Row label={t("dash.perLap")}>
        {fuel ? fmtNum(fuel.perLap, 2) : "—"}
        <small>L</small>
      </Row>
      <Row
        label={need !== null && need <= 0 ? t("dash.spare") : t("dash.toFinish")}
        className={need !== null && need > 0 ? "is-warn" : undefined}
      >
        {need === null ? "—" : `${go.estimate ? "≈ " : ""}${fmtNum(Math.abs(need), 1)}`}
        <small>L</small>
      </Row>
    </section>
  );
}

/** The virtual energy (LMU), when the car has it. */
export function VirtualEnergyPanel({ frame, laps }: Data) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const ve = p?.energy.virtual_energy ?? null;
  const veAuto = p ? energyAutonomy(p.energy, laps) : null;
  return (
    <section className="dpanel">
      <Lab>{t("fuel.virtualEnergy")}</Lab>
      <div className="num dbig is-cold">
        {fmtNum(ve === null ? null : ve * 100, 1)}
        <small>%</small>
      </div>
      <Level fraction={ve} tone="cold" />
      <Row label={t("dash.autonomy")}>
        {veAuto ? fmtNum(veAuto.laps, 1) : "—"}
        <small>{t("dash.laps")}</small>
      </Row>
      <Row label={t("dash.perLap")}>
        {veAuto ? fmtNum(veAuto.perLap * 100, 1) : "—"}
        <small>%</small>
      </Row>
    </section>
  );
}

/** Battery, regeneration, motor state of a hybrid. */
export function HybridPanel({ frame }: Pick<Data, "frame">) {
  const { t } = useT();
  const e = frame?.player?.energy ?? null;
  return (
    <section className="dpanel">
      <Lab>{t("fuel.battery")}</Lab>
      <div className="num dbig is-best">
        {e?.battery_soc == null ? "—" : fmtNum(e.battery_soc * 100, 0)}
        <small>%</small>
      </div>
      <Level fraction={e?.battery_soc ?? null} tone="best" />
      <Row label={t("fuel.regen")}>
        {e?.regen_kw == null ? "—" : fmtNum(e.regen_kw, 0)}
        <small>kW</small>
      </Row>
      <Row label={t("dash.motor")}>
        <span className="dval-text">{e ? t(MOTOR[e.motor_state]) : "—"}</span>
      </Row>
    </section>
  );
}

/** Time left, laps to go, stops planned, pit state. */
export function SessionPanel({ frame, laps }: Data) {
  const { t } = useT();
  const s = frame?.session ?? null;
  const p = frame?.player ?? null;
  const e = p?.energy ?? null;
  const go = toGo(frame, laps);
  return (
    <section className="dpanel">
      <Lab>{t("dash.session")}</Lab>
      {isTimed(s) && s ? (
        <Row label={t("session.remaining")}>{fmtClock(s.time_remaining_s)}</Row>
      ) : null}
      <Row label={t("dash.lapsToGo")}>
        {go.value === null ? "—" : `${go.estimate ? "≈ " : ""}${go.value}`}
      </Row>
      <Row label={t("dash.stops")}>{e ? e.scheduled_stops : "—"}</Row>
      <Row label={t("dash.pit")}>
        <span className="dval-text">{p ? t(PIT[p.pit_state]) : "—"}</span>
      </Row>
    </section>
  );
}

/** Track, air, rain, wetness. */
export function WeatherPanel({ frame }: Pick<Data, "frame">) {
  const { t } = useT();
  const w = frame?.session.weather ?? null;
  return (
    <section className="dpanel">
      <Lab>{t("dash.weather")}</Lab>
      <Row label={t("session.trackTemp")}>{w ? `${fmtNum(w.track_c, 0)}°` : "—"}</Row>
      <Row label={t("session.air")}>{w ? `${fmtNum(w.ambient_c, 0)}°` : "—"}</Row>
      <Row label={t("session.rain")}>
        {w ? fmtNum(w.rain * 100, 0) : "—"}
        <small>%</small>
      </Row>
      <Row label={t("session.wet")}>
        {w ? fmtNum(w.wetness_avg * 100, 0) : "—"}
        <small>%</small>
      </Row>
    </section>
  );
}

/** The car shows a virtual energy panel and / or a hybrid panel. */
export function energyPanels(frame: TelemetryFrame | null): { ve: boolean; hybrid: boolean } {
  const e = frame?.player?.energy ?? null;
  return { ve: (e?.virtual_energy ?? null) !== null, hybrid: e ? hasHybrid(e) : false };
}
