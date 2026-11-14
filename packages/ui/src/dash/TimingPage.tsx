import { useT } from "../i18n";
import { lapNo } from "../lib/laps";
import { AroundMe, LastLaps, SectorTable } from "./blocks/timing";
import { PageHead, type PageProps } from "./parts";

/** Page 4 — « Chronos & écarts »: the sectors in detail, the last laps, the cars around. */
export function TimingPage({ frame, laps, standings }: PageProps) {
  const { t } = useT();
  const lap = frame?.player?.lap ?? null;
  return (
    <div className="dk">
      <PageHead
        title={t("dash.page.timing")}
        info={
          lap ? `${t("laps.lap")} ${lapNo(lap.lap_number)} · S${lap.current_sector}` : undefined
        }
      />
      <div className="dk-grid">
        <SectorTable frame={frame} />
        <LastLaps frame={frame} laps={laps} />
        <AroundMe standings={standings} />
      </div>
    </div>
  );
}
