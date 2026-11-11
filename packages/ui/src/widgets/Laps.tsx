import type { LapRecord, LapTiming } from "@corde/protocol";
import { useT } from "../i18n";
import { COLORS, deltaColor } from "../lib/color";
import { fmtDelta, fmtLapTime } from "../lib/format";
import { lapNo, lapStatus, lapStatusHintKey } from "../lib/laps";
import { useLaps, usePlayer } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

const DELTA_BAR_SCALE_S = 2;

type SectorTone = "best" | "faster" | "slower" | "running" | "none";

interface SectorView {
  value: number | null;
  tone: SectorTone;
}

/** Colour of a completed sector: purple = personal best sector, green/red vs the best lap's sector. */
function tone(
  value: number | null,
  bestLapSector: number | null,
  bestSector: number | null,
): SectorTone {
  if (value === null) return "none";
  if (bestSector !== null && value <= bestSector + 0.0005) return "best";
  if (bestLapSector === null) return "none";
  return value < bestLapSector ? "faster" : "slower";
}

/** The three sectors of the lap in progress (running sector shown live). */
export function currentSectors(lap: LapTiming): [SectorView, SectorView, SectorView] {
  const bestLapS3 =
    lap.best_lap_s !== null && lap.best_lap_s1 !== null && lap.best_lap_s2 !== null
      ? lap.best_lap_s - lap.best_lap_s1 - lap.best_lap_s2
      : null;
  const s1: SectorView =
    lap.cur_s1 !== null
      ? { value: lap.cur_s1, tone: tone(lap.cur_s1, lap.best_lap_s1, lap.best_s1) }
      : { value: lap.current_sector === 1 ? lap.current_lap_s : null, tone: "running" };
  const s2: SectorView =
    lap.cur_s2 !== null
      ? { value: lap.cur_s2, tone: tone(lap.cur_s2, lap.best_lap_s2, lap.best_s2) }
      : {
          value:
            lap.current_sector === 2 && lap.cur_s1 !== null ? lap.current_lap_s - lap.cur_s1 : null,
          tone: "running",
        };
  const s3: SectorView =
    lap.current_sector === 3 && lap.cur_s1 !== null && lap.cur_s2 !== null
      ? { value: lap.current_lap_s - lap.cur_s1 - lap.cur_s2, tone: "running" }
      : lap.current_sector === 1 && lap.last_s3 !== null
        ? // just crossed the line: keep showing the sector 3 that was completed
          { value: lap.last_s3, tone: tone(lap.last_s3, bestLapS3, lap.best_s3) }
        : { value: null, tone: "none" };
  return [s1, s2, s3];
}

const TONE_COLOR: Record<SectorTone, string> = {
  best: COLORS.best,
  faster: COLORS.faster,
  slower: COLORS.slower,
  running: COLORS.dim,
  none: COLORS.faint,
};

function HistoryRow({ lap, best }: { lap: LapRecord; best: number | null }) {
  const { t } = useT();
  const isBest = best !== null && Math.abs(lap.lap_time_s - best) < 0.0005;
  return (
    <div className={`lap-row${lap.valid ? "" : " is-invalid"}`}>
      <span className="lap-no num dim">{lapNo(lap.lap_number)}</span>
      <span className="lap-time num" style={{ color: isBest ? COLORS.best : undefined }}>
        {fmtLapTime(lap.lap_time_s)}
      </span>
      <span className="lap-sect num dim">{fmtLapTime(lap.s1, { ms: 1 })}</span>
      <span className="lap-sect num dim">{fmtLapTime(lap.s2, { ms: 1 })}</span>
      <span className="lap-sect num dim">{fmtLapTime(lap.s3, { ms: 1 })}</span>
      <span className="lap-fuel num faint">
        {lap.fuel_used_l > 0 ? `${lap.fuel_used_l.toFixed(2)} L` : ""}
      </span>
      <span
        className={`lap-valid ${lap.valid ? "is-ok" : "is-ko"}`}
        title={t(lapStatusHintKey(lapStatus(lap)))}
      >
        {lap.valid ? "" : "✕"}
      </span>
    </div>
  );
}

function LapsWidget(_props: WidgetProps) {
  const { t } = useT();
  const player = usePlayer();
  const laps = useLaps();
  if (!player) return <Empty />;
  const lap = player.lap;
  const delta = lap.best_lap_s !== null ? lap.delta_best_s : null;
  const dColor = deltaColor(delta);
  const barFrac = delta === null ? 0 : Math.max(-1, Math.min(1, delta / DELTA_BAR_SCALE_S));
  const sectors = currentSectors(lap);
  const history = [...laps].reverse();
  return (
    <div className={`laps${lap.lap_invalidated ? " is-invalid" : ""}`}>
      <div className="laps-head">
        <span className="dim">
          {t("laps.lap")} <span className="num">{lapNo(lap.lap_number)}</span>
          {" · "}
          {t("laps.sector")}
          <span className="num">{lap.current_sector}</span>
        </span>
        {lap.in_pit_lane ? (
          <span className="pill is-warn">{t("laps.inPit")}</span>
        ) : lap.lap_invalidated ? (
          <span className="pill is-bad">{t("laps.invalid")}</span>
        ) : null}
      </div>

      <div className="laps-current num">
        {fmtLapTime(lap.current_lap_s, { ms: 1, minutes: true })}
      </div>

      <div className="laps-delta num" style={{ color: dColor }}>
        {delta === null ? "—" : fmtDelta(delta, 2)}
      </div>
      <div className="delta-bar">
        <div
          className="delta-bar-fill"
          style={{
            background: dColor,
            left: barFrac < 0 ? `${50 + barFrac * 50}%` : "50%",
            width: `${Math.abs(barFrac) * 50}%`,
          }}
        />
      </div>

      <div className="laps-refs">
        <div>
          <span className="dim">{t("laps.last")}</span>
          <span className="num">{fmtLapTime(lap.last_lap_s)}</span>
        </div>
        <div>
          <span className="dim">{t("laps.best")}</span>
          <span className="num" style={{ color: COLORS.best }}>
            {fmtLapTime(lap.best_lap_s)}
          </span>
        </div>
        <div>
          <span className="dim">{t("laps.estimated")}</span>
          <span className="num">{fmtLapTime(lap.estimated_lap_s, { ms: 1 })}</span>
        </div>
      </div>

      <div className="laps-sectors">
        {sectors.map((s, i) => (
          <div key={i} className={`sector is-${s.tone}`} style={{ color: TONE_COLOR[s.tone] }}>
            <span className="sector-label">S{i + 1}</span>
            <span className="num">{fmtLapTime(s.value, { ms: 1 })}</span>
          </div>
        ))}
      </div>

      <div className="laps-history">
        {history.length === 0 ? (
          <div className="faint laps-nohistory">{t("laps.noHistory")}</div>
        ) : (
          history.map((l) => <HistoryRow key={l.lap_number} lap={l} best={lap.best_lap_s} />)
        )}
      </div>
    </div>
  );
}

export const lapsWidget = registerWidget({
  id: "laps",
  titleKey: "widget.laps",
  minW: 2,
  minH: 4,
  defaultSize: { w: 3, h: 6 },
  mobileSize: { w: 4, h: 5 },
  component: LapsWidget,
});
