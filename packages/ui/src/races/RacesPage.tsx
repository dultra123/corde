import { type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { IconFlag, IconRefresh } from "../dashboard/icons";
import { useT } from "../i18n";
import { isDesktop, isLocalPage } from "../lib/desktop";
import {
  deletePick,
  fetchRaceSettings,
  fetchRaceSources,
  type RacePick,
  type RaceSettings,
  type RaceSettingsPatch,
  type RaceSources,
  type Ratings,
  type ReqKey,
  reportUnreadable,
  savePick,
  saveRaceSettings,
} from "../net/races-api";
import { ApiError } from "../net/sessions-api";
import { fetchSettings, saveSettings } from "../net/system-api";
import { errorMessage } from "../sessions/shared";
import { useRaces } from "../store/races";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { chipDay, dateRange, dayLabel, hm, relLabel } from "./format";
import { useNow, usePicks } from "./hooks";
import {
  buildSchedule,
  CAT_ORDER,
  countdown,
  LEAD_CHOICES,
  LEVELS,
  missing,
  pickOf,
  RATINGS,
  REQ_KEYS,
  reqKey,
  type SourceView,
  safeImage,
  shownRaces,
  upcomingPicks,
  utcLabel,
} from "./model";
import { type Cat, type Level, nextOf, type Race, viewRaces } from "./parse";
import { playChime, unlockAudio } from "./sound";

const SOURCES_EVERY_MS = 5 * 60_000;
const MAX_CHIPS = 8;
const SITE: Record<string, string> = {
  racecontrol: "racecontrol.gg",
  lmutools: "lmu.tools",
};

/** The settings may only be changed from the PC that runs Corde (the server says so too). */
const onThisPc = () => isDesktop() || isLocalPage();

/* ------------------------------------------------------------------ small pieces */

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
    </span>
  );
}

function ReqBadges({ req, profile }: { req: Ratings | undefined; profile: Ratings }) {
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

function KindBadge({ cat, level }: { cat: Cat; level: Level | null }) {
  const { t } = useT();
  if (cat === "daily" && level) {
    return <span className={`race-kind lvl-${level}`}>{t(`races.level.${level}`)}</span>;
  }
  return <span className={`race-kind cat-${cat}`}>{t(`races.cat.${cat}.one`)}</span>;
}

/** The race's picture from the page, or a drawn stand-in when there is none (or it fails). */
function RaceArt({ race }: { race: Race }) {
  const src = safeImage(race.image);
  const [failed, setFailed] = useState(false);
  const mark = race.track.replace(/\(.*?\)/g, "").trim();
  return (
    <div className={`race-art cat-${race.cat}${race.level ? ` lvl-${race.level}` : ""}`}>
      {src && !failed ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          decoding="async"
          onError={() => setFailed(true)}
        />
      ) : (
        <span className="race-art-mark" aria-hidden="true">
          {mark}
        </span>
      )}
    </div>
  );
}

/* ------------------------------------------------------------------ a race */

interface PickActions {
  picked: Set<string>;
  toggle: (race: Race, start: number) => void;
  busy: string | null;
}

function StartChip({
  race,
  start,
  now,
  actions,
}: {
  race: Race;
  start: number;
  now: number;
  actions: PickActions;
}) {
  const { t, lang } = useT();
  const id = `${race.id}@${start}`;
  const on = actions.picked.has(id);
  const day = chipDay(start, now, lang);
  return (
    <button
      type="button"
      className={`race-start${on ? " is-on" : ""}`}
      aria-pressed={on}
      disabled={actions.busy === id}
      title={on ? t("races.start.remove") : t("races.start.add")}
      onClick={() => actions.toggle(race, start)}
    >
      {day && <small>{day}</small>}
      <span className="num">{hm(start)}</span>
    </button>
  );
}

