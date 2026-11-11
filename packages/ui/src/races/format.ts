import { translate } from "../i18n";
import type { Lang } from "../store/settings";

/** Dates and times of the « Courses » page, in the device's local time. */

const DAY = 864e5;
const pad = (n: number) => String(n).padStart(2, "0");

/** `21:25`. */
export function hm(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

const dayKey = (ms: number) => new Date(ms).toDateString();
export const isToday = (ms: number, now: number) => dayKey(ms) === dayKey(now);
export const isTomorrow = (ms: number, now: number) => dayKey(ms) === dayKey(now + DAY);

const locale = (lang: Lang) => (lang === "fr" ? "fr-FR" : "en-GB");

/** `Aujourd'hui` / `Demain` / `jeudi 12 novembre`. */
export function dayLabel(ms: number, now: number, lang: Lang): string {
  if (isToday(ms, now)) return translate(lang, "races.day.today");
  if (isTomorrow(ms, now)) return translate(lang, "races.day.tomorrow");
  const s = new Intl.DateTimeFormat(locale(lang), {
    weekday: "long",
    day: "numeric",
    month: "long",
  }).format(new Date(ms));
  return s.charAt(0).toUpperCase() + s.slice(1);
}

/** The small day written above a start: nothing today, `dem.`, `jeu.`, `14 nov.`. */
export function chipDay(ms: number, now: number, lang: Lang): string {
  if (isToday(ms, now)) return "";
  if (isTomorrow(ms, now)) return translate(lang, "races.day.tomorrowShort");
  const days = (ms - now) / DAY;
  const opts: Intl.DateTimeFormatOptions =
    days < 6 ? { weekday: "short" } : { day: "numeric", month: "short" };
  return new Intl.DateTimeFormat(locale(lang), opts).format(new Date(ms));
}

/** `sam. 21 → dim. 22 nov.` */
export function dateRange(from: number, to: number, lang: Lang): string {
  const a = new Intl.DateTimeFormat(locale(lang), { weekday: "short", day: "numeric" }).format(
    new Date(from),
  );
  const b = new Intl.DateTimeFormat(locale(lang), {
    weekday: "short",
    day: "numeric",
    month: "short",
  }).format(new Date(to));
  return `${a} → ${b}`;
}

/** `dans 12 min`, `dans 1 h 05`, `dans 3 j`, `maintenant`, `en cours`. */
export function relLabel(ms: number, now: number, lang: Lang): string {
  const m = Math.ceil((ms - now) / 6e4);
  if (m <= -1) return translate(lang, "races.rel.started");
  if (m <= 1) return translate(lang, "races.rel.now");
  if (m < 60) return translate(lang, "races.rel.min", { n: m });
  if (m < 48 * 60) {
    const h = Math.floor(m / 60);
    const r = m % 60;
    return translate(lang, "races.rel.hours", { t: `${h} h${r ? ` ${pad(r)}` : ""}` });
  }
  return translate(lang, "races.rel.days", { n: Math.round(m / 1440) });
}
