import type { CompoundType, TelemetryFrame, Wheel } from "@corde/protocol";
import { type TranslationKey, useT } from "../../i18n";
import { BRAKE_SCALE, tempColor, tyreScale, wearColor } from "../../lib/color";
import { fmtNum } from "../../lib/format";
import { brakeTemp, frontBias } from "../model";
import { Lab, Level, Row } from "../parts";

/** The blocks of the « Pneus & freins » page (`TyresPage`), also offered by the Studio. */

const COMPOUND: Record<CompoundType, TranslationKey> = {
  soft: "compound.soft",
  medium: "compound.medium",
  hard: "compound.hard",
  wet: "compound.wet",
  unknown: "compound.unknown",
};

export const CORNERS: TranslationKey[] = ["dash.fl", "dash.fr", "dash.rl", "dash.rr"];

/** The colour of a brake temperature: neutral in its window, « too cold / too hot » outside. */
function brakeTone(temp: number): string | undefined {
  return temp < BRAKE_SCALE.optLow || temp > BRAKE_SCALE.optHigh
    ? tempColor(temp, BRAKE_SCALE)
    : undefined;
}

/** One corner: the tread (three temperatures), pressure, brake, wear. */
export function TyreCard({ wheel, corner }: { wheel: Wheel | null; corner: TranslationKey }) {
  const { t } = useT();
  const scale = tyreScale(wheel?.optimal_temp_c ?? null);
  const bad = wheel ? wheel.flat || wheel.detached : false;
  return (
    <section className={`dpanel dt-card${bad ? " is-bad" : ""}`}>
      <div className="dt-tread">
        <span className="dt-cn">{t(corner)}</span>
        {[0, 1, 2].map((i) => {
          const temp = wheel?.temp_c[i] ?? Number.NaN;
          return (
            <i key={i} className="num" style={{ background: tempColor(temp, scale) }}>
              {wheel ? Math.round(temp) : "—"}
            </i>
          );
        })}
        {bad && wheel && (
          <span className="dt-flag">{t(wheel.detached ? "tyres.detached" : "tyres.flat")}</span>
        )}
      </div>
      <div className="dt-kv">
        <div className="drow">
          <Lab>
            <span className="dt-corner">{t(corner)} · </span>
            {t("dash.pressure")}
          </Lab>
          <span className="num dval">
            {fmtNum(wheel?.pressure_kpa, 0)}
            <small>kPa</small>
          </span>
        </div>
        <div className="drow">
          <Lab>{t("dash.brake")}</Lab>
          <span
            className="num dval"
            style={{
              color:
                brakeTemp(wheel?.brake_temp_c) !== null
                  ? brakeTone(wheel?.brake_temp_c ?? 0)
                  : undefined,
            }}
          >
            {brakeTemp(wheel?.brake_temp_c) !== null
              ? `${fmtNum(wheel?.brake_temp_c ?? 0, 0)}°`
              : "—"}
          </span>
        </div>
        <div className="dt-wear">
          <div className="drow">
            <Lab>{t("dash.wear")}</Lab>
            <span
              className="num dval is-small"
              style={{ color: wheel ? wearColor(wheel.wear_remaining) : undefined }}
            >
              {wheel ? `${fmtNum(wheel.wear_remaining * 100, 0)} %` : "—"}
            </span>
          </div>
          <Level fraction={wheel?.wear_remaining ?? null} tone="text" />
        </div>
      </div>
    </section>
  );
}

/** Compound, track and air temperatures: the line of context of the page. */
export function tyresInfo(frame: TelemetryFrame | null, t: (k: TranslationKey) => string): string {
  const p = frame?.player ?? null;
  const w = frame?.session.weather ?? null;
  const compound = p ? p.tyres.front_compound || t(COMPOUND[p.wheels[0].compound_type]) : null;
  return [
    compound,
    w ? `${t("dash.trackShort")} ${fmtNum(w.track_c, 0)}°` : null,
    w ? `${t("dash.airShort")} ${fmtNum(w.ambient_c, 0)}°` : null,
  ]
    .filter((x) => x)
    .join(" · ");
}

/** The centre column: the ideal window, the brake bias, water and oil. */
export function TyresCentre({
  frame,
  className,
}: {
  frame: TelemetryFrame | null;
  className?: string;
}) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const opt = p?.wheels[0].optimal_temp_c ?? null;
  const scale = tyreScale(opt);
  const bias = p ? frontBias(p.electronics) : null;
  return (
    <section className={`dcentre dt-centre${className ? ` ${className}` : ""}`}>
      {opt !== null && (
        <div>
          <Lab>{t("dash.window")}</Lab>
          <div className="num dbig is-optimal">
            {Math.round(scale.optLow)}–{Math.round(scale.optHigh)}°
          </div>
        </div>
      )}
      <div>
        <Lab>{t("dash.bias")}</Lab>
        <div className="num dbig">{bias === null ? "—" : `${fmtNum(bias, 1)} %`}</div>
      </div>
      <div>
        <Lab>{t("dash.waterOil")}</Lab>
        <div className="num dbig is-small">
          {p ? `${fmtNum(p.engine.water_c, 0)}° · ${fmtNum(p.engine.oil_c, 0)}°` : "—"}
        </div>
      </div>
    </section>
  );
}

/** The four brake temperatures and the bias (Studio only). */
export function BrakesBlock({ frame }: { frame: TelemetryFrame | null }) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const bias = p ? frontBias(p.electronics) : null;
  return (
    <>
      <Lab>{t("dash.brakes")}</Lab>
      <div className="dbrakes">
        {[0, 1, 2, 3].map((i) => {
          const w = p?.wheels[i] ?? null;
          return (
            <div key={i} className="dbrakes-c">
              <Lab>{t(CORNERS[i] as TranslationKey)}</Lab>
              <span
                className="num dval"
                style={{
                  color:
                    brakeTemp(w?.brake_temp_c) !== null
                      ? brakeTone(w?.brake_temp_c ?? 0)
                      : undefined,
                }}
              >
                {brakeTemp(w?.brake_temp_c) !== null ? `${fmtNum(w?.brake_temp_c ?? 0, 0)}°` : "—"}
              </span>
            </div>
          );
        })}
      </div>
      <Row label={t("dash.biasFront")}>{bias === null ? "—" : `${fmtNum(bias, 1)} %`}</Row>
    </>
  );
}
