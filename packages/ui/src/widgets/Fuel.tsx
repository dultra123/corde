import type { MotorState } from "@corde/protocol";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { fmtNum } from "../lib/format";
import { energyLaps, fuelLaps } from "../lib/fuel";
import { useLaps, usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const motorKey = (m: MotorState) =>
  (
    ({
      unavailable: "motor.unavailable",
      inactive: "motor.inactive",
      propulsion: "motor.propulsion",
      regeneration: "motor.regeneration",
    }) as const
  )[m];

function levelColor(fraction: number): string {
  if (fraction < 0.1) return COLORS.hot;
  if (fraction < 0.2) return COLORS.warm;
  return COLORS.text;
}

function Level({ fraction, color }: { fraction: number; color: string }) {
  return (
    <div className="level">
      <div
        className="level-fill"
        style={{ width: `${Math.max(0, Math.min(1, fraction)) * 100}%`, background: color }}
      />
    </div>
  );
}

function FuelWidget(_props: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  const laps = useLaps();
  if (!player) return <Empty />;
  const e = player.energy;
  const fuel = fuelLaps(e.fuel_l, laps);
  const ve = e.virtual_energy !== null ? energyLaps(e.virtual_energy, laps) : null;
  const fuelColor = levelColor(e.fuel_fraction);
  const hasHybrid = e.motor_state !== "unavailable" || e.battery_soc !== null;
  return (
    <div className="fuel">
      <div className="fuel-main">
        <div className="fuel-litres num" style={{ color: fuelColor }}>
          {fmtNum(e.fuel_l, 1)}
          <small> L</small>
        </div>
        <div className="fuel-laps">
          <span className="num">{fuel ? fmtNum(fuel.lapsLeftExact, 1) : "—"}</span>
          <span className="dim">{t("fuel.lapsLeft")}</span>
        </div>
      </div>
      <Level fraction={e.fuel_fraction} color={fuelColor} />
      <div className="fuel-row dim">
        <span>
          <span className="num">{fmtNum(e.fuel_fraction * 100, 0)} %</span> ·{" "}
          {fmtNum(e.fuel_capacity_l, 0)} L {t("fuel.capacity")}
        </span>
        <span>
          {fuel ? (
            <>
              <span className="num">{fmtNum(fuel.perLap, 2)} L</span> {t("fuel.perLap")}
            </>
          ) : (
            t("fuel.needLaps")
          )}
        </span>
      </div>

      {e.virtual_energy !== null && (
        <div className="fuel-block">
          <div className="fuel-row">
            <span className="dim">{t("fuel.virtualEnergy")}</span>
            <span>
              <span className="num fuel-pct" style={{ color: levelColor(e.virtual_energy) }}>
                {fmtNum(e.virtual_energy * 100, 1)} %
              </span>
              {ve && (
                <span className="dim">
                  {" · "}
                  <span className="num">{fmtNum(ve.lapsLeftExact, 1)}</span> {t("fuel.lapsLeft")}
                </span>
              )}
            </span>
          </div>
          <Level fraction={e.virtual_energy} color={COLORS.cold} />
        </div>
      )}

      {hasHybrid && (
        <div className="fuel-block fuel-hybrid">
          <div className="fuel-row">
            <span className="dim">{t("fuel.battery")}</span>
            <span className="num fuel-pct">
              {e.battery_soc !== null ? `${fmtNum(e.battery_soc * 100, 0)} %` : "—"}
            </span>
          </div>
          {e.battery_soc !== null && <Level fraction={e.battery_soc} color={COLORS.best} />}
          <div className="fuel-row dim">
            <span>
              {t("fuel.regen")}{" "}
              <span
                className="num"
                style={{ color: (e.regen_kw ?? 0) > 1 ? COLORS.optimal : undefined }}
              >
                {e.regen_kw !== null ? `${fmtNum(e.regen_kw, 0)} kW` : "—"}
              </span>
            </span>
            <span>{t(motorKey(e.motor_state))}</span>
          </div>
        </div>
      )}
    </div>
  );
}

export const fuelWidget = registerWidget({
  id: "fuel",
  titleKey: "widget.fuel",
  minW: 2,
  minH: 3,
  defaultSize: { w: 3, h: 6 },
  mobileSize: { w: 4, h: 4 },
  component: FuelWidget,
});