function RaceCard({
  race,
  settings,
  now,
  actions,
}: {
  race: Race;
  settings: RaceSettings;
  now: number;
  actions: PickActions;
}) {
  const { t, lang } = useT();
  const [all, setAll] = useState(false);
  const req = settings.req[reqKey(race)];
  const lack = missing(req, settings.profile);
  const special = race.cat === "special";
  const meta = [race.track, race.duration, ...(race.classes ?? [])].filter(Boolean);
  const picked = special
    ? actions.picked.has(`${race.id}@${race.remindAt}`)
    : race.starts.some((s) => actions.picked.has(`${race.id}@${s}`));
  const starts = all ? race.starts : race.starts.slice(0, MAX_CHIPS);
  const fmt = race.fmt;
  const format = fmt
    ? [
        fmt.p ? t("races.fmt.p", { n: fmt.p }) : "",
        fmt.q ? t("races.fmt.q", { n: fmt.q }) : "",
        fmt.r ? t("races.fmt.r", { n: fmt.r }) : "",
      ]
        .filter(Boolean)
        .join(" · ")
    : "";
  return (
    <article className={`race-card cat-${race.cat}${picked ? " is-picked" : ""}`}>
      <RaceArt race={race} />
      <div className="race-body">
        <div className="race-badges">
          <KindBadge cat={race.cat} level={race.level} />
          <ReqBadges req={req} profile={settings.profile} />
          {picked && (
            <span className="race-mine">
              {t(special ? "races.mine.reminder" : "races.mine.in")}
            </span>
          )}
        </div>
        {special && race.from && race.to && (
          <p className="race-dates num">{dateRange(race.from, race.to, lang)}</p>
        )}
        <h3 className="race-name">{race.name}</h3>
        <p className="race-meta dim">{meta.join(" · ")}</p>
        {(format || race.setup || race.cat === "champ") && (
          <p className="race-more faint">
            {[
              format,
              race.setup
                ? t(race.setup === "fixed" ? "races.setup.fixed" : "races.setup.open")
                : "",
              race.cat === "champ" ? t("races.champ.sub") : "",
            ]
              .filter(Boolean)
              .join(" · ")}
          </p>
        )}
        {lack.length > 0 && (
          <p className="race-lock">{t("races.req.need", { what: lack.join(" · ") })}</p>
        )}
        {special ? (
          <div className="race-starts">
            <button
              type="button"
              className={`race-start is-wide${picked ? " is-on" : ""}`}
              aria-pressed={picked}
              disabled={actions.busy === `${race.id}@${race.remindAt}`}
              onClick={() => actions.toggle(race, race.remindAt as number)}
            >
              {picked
                ? t("races.special.on", {
                    day: chipDay(race.remindAt as number, now, lang) || t("races.day.todayShort"),
                    time: hm(race.remindAt as number),
                  })
                : t("races.special.off")}
            </button>
          </div>
        ) : (
          <div className="race-starts">
            {starts.map((s) => (
              <StartChip key={s} race={race} start={s} now={now} actions={actions} />
            ))}
            {race.starts.length > MAX_CHIPS && (
              <button
                type="button"
                className="race-start is-more"
                onClick={() => setAll((v) => !v)}
              >
                {all ? t("races.start.less") : `+${race.starts.length - MAX_CHIPS}`}
              </button>
            )}
          </div>
        )}
      </div>
    </article>
  );
}

/* ------------------------------------------------------------------ the schedule */

