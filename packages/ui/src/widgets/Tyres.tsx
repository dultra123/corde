import type { CompoundType, Wheel } from "@corde/protocol";
import { useT } from "../i18n";
import { tempColor, tyreScale, wearColor } from "../lib/color";
import { fmtNum } from "../lib/format";
import { usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const compoundKey = (c: CompoundType) =>
  (
    ({
      soft: "compound.soft",
      medium: "compound.medium",
      hard: "compound.hard",
      wet: "compound.wet",
      unknown: "compound.unknown",
    }) as const
  )[c];

function Tyre({ wheel, side }: { wheel: Wheel; side: "left" | "right" }) {
  const { t } = useT();
  const scale = tyreScale(wheel.optimal_temp_c);
  const zones = wheel.temp_c;
  const bad = wheel.flat || wheel.detached;
  return (
    <div className={`tyre is-${side}${bad ? " is-bad" : ""}`}>
      <div className="tyre-tread" title={`${t("tyres.inner")} / ${t("tyres.outer")}`}>
        {zones.map((temp, i) => (
          <div key={i} className="tyre-zone num" style={{ background: tempColor(temp, scale) }}>
            {Math.round(temp)}
          </div>
        ))}
        {bad && (
          <div className="tyre-flag">{t(wheel.detached ? "tyres.detached" : "tyres.flat")}</div>
        )}
      </div>
      <div className="tyre-meta">
        <span className="num">
          {fmtNum(wheel.pressure_kpa, 0)}
          <small> kPa</small>
        </span>
        <span className="num" style={{ color: wearColor(wheel.wear_remaining) }}>
          {fmtNum(wheel.wear_remaining * 100, 0)}
          <small> %</small>
        </span>
      </div>
      <div className="tyre-carcass dim">
        {t("tyres.carcass")} <span className="num">{fmtNum(wheel.carcass_c, 0)}°</span>
      </div>
    </div>
  );
}

function TyresWidget(_props: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  if (!player) return <Empty />;
  const [fl, fr, rl, rr] = player.wheels;
  const optimal = fl.optimal_temp_c;
  const front = player.tyres.front_compound || t(compoundKey(fl.compound_type));
  const rear = player.tyres.rear_compound || t(compoundKey(rl.compound_type));
  return (
    <div className="tyres">
      <div className="tyres-sub dim">
        <span>
          {front}
          {rear !== front ? ` / ${rear}` : ""}
        </span>
        {optimal !== null && (
          <span>
            {t("tyres.optimal")} <span className="num">{Math.round(optimal)}°</span>
          </span>
        )}
      </div>
      <div className="tyres-car">
        <Tyre wheel={fl} side="left" />
        <div className="tyres-axle" />
        <Tyre wheel={fr} side="right" />
        <Tyre wheel={rl} side="left" />
        <div className="tyres-axle" />
        <Tyre wheel={rr} side="right" />
      </div>
    </div>
  );
}

export const tyresWidget = registerWidget({
  id: "tyres",
  titleKey: "widget.tyres",
  minW: 2,
  minH: 4,
  defaultSize: { w: 3, h: 6 },
  mobileSize: { w: 2, h: 5 },
  component: TyresWidget,
});
