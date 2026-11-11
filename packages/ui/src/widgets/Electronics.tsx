import { useT } from "../i18n";
import { fmtNum } from "../lib/format";
import { usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

function Setting({
  label,
  value,
  max,
  active,
}: {
  label: string;
  value: number;
  max: number;
  active?: boolean;
}) {
  const available = max > 0;
  return (
    <div className={`setting${active ? " is-active" : ""}${available ? "" : " is-na"}`}>
      <span className="setting-label dim">{label}</span>
      <span className="setting-value num">
        {available ? value : "—"}
        {available && <small>/{max}</small>}
      </span>
      {available && (
        <span className="setting-ticks">
          {Array.from({ length: Math.min(max, 12) }, (_, i) => (
            <i
              key={i}
              className={i < Math.round((value / max) * Math.min(max, 12)) ? "is-on" : ""}
            />
          ))}
        </span>
      )}
    </div>
  );
}

function Lamp({
  label,
  on,
  tone = "on",
}: {
  label: string;
  on: boolean;
  tone?: "on" | "warn" | "cold";
}) {
  return <span className={`lamp is-${tone}${on ? " is-lit" : ""}`}>{label}</span>;
}

function ElectronicsWidget(_props: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  if (!player) return <Empty />;
  const e = player.electronics;
  const eng = player.engine;
  return (
    <div className="elec">
      <div className="elec-lamps">
        <Lamp label={t("elec.tc")} on={e.tc_active} />
        <Lamp label={t("elec.abs")} on={e.abs_active} />
        <Lamp label={t("elec.limiter")} on={eng.speed_limiter} tone="warn" />
        <Lamp label={t("elec.headlights")} on={e.headlights} tone="cold" />
        <Lamp label={t("elec.wiper")} on={e.wiper > 0} tone="cold" />
        <Lamp label={t("elec.liftCoast")} on={e.lift_and_coast > 0} tone="warn" />
      </div>
      <div className="elec-grid">
        <Setting label={t("elec.tc")} value={e.tc} max={e.tc_max} active={e.tc_active} />
        <Setting label={t("elec.tcSlip")} value={e.tc_slip} max={e.tc_slip_max} />
        <Setting label={t("elec.tcCut")} value={e.tc_cut} max={e.tc_cut_max} />
        <Setting label={t("elec.abs")} value={e.abs} max={e.abs_max} active={e.abs_active} />
        <Setting label={t("elec.map")} value={e.motor_map} max={e.motor_map_max} />
        <Setting
          label={t("elec.migration")}
          value={e.brake_migration}
          max={e.brake_migration_max}
        />
        <Setting label={t("elec.arbFront")} value={e.arb_front} max={e.arb_front_max} />
        <Setting label={t("elec.arbRear")} value={e.arb_rear} max={e.arb_rear_max} />
        <div className="setting">
          <span className="setting-label dim">{t("elec.bias")}</span>
          <span className="setting-value num">
            {fmtNum((1 - e.brake_bias_rear) * 100, 1)}
            <small>%</small>
          </span>
        </div>
      </div>
    </div>
  );
}

export const electronicsWidget = registerWidget({
  id: "electronics",
  titleKey: "widget.electronics",
  minW: 2,
  minH: 3,
  defaultSize: { w: 5, h: 4 },
  mobileSize: { w: 4, h: 4 },
  component: ElectronicsWidget,
});
