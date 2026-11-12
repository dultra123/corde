import { useT } from "../i18n";
import type { Ratings } from "../net/races-api";
import { missing } from "./model";
import type { Cat, Level } from "./parse";

/** Small pieces shared by the Courses page and the home page's carousel. */

function RatingBadge({
  kind,
  value,
  locked,
}: {
  kind: "SR" | "DR";
  value: string;
  locked: boolean;
}) {
  const { t } = useT();
  return (
    <span
      className={`race-req is-${value.charAt(0).toLowerCase()}${locked ? " is-locked" : ""}`}
      title={locked ? t("races.req.locked") : undefined}
    >
      <i>{kind}</i>
      <b className="race-req-v">{value}</b>
      {locked && <span className="sr-only"> — {t("races.req.locked")}</span>}
    </span>
  );
}

/** « SR S2 » / « DR B2 »: the access requirement, marked when the driver's rank is below it. */
export function ReqBadges({ req, profile }: { req: Ratings | undefined; profile: Ratings }) {
  if (!req) return null;
  return (
    <>
      {req.sr && (
        <RatingBadge
          kind="SR"
          value={req.sr}
          locked={missing({ sr: req.sr, dr: "" }, profile).length > 0}
        />
      )}
      {req.dr && (
        <RatingBadge
          kind="DR"
          value={req.dr}
          locked={missing({ sr: "", dr: req.dr }, profile).length > 0}
        />
      )}
    </>
  );
}

/** The level of a daily race, the category otherwise. */
export function KindBadge({ cat, level }: { cat: Cat; level: Level | null }) {
  const { t } = useT();
  if (cat === "daily" && level) {
    return <span className={`race-kind lvl-${level}`}>{t(`races.level.${level}`)}</span>;
  }
  return <span className={`race-kind cat-${cat}`}>{t(`races.cat.${cat}.one`)}</span>;
}
