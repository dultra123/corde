import { useT } from "../i18n";
import { classColor } from "../lib/color";
import { fmtLapTime } from "../lib/format";
import { useStandings } from "../store/telemetry";
import { configString, registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";
import { fullRows, type StandingRow as Row, relativeRows } from "./shared/standings";

function Gap({
  row,
  relative,
  lapWord,
}: {
  row: Row;
  relative: boolean;
  lapWord: (n: number) => string;
}) {
  const { s, gapS, gapLaps } = row;
  if (s.is_player && relative) return <span className="st-gap num faint">—</span>;
  if (!relative && s.place === 1) return <span className="st-gap num dim">{lapWord(0)}</span>;
  if (gapLaps !== 0) {
    const n = Math.abs(gapLaps);
    return (
      <span className="st-gap num dim">
        {gapLaps < 0 ? "−" : "+"}
        {n} {lapWord(n)}
      </span>
    );
  }
  const sign = relative ? (gapS < 0 ? "−" : "+") : "+";
  return (
    <span className="st-gap num">
      {sign}
      {Math.abs(gapS).toFixed(1)}
    </span>
  );
}

function StandingsWidget({ config }: WidgetProps) {
  const { t } = useT();
  const standings = useStandings();
  const mode = configString(config, "mode", "relative");
  if (standings.length === 0) return <Empty textKey="widget.noData" />;
  const relative = mode !== "full";
  const rows = relative ? relativeRows(standings) : fullRows(standings);
  const lapWord = (n: number) =>
    n === 0 ? t("standings.leader") : n > 1 ? t("standings.laps") : t("standings.lap");
  return (
    <div className={`standings is-${relative ? "relative" : "full"}`}>
      {rows.map((row) => {
        const s = row.s;
        return (
          <div
            key={s.id}
            className={`st-row${s.is_player ? " is-me" : ""}${s.in_pits ? " is-pit" : ""}${row.isClassLeaderOutsideWindow ? " is-detached" : ""}`}
          >
            <span className="st-class" style={{ background: classColor(s.vehicle_class) }} />
            <span className="st-pos num">{s.place}</span>
            <span className="st-cpos num dim">{s.class_place}</span>
            <span className="st-name">
              {s.driver_name}
              <small className="st-car dim">{s.vehicle_name}</small>
            </span>
            {/* always one cell, so that a badge never pushes the row onto two lines */}
            <span className="st-flag">
              {s.in_pits && <span className="pill is-warn">{t("standings.pit")}</span>}
              {s.flag_blue && !s.in_pits && <span className="pill is-blue">B</span>}
            </span>
            <Gap row={row} relative={relative} lapWord={lapWord} />
            <span className="st-last num dim">{fmtLapTime(s.last_lap_s, { ms: 1 })}</span>
          </div>
        );
      })}
    </div>
  );
}

export const standingsWidget = registerWidget({
  id: "standings",
  titleKey: "widget.standings",
  minW: 3,
  minH: 3,
  defaultSize: { w: 4, h: 6 },
  mobileSize: { w: 4, h: 5 },
  component: StandingsWidget,
  defaultConfig: { mode: "relative" },
  configFields: [
    {
      key: "mode",
      labelKey: "standings.mode",
      options: [
        { value: "relative", labelKey: "standings.relative" },
        { value: "full", labelKey: "standings.full" },
      ],
    },
  ],
});
