import { useMemo } from "react";
import { useT } from "../i18n";
import { COLORS } from "../lib/color";
import { fmtGear, fmtNum } from "../lib/format";
import { usePlayer, useTelemetry } from "../store/telemetry";
import { configNumber, registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";
import { Trace } from "./shared/Trace";

const RPM_SEGMENTS = 28;

function Pedal({
  value,
  raw,
  color,
  label,
  cut,
}: {
  value: number;
  raw: number;
  color: string;
  label: string;
  cut?: boolean;
}) {
  return (
    <div className="pedal">
      <div className="pedal-col">
        <div
          className="pedal-fill"
          style={{ height: `${Math.min(1, Math.max(0, value)) * 100}%`, background: color }}
        />
        {cut && Math.abs(raw - value) > 0.02 && (
          <div
            className="pedal-raw"
            style={{ bottom: `${Math.min(1, Math.max(0, raw)) * 100}%` }}
          />
        )}
      </div>
      <div className="pedal-val num">{Math.round(value * 100)}</div>
      <div className="pedal-label dim">{label}</div>
    </div>
  );
}

function Wheel({ angle }: { angle: number }) {
  return (
    <svg className="wheel" viewBox="0 0 100 100" style={{ transform: `rotate(${angle}deg)` }}>
      <title>steering</title>
      <circle cx="50" cy="50" r="44" fill="none" stroke="currentColor" strokeWidth="9" />
      <path
        d="M50 50 L50 92 M50 50 L10 42 M50 50 L90 42"
        stroke="currentColor"
        strokeWidth="9"
        strokeLinecap="round"
      />
      <circle cx="50" cy="50" r="10" fill="currentColor" />
      <path d="M50 6 L50 20" stroke={COLORS.accent} strokeWidth="9" strokeLinecap="round" />
    </svg>
  );
}

function RpmBar({ rpm, max }: { rpm: number; max: number }) {
  const frac = max > 0 ? Math.min(1, rpm / max) : 0;
  const lit = Math.round(frac * RPM_SEGMENTS);
  const segs = useMemo(() => Array.from({ length: RPM_SEGMENTS }, (_, i) => i), []);
  return (
    <div className={`rpm${frac > 0.96 ? " is-shift" : ""}`}>
      {segs.map((i) => {
        const pos = (i + 1) / RPM_SEGMENTS;
        const zone = pos > 0.95 ? "hot" : pos > 0.86 ? "warm" : pos > 0.7 ? "optimal" : "neutral";
        return <div key={i} className={`rpm-seg is-${zone}${i < lit ? " is-lit" : ""}`} />;
      })}
    </div>
  );
}

function InputsWidget({ config }: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  const buffers = useTelemetry((s) => s.buffers);
  const windowS = configNumber(config, "window", 10);
  const series = useMemo(
    () => [
      { buffer: buffers.throttle, color: COLORS.optimal, width: 1.6 },
      { buffer: buffers.brake, color: COLORS.hot, width: 1.6 },
      {
        buffer: buffers.steering,
        color: "rgba(233,237,242,0.55)",
        width: 1,
        map: (v: number) => (v + 1) / 2,
      },
    ],
    [buffers],
  );
  if (!player) return <Empty />;
  const { inputs, engine, motion, electronics } = player;
  return (
    <div className="inputs">
      <div className="inputs-top">
        <RpmBar rpm={engine.rpm} max={engine.max_rpm} />
        <div className="inputs-rpm num dim">
          {fmtNum(engine.rpm, 0)} <small>{t("inputs.rpm")}</small>
        </div>
      </div>
      <div className="inputs-mid">
        <div className="inputs-gear">
          <div className="gear num">{fmtGear(engine.gear)}</div>
          <div className="speed num">
            {fmtNum(motion.speed_kmh, 0)}
            <small> {t("inputs.speed")}</small>
          </div>
        </div>
        <div className="inputs-steer">
          <Wheel angle={inputs.steering_angle_deg} />
          <div className="num dim">{fmtNum(inputs.steering_angle_deg, 0)}°</div>
        </div>
        <div className="inputs-pedals">
          <Pedal
            value={inputs.throttle}
            raw={inputs.throttle_raw}
            color={COLORS.optimal}
            label={t("inputs.throttle")}
            cut={electronics.tc_active}
          />
          <Pedal
            value={inputs.brake}
            raw={inputs.brake_raw}
            color={COLORS.hot}
            label={t("inputs.brake")}
            cut={electronics.abs_active}
          />
          <Pedal
            value={inputs.clutch}
            raw={inputs.clutch_raw}
            color={COLORS.cold}
            label={t("inputs.clutch")}
          />
        </div>
        <div className="inputs-flags">
          {electronics.tc_active && <span className="pill is-on">TC</span>}
          {electronics.abs_active && <span className="pill is-on">ABS</span>}
          {engine.speed_limiter && <span className="pill is-warn">{t("inputs.limiter")}</span>}
        </div>
      </div>
      <div className="inputs-trace">
        <Trace series={series} windowS={windowS} />
      </div>
    </div>
  );
}

export const inputsWidget = registerWidget({
  id: "inputs",
  titleKey: "widget.inputs",
  minW: 3,
  minH: 4,
  defaultSize: { w: 4, h: 6 },
  mobileSize: { w: 4, h: 5 },
  component: InputsWidget,
  defaultConfig: { window: 10 },
  configFields: [
    {
      key: "window",
      labelKey: "inputs.window",
      options: [
        { value: "5", labelKey: "inputs.window5" },
        { value: "10", labelKey: "inputs.window10" },
        { value: "20", labelKey: "inputs.window20" },
      ],
    },
  ],
});
