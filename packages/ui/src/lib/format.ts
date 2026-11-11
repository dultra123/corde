/** Number and time formatting shared by every widget. Metric units only. */

const pad2 = (n: number) => String(n).padStart(2, "0");
const pad3 = (n: number) => String(n).padStart(3, "0");

/** `1:43.217` — lap or sector time in seconds. `—` when absent. */
export function fmtLapTime(
  s: number | null | undefined,
  opts?: { ms?: number; minutes?: boolean },
): string {
  if (s === null || s === undefined || !Number.isFinite(s) || s < 0) return "—";
  const msDigits = opts?.ms ?? 3;
  const total = Math.round(s * 10 ** msDigits);
  const unit = 10 ** msDigits;
  const whole = Math.floor(total / unit);
  const frac = total % unit;
  const m = Math.floor(whole / 60);
  const sec = whole % 60;
  const fracStr = String(frac).padStart(msDigits, "0");
  return m > 0 || opts?.minutes ? `${m}:${pad2(sec)}.${fracStr}` : `${sec}.${fracStr}`;
}

/** `+0.312` / `-1.040`. Always signed; `—` when absent. */
export function fmtDelta(s: number | null | undefined, digits = 3): string {
  if (s === null || s === undefined || !Number.isFinite(s)) return "—";
  const sign = s > 0 ? "+" : s < 0 ? "−" : "±";
  return `${sign}${Math.abs(s).toFixed(digits)}`;
}

/** Gap to another car: `+1.2` / `+1 tour` style is handled by the caller; this is seconds only. */
export function fmtGap(s: number | null | undefined, digits = 1): string {
  if (s === null || s === undefined || !Number.isFinite(s)) return "—";
  return Math.abs(s).toFixed(digits);
}

/** `1:23:45` or `23:45` — a session clock. */
export function fmtClock(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds)) return "—";
  const total = Math.max(0, Math.floor(seconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  return h > 0 ? `${h}:${pad2(m)}:${pad2(s)}` : `${pad2(m)}:${pad2(s)}`;
}

/** `16:42` — in-game time of day from seconds since midnight. */
export function fmtTimeOfDay(seconds: number): string {
  const total = ((Math.floor(seconds) % 86400) + 86400) % 86400;
  return `${pad2(Math.floor(total / 3600))}:${pad2(Math.floor((total % 3600) / 60))}`;
}

/** Fixed decimals, `—` when not a finite number. */
export function fmtNum(v: number | null | undefined, digits = 0): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "—";
  return v.toFixed(digits);
}

/** Percent from a 0..1 fraction. */
export function fmtPct(fraction: number | null | undefined, digits = 0): string {
  if (fraction === null || fraction === undefined || !Number.isFinite(fraction)) return "—";
  return `${(fraction * 100).toFixed(digits)} %`;
}

/** Gear label: R, N, 1.. */
export function fmtGear(gear: number): string {
  if (gear < 0) return "R";
  if (gear === 0) return "N";
  return String(gear);
}

/** Ordinal suffix for a position, locale aware (1er / 2e, 1st / 2nd). */
export function fmtOrdinal(n: number, lang: "fr" | "en"): string {
  if (lang === "fr") return n === 1 ? "1er" : `${n}e`;
  const rem10 = n % 10;
  const rem100 = n % 100;
  if (rem10 === 1 && rem100 !== 11) return `${n}st`;
  if (rem10 === 2 && rem100 !== 12) return `${n}nd`;
  if (rem10 === 3 && rem100 !== 13) return `${n}rd`;
  return `${n}th`;
}

/** `1,2 Mo` / `1.2 MB` — binary-ish sizes with locale decimal separator. */
export function fmtBytes(bytes: number | null | undefined, lang: "fr" | "en" = "fr"): string {
  if (bytes === null || bytes === undefined || !Number.isFinite(bytes)) return "—";
  const units = lang === "fr" ? ["o", "ko", "Mo", "Go", "To"] : ["B", "kB", "MB", "GB", "TB"];
  let v = Math.max(0, bytes);
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i += 1;
  }
  const digits = i === 0 ? 0 : v < 10 ? 1 : 0;
  const n = v.toLocaleString(lang === "fr" ? "fr-FR" : "en-GB", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });
  return `${n} ${units[i]}`;
}

/** `mar. 4 nov. 2026, 07:39` — a wall-clock timestamp in the user's language. */
export function fmtDateTime(ms: number | null | undefined, lang: "fr" | "en" = "fr"): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "—";
  return new Intl.DateTimeFormat(lang === "fr" ? "fr-FR" : "en-GB", {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(new Date(ms));
}

/** `4 nov. 2026` — date only. */
export function fmtDate(ms: number | null | undefined, lang: "fr" | "en" = "fr"): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "—";
  return new Intl.DateTimeFormat(lang === "fr" ? "fr-FR" : "en-GB", {
    day: "numeric",
    month: "short",
    year: "numeric",
  }).format(new Date(ms));
}

const DAY_MS = 86_400_000;

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/**
 * `aujourd'hui 21:04` / `hier 21:04` / `mardi 21:04` / `4 nov. 21:04` / `4 nov. 2025` — how a
 * person would say when something recent happened. Calendar days in the local time zone; the
 * weekday is used for the last six days, the year only when it is not the current one.
 * `time: false` drops the clock (`hier`, `mardi`, `4 nov.`).
 */
export function fmtRelativeDay(
  ms: number | null | undefined,
  lang: "fr" | "en" = "fr",
  now: number = Date.now(),
  opts?: { time?: boolean },
): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "—";
  const locale = lang === "fr" ? "fr-FR" : "en-GB";
  const date = new Date(ms);
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / DAY_MS);
  const sameYear = date.getFullYear() === new Date(now).getFullYear();
  if (days < 0 || days > 6) {
    if (!sameYear) return fmtDate(ms, lang);
    const day = new Intl.DateTimeFormat(locale, { day: "numeric", month: "short" }).format(date);
    if (opts?.time === false) return day;
    return `${day} ${clockOf(date, locale)}`;
  }
  const day =
    days === 0
      ? lang === "fr"
        ? "aujourd'hui"
        : "today"
      : days === 1
        ? lang === "fr"
          ? "hier"
          : "yesterday"
        : new Intl.DateTimeFormat(locale, { weekday: "long" }).format(date);
  return opts?.time === false ? day : `${day} ${clockOf(date, locale)}`;
}

function clockOf(date: Date, locale: string): string {
  return new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(date);
}

/** `1 h 02 min` / `12 min` / `45 s` — a human duration from seconds. */
export function fmtDuration(seconds: number | null | undefined): string {
  if (seconds === null || seconds === undefined || !Number.isFinite(seconds) || seconds < 0)
    return "—";
  const total = Math.round(seconds);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h} h ${pad2(m)} min`;
  if (m > 0) return `${m} min${s > 0 && m < 10 ? ` ${pad2(s)} s` : ""}`;
  return `${s} s`;
}

export { pad2, pad3 };
