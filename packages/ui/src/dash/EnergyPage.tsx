import type { MotorState, PitState, SessionType } from "@corde/protocol";
import { type TranslationKey, useT } from "../i18n";
import { fmtClock, fmtNum } from "../lib/format";
import {
  energyAutonomy,
  fuelAutonomy,
  fuelToFinish,
  hasHybrid,
  isTimed,
  lapLimit,
  lapsLeft,
  lapsLeftTimed,
} from "./model";
import { Lab, Level, PageHead, type PageProps, Row } from "./parts";

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

const SESSION: Record<SessionType, TranslationKey> = {
  test_day: "session.type.test_day",
  practice: "session.type.practice",
  qualify: "session.type.qualify",
  warmup: "session.type.warmup",
  race: "session.type.race",
  unknown: "session.type.unknown",
};

/** Low levels turn amber then red, like the Fuel widget. */
function levelTone(fraction: number | null): string {
  if (fraction === null) return "text";
  if (fraction < 0.1) return "hot";
  if (fraction < 0.2) return "warm";
  return "text";
}

/** Page 3 — « Carburant & énergie »: autonomy, hybrid, what is left of the session, weather. */
export function EnergyPage({ frame, laps }: PageProps) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const s = frame?.session ?? null;
  const e = p?.energy ?? null;
  const fuel = p ? fuelAutonomy(p.energy, laps) : null;
  const ve = e?.virtual_energy ?? null;
  const veAuto = p ? energyAutonomy(p.energy, laps) : null;
  const hybrid = e ? hasHybrid(e) : false;
  const limit = lapLimit(s);
  const timed = isTimed(s);
  const left = lapsLeft(s, p);
  const leftTimed = limit === null ? lapsLeftTimed(s, laps) : null;
  const toGo = left ?? leftTimed;
  const need = e ? fuelToFinish(e.fuel_l, fuel?.perLap ?? null, toGo) : null;
  const w = s?.weather ?? null;
  const fuelTone = levelTone(e?.fuel_fraction ?? null);

  return (
    <div className="de">
      <PageHead
        title={t("dash.page.energy")}
        info={
          s ? `${t(SESSION[s.session_type])}${s.track_name ? ` · ${s.track_name}` : ""}` : undefined
        }
      />
      <div className="de-grid">
        <section className="dpanel de-fuel">
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
            {need === null ? "—" : `${leftTimed !== null ? "≈ " : ""}${fmtNum(Math.abs(need), 1)}`}
            <small>L</small>
          </Row>
        </section>

        {(ve !== null || hybrid) && (
          <div className="de-col">
            {ve !== null && (
              <section className="dpanel">
                <Lab>{t("fuel.virtualEnergy")}</Lab>
                <div className="num dbig is-cold">
                  {fmtNum(ve * 100, 1)}
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
            )}
            {hybrid && e && (
              <section className="dpanel">
                <Lab>{t("fuel.battery")}</Lab>
                <div className="num dbig is-best">
                  {e.battery_soc === null ? "—" : fmtNum(e.battery_soc * 100, 0)}
                  <small>%</small>
                </div>
                <Level fraction={e.battery_soc} tone="best" />
                <Row label={t("fuel.regen")}>
                  {e.regen_kw === null ? "—" : fmtNum(e.regen_kw, 0)}
                  <small>kW</small>
                </Row>
                <Row label={t("dash.motor")}>
                  <span className="dval-text">{t(MOTOR[e.motor_state])}</span>
                </Row>
              </section>
            )}
          </div>
        )}

        <div className="de-col">
          <section className="dpanel">
            <Lab>{t("dash.session")}</Lab>
            {timed && s ? (
              <Row label={t("session.remaining")}>{fmtClock(s.time_remaining_s)}</Row>
            ) : null}
            <Row label={t("dash.lapsToGo")}>
              {toGo === null ? "—" : `${leftTimed !== null ? "≈ " : ""}${toGo}`}
            </Row>
            <Row label={t("dash.stops")}>{e ? e.scheduled_stops : "—"}</Row>
            <Row label={t("dash.pit")}>
              <span className="dval-text">{p ? t(PIT[p.pit_state]) : "—"}</span>
            </Row>
          </section>
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
        </div>
      </div>
    </div>
  );
}
