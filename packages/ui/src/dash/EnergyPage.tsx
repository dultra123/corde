import { useT } from "../i18n";
import {
  energyPanels,
  FuelPanel,
  HybridPanel,
  SESSION,
  SessionPanel,
  VirtualEnergyPanel,
  WeatherPanel,
} from "./blocks/energy";
import { PageHead, type PageProps } from "./parts";

/** Page 3 — « Carburant & énergie »: autonomy, hybrid, what is left of the session, weather. */
export function EnergyPage({ frame, laps }: PageProps) {
  const { t } = useT();
  const s = frame?.session ?? null;
  const { ve, hybrid } = energyPanels(frame);
  return (
    <div className="de">
      <PageHead
        title={t("dash.page.energy")}
        info={
          s ? `${t(SESSION[s.session_type])}${s.track_name ? ` · ${s.track_name}` : ""}` : undefined
        }
      />
      <div className="de-grid">
        <FuelPanel frame={frame} laps={laps} />

        {(ve || hybrid) && (
          <div className="de-col">
            {ve && <VirtualEnergyPanel frame={frame} laps={laps} />}
            {hybrid && <HybridPanel frame={frame} />}
          </div>
        )}

        <div className="de-col">
          <SessionPanel frame={frame} laps={laps} />
          <WeatherPanel frame={frame} />
        </div>
      </div>
    </div>
  );
}
