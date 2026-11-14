import type { LapRecord, Standing, TelemetryFrame } from "@corde/protocol";
import { useT } from "../../i18n";
import { classColor } from "../../lib/color";
import { fmtDelta, fmtLapTime } from "../../lib/format";
import { lapNo } from "../../lib/laps";
import { currentSectors } from "../../widgets/shared/sectors";
import { relativeRows } from "../../widgets/shared/standings";
import { Lab } from "../parts";

/** The blocks of the « Chronos & écarts » page (`TimingPage`), also offered by the Studio. */

/** How many laps the « Derniers tours » list shows by default. */
export const LAST_LAPS = 5;

/** The sectors in detail: in progress, last lap, best, and the lap times. */
export function SectorTable({ frame }: { frame: TelemetryFrame | null }) {
  const { t } = useT();
  const lap = frame?.player?.lap ?? null;
  const cur = lap ? currentSectors(lap) : null;
  const lastS = lap ? [lap.last_s1, lap.last_s2, lap.last_s3] : [null, null, null];
  const bestS = lap ? [lap.best_s1, lap.best_s2, lap.best_s3] : [null, null, null];
  const best = lap?.best_lap_s ?? null;
  return (
    <section className="dpanel dk-sectors">
      <table className="dk-table">
        <caption className="sr-only">{t("dash.sectors")}</caption>
        <thead>
          <tr>
            <td />
            <th scope="col">
              <Lab>{t("dash.now")}</Lab>
            </th>
            <th scope="col">
              <Lab>{t("laps.last")}</Lab>
            </th>
            <th scope="col">
              <Lab>{t("laps.best")}</Lab>
            </th>
          </tr>
        </thead>
        <tbody>
          {[0, 1, 2].map((i) => {
            const c = cur?.[i];
            return (
              <tr key={i}>
                <th scope="row">
                  <Lab>S{i + 1}</Lab>
                </th>
                <td className="num dk-td" data-tone={c?.tone ?? "none"}>
                  {fmtLapTime(c?.value ?? null, { ms: 3 })}
                </td>
                <td className="num dk-td">{fmtLapTime(lastS[i] ?? null, { ms: 3 })}</td>
                <td className="num dk-td is-best">{fmtLapTime(bestS[i] ?? null, { ms: 3 })}</td>
              </tr>
            );
          })}
          <tr className="is-total">
            <th scope="row">
              <Lab>{t("laps.lap")}</Lab>
            </th>
            <td className="num dk-td">
              {fmtLapTime(lap?.current_lap_s ?? null, { ms: 1, minutes: true })}
            </td>
            <td className="num dk-td">{fmtLapTime(lap?.last_lap_s ?? null)}</td>
            <td className="num dk-td is-best">{fmtLapTime(best)}</td>
          </tr>
        </tbody>
      </table>
    </section>
  );
}

/** The last laps, the best in purple, invalid ones struck through. */
export function LastLaps({
  frame,
  laps,
  count = LAST_LAPS,
}: {
  frame: TelemetryFrame | null;
  laps: LapRecord[];
  count?: number;
}) {
  const { t } = useT();
  const best = frame?.player?.lap.best_lap_s ?? null;
  const recent = [...laps].reverse().slice(0, count);
  return (
    <section className="dpanel dk-laps">
      <Lab>{t("laps.history")}</Lab>
      {recent.length === 0 ? (
        <p className="dk-none">{t("dash.noLaps")}</p>
      ) : (
        <ol className="dk-list">
          {recent.map((l) => {
            const isBest = best !== null && Math.abs(l.lap_time_s - best) < 0.0005;
            return (
              <li key={l.lap_number} className={l.valid ? undefined : "is-invalid"}>
                <span className="num dk-no">{lapNo(l.lap_number)}</span>
                <span className={`num dk-time${isBest ? " is-best" : ""}`}>
                  {fmtLapTime(l.lap_time_s)}
                </span>
                <span className="num dk-diff">
                  {best !== null && !isBest ? fmtDelta(l.lap_time_s - best, 2) : ""}
                </span>
                <span className="dk-mark">
                  {l.valid ? "" : "✕"}
                  {!l.valid && <span className="sr-only">{t("laps.invalid")}</span>}
                </span>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}

/** The cars around the driver by position (two ahead, two behind by default). */
export function AroundMe({ standings, span = 2 }: { standings: Standing[]; span?: number }) {
  const { t, nOf } = useT();
  const around = relativeRows(standings, span).filter((r) => !r.isClassLeaderOutsideWindow);
  return (
    <section className="dpanel dk-around">
      <Lab>{t("dash.around")}</Lab>
      {around.length === 0 ? (
        <p className="dk-none">—</p>
      ) : (
        <ol className="dk-list">
          {around.map(({ s, gapS, gapLaps }) => (
            <li key={s.id} className={s.is_player ? "is-me" : undefined}>
              <span className="num dk-no">{s.place}</span>
              <i className="dk-class" style={{ background: classColor(s.vehicle_class) }} />
              <span className="dk-name">{s.driver_name}</span>
              <span className="num dk-gap">
                {s.is_player
                  ? ""
                  : gapLaps !== 0
                    ? `${gapLaps < 0 ? "−" : "+"}${nOf(Math.abs(gapLaps), "standings.lap", "standings.laps")}`
                    : `${gapS < 0 ? "−" : "+"}${Math.abs(gapS).toFixed(1)}`}
              </span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}
