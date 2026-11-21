import { brakeTemp, frontBias } from "../dash/model";
import { useT } from "../i18n";
import { BRAKE_SCALE, tempColor } from "../lib/color";
import { fmtNum } from "../lib/format";
import { usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const MAX_C = 800;

function BrakeBar({ temp, label }: { temp: number; label: string }) {
  // A game that publishes no brake temperature gives 0: shown as « not known », not as cold.
  const known = brakeTemp(temp) !== null;
  const color = known ? tempColor(temp, BRAKE_SCALE) : undefined;
  const fill = known ? Math.max(0.02, Math.min(1, temp / MAX_C)) : 0;
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
        {known ? `${fmtNum(temp, 0)}°` : "—"}
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
  // `null`: the game does not publish the bias (shown as « — », not as « 100 % front »).
  const front = frontBias(player.electronics);
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
          {front === null ? (
            <span className="bias-txt num is-none dim">—</span>
          ) : (
            <>
              <div className="bias-front" style={{ width: `${front}%` }} />
              <span className="bias-txt num is-front">
                {t("brakes.front")} {fmtNum(front, 1)}
              </span>
              <span className="bias-txt num is-rear">
                {fmtNum(100 - front, 1)} {t("brakes.rear")}
              </span>
            </>
          )}
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
