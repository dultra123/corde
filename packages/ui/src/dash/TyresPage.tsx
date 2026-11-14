import type { CompoundType, Wheel } from "@corde/protocol";
import { type TranslationKey, useT } from "../i18n";
import { BRAKE_SCALE, tempColor, tyreScale, wearColor } from "../lib/color";
import { fmtNum } from "../lib/format";
import { frontBias } from "./model";
import { Lab, Level, PageHead, type PageProps } from "./parts";

const COMPOUND: Record<CompoundType, TranslationKey> = {
  soft: "compound.soft",
  medium: "compound.medium",
  hard: "compound.hard",
  wet: "compound.wet",
  unknown: "compound.unknown",
};

const CORNERS: TranslationKey[] = ["dash.fl", "dash.fr", "dash.rl", "dash.rr"];

function TyreCard({ wheel, corner }: { wheel: Wheel | null; corner: TranslationKey }) {
  const { t } = useT();
  const scale = tyreScale(wheel?.optimal_temp_c ?? null);
  const bad = wheel ? wheel.flat || wheel.detached : false;
  const brake = wheel ? tempColor(wheel.brake_temp_c, BRAKE_SCALE) : undefined;
  // Neutral while the brakes sit in their window: colour only says « too cold / too hot ».
  const brakeTone =
    wheel && (wheel.brake_temp_c < BRAKE_SCALE.optLow || wheel.brake_temp_c > BRAKE_SCALE.optHigh)
      ? brake
      : undefined;
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
          <span className="num dval" style={{ color: brakeTone }}>
            {wheel ? `${fmtNum(wheel.brake_temp_c, 0)}°` : "—"}
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

/** Page 2 — « Pneus & freins »: the four corners, the ideal window, the bias, the engine. */
export function TyresPage({ frame }: PageProps) {
  const { t } = useT();
  const p = frame?.player ?? null;
  const w = frame?.session.weather ?? null;
  const wheels = p?.wheels ?? null;
  const opt = wheels?.[0].optimal_temp_c ?? null;
  const scale = tyreScale(opt);
  const compound = p ? p.tyres.front_compound || t(COMPOUND[p.wheels[0].compound_type]) : null;
  const info = [
    compound,
    w ? `${t("dash.trackShort")} ${fmtNum(w.track_c, 0)}°` : null,
    w ? `${t("dash.airShort")} ${fmtNum(w.ambient_c, 0)}°` : null,
  ]
    .filter((x) => x)
    .join(" · ");
  const bias = p ? frontBias(p.electronics) : null;
  return (
    <div className="dt">
      <PageHead title={t("dash.page.tyres")} info={info || undefined} />
      <div className="dt-grid">
        <TyreCard wheel={wheels?.[0] ?? null} corner={CORNERS[0] as TranslationKey} />
        <section className="dcentre dt-centre">
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
        <TyreCard wheel={wheels?.[1] ?? null} corner={CORNERS[1] as TranslationKey} />
        <TyreCard wheel={wheels?.[2] ?? null} corner={CORNERS[2] as TranslationKey} />
        <TyreCard wheel={wheels?.[3] ?? null} corner={CORNERS[3] as TranslationKey} />
      </div>
    </div>
  );
}
