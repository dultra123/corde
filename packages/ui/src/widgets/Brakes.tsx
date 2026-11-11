import { useT } from "../i18n";
import { BRAKE_SCALE, tempColor } from "../lib/color";
import { fmtNum } from "../lib/format";
import { usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const MAX_C = 800;

function BrakeBar({ temp, label }: { temp: number; label: string }) {
  const color = tempColor(temp, BRAKE_SCALE);
  const fill = Math.max(0.02, Math.min(1, temp / MAX_C));
  return (
    <div className="brake">
      <div className="brake-col">
        <div className="brake-fill" style={{ height: `${fill * 100}%`, background: color }} />
        <div
          className="brake-opt"
          style={{
            bottom: `${(BRAKE_SCALE.optLow / MAX_C) * 100}%`,
            height: `${((BRAKE_SCALE.optHigh - BRAKE_SCALE.optLow) / MAX_C) * 100}%`,
          }}
        />
      </div>
      <div className="brake-val num" style={{ color }}>
        {fmtNum(temp, 0)}°
      </div>
      <div className="brake-label dim">{label}</div>
    </div>
  );
}

function BrakesWidget(_props: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  if (!player) return <Empty />;
  const [fl, fr, rl, rr] = player.wheels;
  const rear = player.electronics.brake_bias_rear;
  const front = 1 - rear;
  const absOn = player.electronics.abs_active;
  return (
    <div className="brakes">
      <div className="brakes-grid">
        <BrakeBar temp={fl.brake_temp_c} label="AV G" />
        <BrakeBar temp={fr.brake_temp_c} label="AV D" />
        <BrakeBar temp={rl.brake_temp_c} label="AR G" />
        <BrakeBar temp={rr.brake_temp_c} label="AR D" />
      </div>
      <div className="brakes-bias">
        <div className="brakes-bias-head">
          <span className="dim">{t("brakes.bias")}</span>
          {absOn && <span className="pill is-on">ABS</span>}
        </div>
        <div className="bias-bar">
          <div className="bias-front" style={{ width: `${front * 100}%` }} />
          <span className="bias-txt num is-front">
            {t("brakes.front")} {fmtNum(front * 100, 1)}
          </span>
          <span className="bias-txt num is-rear">
            {fmtNum(rear * 100, 1)} {t("brakes.rear")}
          </span>
        </div>
      </div>
    </div>
  );
}

export const brakesWidget = registerWidget({
  id: "brakes",
  titleKey: "widget.brakes",
  minW: 2,
  minH: 3,
  defaultSize: { w: 2, h: 6 },
  mobileSize: { w: 2, h: 5 },
  component: BrakesWidget,
});