function CatSection({
  cat,
  races,
  settings,
  now,
  actions,
}: {
  cat: Cat;
  races: Race[];
  settings: RaceSettings;
  now: number;
  actions: PickActions;
}) {
  const { t, nOf } = useT();
  const [level, setLevel] = useState<Level | "all">("all");
  const levels = LEVELS.filter(
    (l) => settings.levels.includes(l) && races.some((r) => r.level === l),
  );
  const list = cat === "daily" && level !== "all" ? races.filter((r) => r.level === level) : races;
  const catReq = cat === "daily" ? undefined : settings.req[cat as ReqKey];
  return (
    <section className={`races-cat cat-${cat}`} aria-labelledby={`races-cat-${cat}`}>
      <div className="races-cat-head">
        <div>
          <h2 id={`races-cat-${cat}`} className="races-cat-title">
            {t(`races.cat.${cat}.title`)}
            <span className="races-cat-count dim num">
              {nOf(
                list.length,
                cat === "special" ? "word.event" : "word.series",
                cat === "special" ? "word.events" : "word.seriess",
              )}
            </span>
          </h2>
          <p className="races-cat-desc dim">{t(`races.cat.${cat}.desc`)}</p>
        </div>
        {catReq && (catReq.sr || catReq.dr) && (
          <div className="race-badges">
            <ReqBadges req={catReq} profile={settings.profile} />
          </div>
        )}
      </div>
      {cat === "daily" && levels.length > 1 && (
        <fieldset className="races-levels" aria-label={t("races.levels")}>
          {(["all", ...levels] as const).map((l) => (
            <button
              key={l}
              type="button"
              className={`races-chip${level === l ? " is-on" : ""}${l !== "all" ? ` lvl-${l}` : ""}`}
              aria-pressed={level === l}
              onClick={() => setLevel(l)}
            >
              {l !== "all" && <i aria-hidden="true" />}
              {l === "all" ? t("races.levels.all") : t(`races.level.${l}`)}
              <small className="num">
                {l === "all" ? races.length : races.filter((r) => r.level === l).length}
              </small>
            </button>
          ))}
        </fieldset>
      )}
      {list.length === 0 ? (
        <p className="races-empty dim">
          {t(cat === "daily" ? "races.empty.daily" : "races.empty.cat")}
        </p>
      ) : (
        <div className="races-grid">
          {list.map((r) => (
            <RaceCard key={r.id} race={r} settings={settings} now={now} actions={actions} />
          ))}
        </div>
      )}
    </section>
  );
}

function SourceNotice({ s }: { s: SourceView }) {
  const { t } = useT();
  const site = SITE[s.id] ?? s.id;
  if (s.unreadable) {
    return (
      <div className="races-notice is-warn" role="status">
        <p className="races-notice-title">{t("races.unreadable.title", { site })}</p>
        <p>
          {t("races.unreadable.body", { file: `diagnostics/races/${s.id}.html` })}{" "}
          <a className="link" href={s.url} target="_blank" rel="noreferrer noopener">
            {t("races.openSite", { site })}
          </a>
        </p>
      </div>
    );
  }
  if (s.state === "error") {
    return (
      <div className="races-notice is-error" role="status">
        <p className="races-notice-title">{t("races.sourceError.title", { site })}</p>
        <p>
          {s.error}
          {s.fetchedAt ? ` — ${t("races.sourceError.stale", { time: hm(s.fetchedAt) })}` : ""}{" "}
          <a className="link" href={s.url} target="_blank" rel="noreferrer noopener">
            {t("races.openSite", { site })}
          </a>
        </p>
      </div>
    );
  }
  return null;
}

/* ------------------------------------------------------------------ my races */

