import { useEffect, useRef } from "react";
import { IconClose } from "../dashboard/icons";
import { useT } from "../i18n";
import { routeHash } from "../nav/routes";
import { type ShownReminder, useRaces } from "../store/races";
import { useSettings } from "../store/settings";
import { hm } from "./format";
import { useNow } from "./hooks";
import { isUrgent, minutesLeft } from "./model";
import { installAudioUnlock, playChime } from "./sound";

function ReminderLine({ r, now }: { r: ShownReminder; now: number }) {
  const { t } = useT();
  const dismiss = useRaces((s) => s.dismiss);
  let head: string;
  if (r.kind === "eve") head = t("races.banner.eve");
  else if (r.kind === "day") head = t("races.banner.day");
  else {
    const n = minutesLeft(r.start_ms, now);
    head = n > 0 ? t("races.banner.in", { n }) : t("races.banner.now");
  }
  const what =
    r.kind === "lead" ? `${r.name}, ${r.track}, ${hm(r.start_ms)}` : `${r.name}, ${r.track}`;
  return (
    <div className={`race-banner${isUrgent(r) ? " is-urgent" : ""}`} role="alert">
      <span className="race-banner-dot" aria-hidden="true" />
      <p className="race-banner-text">
        <b>{head}</b> — {what}
        <span className="race-banner-hint">
          {r.kind === "lead" ? t("races.banner.hint") : t("races.banner.hintEvent")}
        </span>
      </p>
      <a
        className="race-banner-view"
        href={routeHash({ page: "races" })}
        onClick={() => dismiss(r.key)}
      >
        {t("races.banner.view")}
      </a>
      <button
        type="button"
        className="race-banner-close"
        aria-label={t("races.banner.close")}
        title={t("races.banner.close")}
        onClick={() => dismiss(r.key)}
      >
        <IconClose />
      </button>
    </div>
  );
}

/**
 * The reminders of picked races, on every page (PC and phone): a banner at the top and a short
 * sound (`races/sound`), when the server sends `race_reminder`.
 */
export function ReminderBanner() {
  const reminders = useRaces((s) => s.reminders);
  const sound = useSettings((s) => s.raceSound);
  const now = useNow(15_000);
  const played = useRef(new Set<string>());
  const { t } = useT();

  useEffect(() => {
    installAudioUnlock();
  }, []);

  useEffect(() => {
    for (const r of reminders) {
      if (played.current.has(r.key)) continue;
      played.current.add(r.key);
      if (sound) playChime(isUrgent(r));
    }
  }, [reminders, sound]);

  if (reminders.length === 0) return null;
  return (
    <section className="race-banners" aria-label={t("races.banner.label")}>
      {reminders.map((r) => (
        <ReminderLine key={r.key} r={r} now={Math.max(now, r.received_ms)} />
      ))}
    </section>
  );
}
