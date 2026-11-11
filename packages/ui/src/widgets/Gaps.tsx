import { useT } from "../i18n";
import { COLORS, classColor } from "../lib/color";
import { fmtGap, fmtOrdinal } from "../lib/format";
import { usePlayer, useStandings } from "../store/telemetry";
import { registerWidget, type WidgetProps } from "./registry";
import { Empty } from "./shared/Empty";

function GapsWidget(_props: WidgetProps) {
  const { t, lang } = useT();
  const player = usePlayer();
  const standings = useStandings();
  if (!player) return <Empty />;
  const me = standings.find((s) => s.is_player);
  const ahead = standings.find((s) => s.place === player.place - 1);
  const behind = standings.find((s) => s.place === player.place + 1);
  const classCount = me ? standings.filter((s) => s.class_name === me.class_name).length : 0;
  const g = player.gaps;
  const trend = (place: number | null, track: number | null) =>
    place !== null && track !== null && Math.abs(place - track) < 0.05 ? null : track;
  return (
    <div className="gaps">
      <div className="gaps-block is-ahead">
        <div className="gaps-label dim">{t("gaps.ahead")}</div>
        <div
          className="gaps-value num"
          style={{ color: g.place_ahead_s === null ? COLORS.faint : COLORS.text }}
        >
          {g.place_ahead_s === null ? t("gaps.noCar") : `−${fmtGap(g.place_ahead_s)}`}
        </div>
        {ahead && (
          <div className="gaps-who">
            <span className="gaps-class" style={{ background: classColor(ahead.vehicle_class) }} />
            <span className="num dim">{ahead.place}</span> {ahead.driver_name}
            {ahead.in_pits && <span className="pill is-warn">{t("standings.pit")}</span>}
          </div>
        )}
        {trend(g.place_ahead_s, g.car_ahead_s) !== null && (
          <div className="gaps-track faint">
            {t("gaps.onTrack")} −{fmtGap(g.car_ahead_s)}
          </div>
        )}
      </div>

      <div className="gaps-place">
        <div className="gaps-pos num">
          {player.place}
          <small>{fmtOrdinal(player.place, lang).replace(String(player.place), "")}</small>
        </div>
        <div className="gaps-sub dim">
          {me ? (
            <>
              <span className="num">{fmtOrdinal(me.class_place, lang)}</span> {t("gaps.inClass")}{" "}
              <span className="gaps-class" style={{ background: classColor(me.vehicle_class) }} />
              {me.class_name} · {classCount}
            </>
          ) : (
            `${t("gaps.ofCars")} ${standings.length}`
          )}
        </div>
      </div>

      <div className="gaps-block is-behind">
        <div className="gaps-label dim">{t("gaps.behind")}</div>
        <div
          className="gaps-value num"
          style={{ color: g.place_behind_s === null ? COLORS.faint : COLORS.text }}
        >
          {g.place_behind_s === null ? t("gaps.noCar") : `+${fmtGap(g.place_behind_s)}`}
        </div>
        {behind && (
          <div className="gaps-who">
            <span className="gaps-class" style={{ background: classColor(behind.vehicle_class) }} />
            <span className="num dim">{behind.place}</span> {behind.driver_name}
            {behind.in_pits && <span className="pill is-warn">{t("standings.pit")}</span>}
          </div>
        )}
        {trend(g.place_behind_s, g.car_behind_s) !== null && (
          <div className="gaps-track faint">
            {t("gaps.onTrack")} +{fmtGap(g.car_behind_s)}
          </div>
        )}
      </div>
    </div>
  );
}

export const gapsWidget = registerWidget({
  id: "gaps",
  titleKey: "widget.gaps",
  minW: 2,
  minH: 3,
  defaultSize: { w: 2, h: 6 },
  mobileSize: { w: 4, h: 3 },
  component: GapsWidget,
});