function MyRaces({
  picks,
  now,
  settings,
  onRemove,
  busy,
}: {
  picks: RacePick[];
  now: number;
  settings: RaceSettings | null;
  onRemove: (p: RacePick) => void;
  busy: string | null;
}) {
  const { t, lang, nOf } = useT();
  const list = upcomingPicks(picks, now);
  const next = list.find((p) => p.cat !== "special") ?? list[0] ?? null;
  const leads = settings?.leads ?? [];
  const before = leads.filter((l) => l > 0);
  let leadText = t("races.leads.none");
  if (before.length > 0) {
    leadText = t("races.leads.text", { list: before.join(", ") });
    if (leads.includes(0)) leadText += ` ${t("races.leads.andStart")}`;
  } else if (leads.includes(0)) leadText = t("races.leads.onlyStart");
  const groups: { day: string; items: RacePick[] }[] = [];
  for (const p of list) {
    const day = dayLabel(p.start_ms, now, lang);
    const g = groups.at(-1);
    if (g && g.day === day) g.items.push(p);
    else groups.push({ day, items: [p] });
  }
  return (
    <section className="card races-card races-mine" aria-labelledby="races-mine-title">
      <div className="card-head">
        <h2 id="races-mine-title">{t("races.mine.title")}</h2>
        {list.length > 0 && (
          <span className="dim num">{nOf(list.length, "word.race", "word.races")}</span>
        )}
      </div>
      {!next ? (
        <p className="dim races-mine-empty">{t("races.mine.empty")}</p>
      ) : (
        <>
          <NextUp pick={next} now={now} />
          {groups.map((g) => (
            <div key={g.day} className="races-mine-day">
              <p className="races-mine-dayname faint">{g.day}</p>
              <ul className="races-mine-list">
                {g.items.map((p) => (
                  <li key={p.id} className={`races-mine-row cat-${p.cat}`}>
                    <span className="races-mine-time num">{hm(p.start_ms)}</span>
                    <span className="races-mine-what">
                      <b>{p.name}</b>
                      <span className="dim">
                        {p.track} ·{" "}
                        {p.cat === "daily" && p.level
                          ? t(`races.level.${p.level}`)
                          : t(`races.cat.${p.cat}.one`)}
                      </span>
                    </span>
                    <span className="races-mine-side">
                      <span className="dim num">{relLabel(p.start_ms, now, lang)}</span>
                      <button
                        type="button"
                        className="link"
                        disabled={busy === p.id}
                        onClick={() => onRemove(p)}
                      >
                        {t("races.mine.remove")}
                      </button>
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          ))}
          <p className="races-mine-foot faint">
            {leadText}
            {list.some((p) => p.cat === "special") && <> · {t("races.mine.specialRule")}</>}
          </p>
        </>
      )}
    </section>
  );
}

/** The next race in big, counting down to the second. */
function NextUp({ pick, now: slowNow }: { pick: RacePick; now: number }) {
  const { t, lang } = useT();
  const fast = useNow(1000);
  const now = Math.max(fast, slowNow);
  const left = pick.start_ms - now;
  const urgent = left <= 5 * 60_000;
  const special = pick.cat === "special";
  return (
    <div className={`races-next${urgent ? " is-urgent" : ""}${left <= 0 ? " is-go" : ""}`}>
      <p className="races-next-kicker">{t(special ? "races.next.special" : "races.next.kicker")}</p>
      <p className="races-next-clock num" aria-live="off">
        {special ? relLabel(pick.start_ms, now, lang) : countdown(left)}
      </p>
      <p className="races-next-name">{pick.name}</p>
      <p className="races-next-sub dim">
        {pick.track} · {dayLabel(pick.start_ms, now, lang)},{" "}
        {t(special ? "races.next.reminderAt" : "races.next.startAt")} {hm(pick.start_ms)}
      </p>
      <p className={`races-next-hint${urgent ? " is-urgent" : ""}`}>
        {left <= 0 ? t("races.next.go") : urgent ? t("races.next.soon") : t("races.next.join")}
      </p>
    </div>
  );
}

/* ------------------------------------------------------------------ settings */

function RatingSelect({
  label,
  value,
  onChange,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
}) {
  const { t } = useT();
  return (
    <label className="races-select">
      <span>{label}</span>
      <select className="select" value={value} onChange={(e) => onChange(e.target.value)}>
        {RATINGS.map((r) => (
          <option key={r || "none"} value={r}>
            {r || t("races.req.none")}
          </option>
        ))}
      </select>
    </label>
  );
}

function ProfileCard({
  settings,
  save,
}: {
  settings: RaceSettings;
  save: (p: RaceSettingsPatch) => void;
}) {
  const { t } = useT();
  return (
    <section className="card races-card" aria-labelledby="races-profile-title">
      <div className="card-head">
        <h2 id="races-profile-title">{t("races.profile.title")}</h2>
      </div>
      <div className="races-pair">
        <RatingSelect
          label="DR"
          value={settings.profile.dr}
          onChange={(dr) => save({ profile: { ...settings.profile, dr } })}
        />
        <RatingSelect
          label="SR"
          value={settings.profile.sr}
          onChange={(sr) => save({ profile: { ...settings.profile, sr } })}
        />
      </div>
      <p className="races-hint faint">{t("races.profile.hint")}</p>
    </section>
  );
}

function RemindersCard({
  settings,
  save,
}: {
  settings: RaceSettings;
  save: (p: RaceSettingsPatch) => void;
}) {
  const { t } = useT();
  const sound = useSettings((s) => s.raceSound);
  const update = useSettings((s) => s.update);
  const [test, setTest] = useState<"idle" | "ok" | "blocked">("idle");
  const toggle = (l: number) => {
    const next = settings.leads.includes(l)
      ? settings.leads.filter((x) => x !== l)
      : [...settings.leads, l];
    save({ leads: next.sort((a, b) => b - a) });
  };
  const ring = async () => {
    const ok = await unlockAudio();
    playChime(true);
    setTest(ok ? "ok" : "blocked");
  };
  return (
    <section className="card races-card" aria-labelledby="races-reminders-title">
      <div className="card-head">
        <h2 id="races-reminders-title">{t("races.reminders.title")}</h2>
      </div>
      <fieldset className="races-leads" aria-label={t("races.reminders.title")}>
        {LEAD_CHOICES.map((l) => (
          <button
            key={l}
            type="button"
            className={`races-chip${settings.leads.includes(l) ? " is-on" : ""}`}
            aria-pressed={settings.leads.includes(l)}
            onClick={() => toggle(l)}
          >
            {l === 0 ? t("races.reminders.atStart") : t("races.reminders.before", { n: l })}
          </button>
        ))}
      </fieldset>
      <p className="races-hint faint">{t("races.reminders.hint")}</p>
      <div className="races-sound">
        <label className="switch">
          <input
            type="checkbox"
            checked={sound}
            onChange={(e) => update({ raceSound: e.target.checked })}
          />
          {t("races.sound.label")}
        </label>
        <button type="button" className="home-btn races-test" onClick={() => void ring()}>
          {t("races.sound.test")}
        </button>
      </div>
      <p className="races-hint faint" role="status">
        {test === "ok"
          ? t("races.sound.ok")
          : test === "blocked"
            ? t("races.sound.blocked")
            : t("races.sound.hint")}
      </p>
    </section>
  );
}

function DisplayCard({
  settings,
  save,
  method,
  offset,
  onDisable,
  busy,
}: {
  settings: RaceSettings;
  save: (p: RaceSettingsPatch) => void;
  method: string | null | undefined;
  offset: number | null | undefined;
  onDisable: () => void;
  busy: boolean;
}) {
  const { t } = useT();
  const toggleIn = (list: string[], v: string) =>
    list.includes(v) ? list.filter((x) => x !== v) : [...list, v];
  const how =
    method === "horodatage"
      ? t("races.hour.read")
      : offset != null
        ? t("races.hour.guessed", { zone: utcLabel(offset) })
        : t("races.hour.auto");
  return (
    <section className="card races-card" aria-labelledby="races-display-title">
      <details className="races-details">
        <summary>
          <h2 id="races-display-title">{t("races.display.title")}</h2>
        </summary>
        <p className="races-sub">{t("races.display.cats")}</p>
        <div className="races-leads">
          {CAT_ORDER.map((c) => {
            const on = settings.cats.includes(c);
            return (
              <button
                key={c}
                type="button"
                className={`races-chip${on ? " is-on" : ""}`}
                aria-pressed={on}
                disabled={on && settings.cats.length === 1}
                onClick={() => save({ cats: toggleIn(settings.cats, c) })}
              >
                {t(`races.cat.${c}.short`)}
              </button>
            );
          })}
        </div>
        <p className="races-sub">{t("races.display.levels")}</p>
        <div className="races-leads">
          {LEVELS.map((l) => {
            const on = settings.levels.includes(l);
            return (
              <button
                key={l}
                type="button"
                className={`races-chip lvl-${l}${on ? " is-on" : ""}`}
                aria-pressed={on}
                disabled={on && settings.levels.length === 1}
                onClick={() => save({ levels: toggleIn(settings.levels, l) })}
              >
                <i aria-hidden="true" />
                {t(`races.level.${l}`)}
              </button>
            );
          })}
        </div>
        <p className="races-sub">{t("races.hour.title")}</p>
        <fieldset className="races-seg" aria-label={t("races.hour.title")}>
          {[-2, -1, 0, 1, 2].map((v) => (
            <button
              key={v}
              type="button"
              className={settings.hour_fix === v ? "is-on" : ""}
              aria-pressed={settings.hour_fix === v}
              onClick={() => save({ hour_fix: v })}
            >
              {v === 0 ? t("races.hour.autoShort") : `${v > 0 ? "+" : "−"}${Math.abs(v)} h`}
            </button>
          ))}
        </fieldset>
        <p className="races-hint faint">{t("races.hour.hint", { how })}</p>
        <p className="races-sub">{t("races.req.title")}</p>
        <div className="races-reqs">
          {REQ_KEYS.map((k) => (
            <div key={k} className="races-req-row">
              <span>{t(`races.reqKey.${k}`)}</span>
              <div className="races-pair">
                {(["sr", "dr"] as const).map((f) => (
                  <RatingSelect
                    key={f}
                    label={f.toUpperCase()}
                    value={settings.req[k]?.[f] ?? ""}
                    onChange={(v) => save({ req: { [k]: { [f]: v } } })}
                  />
                ))}
              </div>
            </div>
          ))}
        </div>
        <p className="races-hint faint">{t("races.req.hint")}</p>
        {onThisPc() && (
          <p className="races-off">
            <button type="button" className="link" onClick={onDisable} disabled={busy}>
              {t("races.disable")}
            </button>
          </p>
        )}
      </details>
    </section>
  );
}

/* ------------------------------------------------------------------ off */

function OffCard({
  onEnable,
  busy,
  error,
}: {
  onEnable: () => void;
  busy: boolean;
  error: string | null;
}) {
  const { t } = useT();
  return (
    <section className="card races-off-card">
      <IconFlag />
      <div>
        <h2 className="races-off-title">{t("races.off.title")}</h2>
        <p>{t("races.off.body")}</p>
        <p className="dim">{t("races.off.privacy")}</p>
        {onThisPc() ? (
          <button type="button" className="home-btn is-primary" onClick={onEnable} disabled={busy}>
            {t("races.off.enable")}
          </button>
        ) : (
          <p className="faint">{t("races.off.remote")}</p>
        )}
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </div>
    </section>
  );
}

/* ------------------------------------------------------------------ page */

/**
 * `#/races` — the online races of Le Mans Ultimate (daily, weekly, special events, RaceControl
 * Pro championships), read from racecontrol.gg and lmu.tools by the server on demand; the starts
 * the driver picks (« Mes courses », shared with the phone) and their reminders.
 */
export function RacesPage() {
  const { t } = useT();
  const online = useTelemetry((s) => s.socket === "open");
  const now = useNow(20_000);
  const [enabled, setEnabled] = useState<boolean | null>(null);
  const [data, setData] = useState<RaceSources | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [settings, setSettings] = useState<RaceSettings | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const { picks } = usePicks();
  const reported = useRef(new Set<string>());

  const load = async (force: boolean) => {
    setLoading(true);
    try {
      const d = await fetchRaceSources(force);
      setData(d);
      setEnabled(d.enabled);
      setError(null);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setLoading(false);
    }
  };

  // biome-ignore lint/correctness/useExhaustiveDependencies: load once online, then on a timer
  useEffect(() => {
    if (!online) return;
    void fetchSettings()
      .then((s) => setEnabled(s.online_schedule === true))
      .catch(() => {});
    void fetchRaceSettings()
      .then(setSettings)
      .catch((e: unknown) => setError(errorMessage(e)));
    void load(false);
    const id = setInterval(() => {
      if (document.visibilityState !== "hidden") void load(false);
    }, SOURCES_EVERY_MS);
    return () => clearInterval(id);
  }, [online]);

  const view = useMemo(() => (data ? buildSchedule(data) : null), [data]);

  // Pages from which nothing could be read: the server keeps its copy for diagnosis (once each).
  useEffect(() => {
    for (const s of view?.sources ?? []) {
      const key = `${s.id}@${s.fetchedAt}`;
      if (s.unreadable && !reported.current.has(key)) {
        reported.current.add(key);
        void reportUnreadable(s.id).catch(() => {});
      }
    }
  }, [view]);

  const races = useMemo(() => {
    if (!view?.schedule || !settings) return [];
    return shownRaces(viewRaces(view.schedule.races, settings.hour_fix, now), settings);
  }, [view, settings, now]);

  const picked = useMemo(() => new Set((picks ?? []).map((p) => p.id)), [picks]);

  const saveSettingsPatch = (patch: RaceSettingsPatch) => {
    // Optimistic: the page follows at once, the server answer replaces it.
    setSettings((s) => (s ? mergeSettings(s, patch) : s));
    saveRaceSettings(patch)
      .then((s) => {
        setSettings(s);
        if (patch.hour_fix !== undefined) useRaces.getState().bumpPicks();
      })
      .catch((e: unknown) => {
        setActionError(errorMessage(e));
        void fetchRaceSettings().then(setSettings);
      });
  };

  const toggle = async (race: Race, start: number) => {
    const pick = pickOf(race, start);
    const current = useRaces.getState().picks ?? [];
    const on = current.some((p) => p.id === pick.id);
    setBusy(pick.id);
    setActionError(null);
    useRaces.getState().setPicks(on ? current.filter((p) => p.id !== pick.id) : [...current, pick]);
    try {
      if (on) await deletePick(pick.id);
      else await savePick(pick);
    } catch (e) {
      setActionError(
        e instanceof ApiError && e.status === 409 ? t("races.error.full") : errorMessage(e),
      );
    } finally {
      setBusy(null);
      useRaces.getState().bumpPicks();
    }
  };

  const remove = async (p: RacePick) => {
    setBusy(p.id);
    useRaces.getState().setPicks((useRaces.getState().picks ?? []).filter((x) => x.id !== p.id));
    try {
      await deletePick(p.id);
    } catch (e) {
      if (!(e instanceof ApiError && e.status === 404)) setActionError(errorMessage(e));
    } finally {
      setBusy(null);
      useRaces.getState().bumpPicks();
    }
  };

  const setOnline = async (on: boolean) => {
    setBusy("setting");
    setActionError(null);
    try {
      await saveSettings({ online_schedule: on });
      setEnabled(on);
      await load(false);
    } catch (e) {
      setActionError(
        e instanceof ApiError && e.status === 403 ? t("races.off.remote") : errorMessage(e),
      );
    } finally {
      setBusy(null);
    }
  };

  const actions: PickActions = { picked, toggle: (r, s) => void toggle(r, s), busy };
  const fetched = view?.sources
    .map((s) => (s.fetchedAt ? `${SITE[s.id]} ${hm(s.fetchedAt)}` : null))
    .filter(Boolean)
    .join(" · ");
  const byCat = (c: Cat) => races.filter((r) => r.cat === c).sort((a, b) => nextOf(a) - nextOf(b));
  const nothing = view && !view.schedule && enabled;

  let body: ReactNode;
  if (!online) {
    body = <p className="dim">{t("common.loading")}</p>;
  } else if (enabled === false) {
    body = (
      <div className="races-layout is-off">
        <OffCard
          onEnable={() => void setOnline(true)}
          busy={busy === "setting"}
          error={actionError}
        />
        {picks && picks.length > 0 && (
          <MyRaces
            picks={picks}
            now={now}
            settings={settings}
            onRemove={(p) => void remove(p)}
            busy={busy}
          />
        )}
      </div>
    );
  } else {
    body = (
      <div className="races-layout">
        <div className="races-main">
          {view?.sources.map((s) => (
            <SourceNotice key={s.id} s={s} />
          ))}
          {error && (
            <p className="form-error" role="alert">
              {t("common.serverError", { msg: error })}
            </p>
          )}
          {actionError && (
            <p className="form-error" role="alert">
              {actionError}
            </p>
          )}
          {!view || !settings ? (
            <div className="notice">
              <div className="spinner" />
              <p className="notice-body">{t("races.loading")}</p>
            </div>
          ) : nothing ? (
            <div className="notice is-error">
              <p className="notice-title">{t("races.none.title")}</p>
              <p className="notice-body">{t("races.none.body")}</p>
            </div>
          ) : (
            CAT_ORDER.filter((c) => settings.cats.includes(c)).map((c) => (
              <CatSection
                key={c}
                cat={c}
                races={byCat(c)}
                settings={settings}
                now={now}
                actions={actions}
              />
            ))
          )}
        </div>
        <div className="races-side-mine">
          <MyRaces
            picks={picks ?? []}
            now={now}
            settings={settings}
            onRemove={(p) => void remove(p)}
            busy={busy}
          />
        </div>
        {settings && (
          <div className="races-side-settings">
            <ProfileCard settings={settings} save={saveSettingsPatch} />
            <RemindersCard settings={settings} save={saveSettingsPatch} />
            <DisplayCard
              settings={settings}
              save={saveSettingsPatch}
              method={view?.schedule?.method}
              offset={view?.schedule?.offset}
              onDisable={() => void setOnline(false)}
              busy={busy === "setting"}
            />
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="races-page">
      <header className="page-head">
        <div className="page-head-row">
          <div className="page-head-main">
            <h1 className="page-title">{t("races.title")}</h1>
            <p className="page-subtitle dim">
              {enabled && fetched
                ? t("races.subtitle.read", { when: fetched })
                : t("races.subtitle")}
            </p>
          </div>
          {enabled && (
            <div className="page-actions">
              <button
                type="button"
                className="btn is-cta"
                onClick={() => void load(true)}
                disabled={loading}
                title={t("races.refreshHint")}
              >
                <IconRefresh />
                {t("races.refresh")}
              </button>
            </div>
          )}
        </div>
      </header>
      {body}
      <footer className="page-foot races-foot dim">
        <span>{t("races.foot.join")}</span>
        <span>
          {t("races.foot.sources")}{" "}
          <a
            className="link"
            href="https://racecontrol.gg/"
            target="_blank"
            rel="noreferrer noopener"
          >
            racecontrol.gg
          </a>{" "}
          ·{" "}
          <a
            className="link"
            href="https://lmu.tools/schedule"
            target="_blank"
            rel="noreferrer noopener"
          >
            lmu.tools
          </a>
        </span>
        <span className="faint">{t("races.foot.unofficial")}</span>
      </footer>
    </div>
  );
}

/** The settings with a patch applied (the server's answer replaces them). */
export function mergeSettings(s: RaceSettings, patch: RaceSettingsPatch): RaceSettings {
  const req = { ...s.req };
  for (const [k, v] of Object.entries(patch.req ?? {}) as [ReqKey, Partial<Ratings>][]) {
    req[k] = { ...req[k], ...v };
  }
  const { req: _r, profile: _p, ...rest } = patch;
  return { ...s, ...rest, profile: { ...s.profile, ...(patch.profile ?? {}) }, req };
}
