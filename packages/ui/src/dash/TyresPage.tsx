import type { TranslationKey } from "../i18n";
import { useT } from "../i18n";
import { CORNERS, TyreCard, TyresCentre, tyresInfo } from "./blocks/tyres";
import { PageHead, type PageProps } from "./parts";

/** Page 2 — « Pneus & freins »: the four corners, the ideal window, the bias, the engine. */
export function TyresPage({ frame }: PageProps) {
  const { t } = useT();
  const wheels = frame?.player?.wheels ?? null;
  const info = tyresInfo(frame, t);
  return (
    <div className="dt">
      <PageHead title={t("dash.page.tyres")} info={info || undefined} />
      <div className="dt-grid">
        <TyreCard wheel={wheels?.[0] ?? null} corner={CORNERS[0] as TranslationKey} />
        <TyresCentre frame={frame} />
        <TyreCard wheel={wheels?.[1] ?? null} corner={CORNERS[1] as TranslationKey} />
        <TyreCard wheel={wheels?.[2] ?? null} corner={CORNERS[2] as TranslationKey} />
        <TyreCard wheel={wheels?.[3] ?? null} corner={CORNERS[3] as TranslationKey} />
      </div>
    </div>
  );
}
