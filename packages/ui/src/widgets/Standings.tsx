import type { Standing } from "@corde/protocol";
import { useT } from "../i18n";
import { classColor } from "../lib/color";
import { fmtLapTime } from "../lib/format";
import { useStandings } from "../store/telemetry";
import { configString, registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

interface Row {
  s: Standing;
  /** Seconds to the player (relative) or to the leader (full); negative = ahead. */
  gapS: number;
  /** Whole laps of difference (same sign convention). */
  gapLaps: number;
  isClassLeaderOutsideWindow?: boolean;
}

/** 3 cars ahead and 3 behind the player (by position) plus the class leader when out of view. */
export function relativeRows(standings: Standing[], around = 3): Row[] {
  const sorted = [...standings].filter((s) => !s.in_garage).sort((a, b) => a.place - b.place);
  const pi = sorted.findIndex((s) => s.is_player);
  if (pi < 0)
    return sorted
      .slice(0, around * 2 + 1)
      .map((s) => ({ s, gapS: s.time_behind_leader_s, gapLaps: s.laps_behind_leader }));
  const me = sorted[pi] as Standing;
  const rows: Row[] = [];
  // cumulative intervals from the player outwards
  let acc = 0;
  const ahead: Row[] = [];
  for (let i = pi; i > 0 && ahead.length < around; i -= 1) {
    const car = sorted[i - 1] as Standing;
    acc += (sorted[i] as Standing).time_behind_next_s;
    ahead.unshift({
      s: car,
      gapS: -acc,
      gapLaps: -(me.laps_behind_leader - car.laps_behind_leader),
    });
  }
  acc = 0;
  const behind: Row[] = [];
  for (let i = pi + 1; i < sorted.length && behind.length < around; i += 1) {
    const car = sorted[i] as Standing;
    acc += car.time_behind_next_s;
    behind.push({ s: car, gapS: acc, gapLaps: car.laps_behind_leader - me.laps_behind_leader });
  }
  rows.push(...ahead, { s: me, gapS: 0, gapLaps: 0 }, ...behind);
  const leader = sorted.find((s) => s.class_name === me.class_name && s.class_place === 1);
  if (leader && !rows.some((r) => r.s.id === leader.id)) {
    let gap = 0;
    for (let i = sorted.indexOf(leader) + 1; i <= pi; i += 1)
      gap += (sorted[i] as Standing).time_behind_next_s;
    rows.unshift({
      s: leader,
      gapS: -gap,
      gapLaps: -(me.laps_behind_leader - leader.laps_behind_leader),
      isClassLeaderOutsideWindow: true,
    });
  }
  return rows;
}

export function fullRows(standings: Standing[]): Row[] {
  return [...standings]
    .filter((s) => !s.in_garage)
    .sort((a, b) => a.place - b.place)
    .map((s) => ({ s, gapS: s.time_behind_leader_s, gapLaps: s.laps_behind_leader }));
}

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
            {s.in_pits && <span className="pill is-warn">{t("standings.pit")}</span>}
            {s.flag_blue && !s.in_pits && <span className="pill is-blue">B</span>}
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
