import {
  CarStripBlock,
  CurrentLapBlock,
  DeltaBlock,
  GapsBlock,
  GearBlock,
  LapRowsBlock,
  PositionBlock,
  RevBlock,
  SectorsBlock,
} from "./blocks/course";
import type { PageProps } from "./parts";

/**
 * Page 1 — « Course »: revs, position, laps, gear and speed, delta, sectors, gaps, the car. The
 * blocks are those the Studio offers one by one (`blocks/course.tsx`).
 */
export function CoursePage({ frame, laps, standings, flags, active, idle }: PageProps) {
  return (
    <div className="dc">
      <RevBlock flags={flags} active={active} />

      <section className="dpanel dc-left">
        <PositionBlock frame={frame} standings={standings} />
        <LapRowsBlock frame={frame} />
      </section>

      <GearBlock active={active} idle={idle} />

      <section className="dpanel dc-right">
        <DeltaBlock active={active} />
        <CurrentLapBlock frame={frame} active={active} />
        <SectorsBlock frame={frame} />
        <GapsBlock frame={frame} />
      </section>

      <CarStripBlock frame={frame} laps={laps} />
    </div>
  );
}
