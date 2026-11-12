import {
  type FocusEvent,
  type KeyboardEvent,
  type ReactNode,
  type TouchEvent,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from "react";
import { useT } from "../i18n";
import { routeHash } from "../nav/routes";
import type { RaceSettings } from "../net/races-api";
import { chipDay, dateRange, hm } from "../races/format";
import {
  isFull,
  removePick,
  togglePick,
  useNow,
  usePicks,
  useRaceSettings,
  useRaceSources,
} from "../races/hooks";
import { buildSchedule, reqKey } from "../races/model";
import { KindBadge, ReqBadges } from "../races/parts";
import {
  autoplays,
  buildSlides,
  countdownLabel,
  followIndex,
  isSoon,
  SLIDE_MS,
  type Slide,
  slideArt,
  swipeStep,
  trackMark,
  wrap,
} from "../races/slides";
import { errorMessage } from "../sessions/shared";

const SITE: Record<string, string> = {
  racecontrol: "racecontrol.gg",
  lmutools: "lmu.tools",
};

/** `matchMedia(query).matches`, following changes. */
function useMedia(query: string): boolean {
  const [on, setOn] = useState(
    () => typeof window !== "undefined" && !!window.matchMedia?.(query).matches,
  );
  useEffect(() => {
    const mq = window.matchMedia?.(query);
    if (!mq) return;
    const update = () => setOn(mq.matches);
    update();
    mq.addEventListener("change", update);
    return () => mq.removeEventListener("change", update);
  }, [query]);
  return on;
}

/** The tab is hidden (another tab, minimised window). */
function useHidden(): boolean {
  const [hidden, setHidden] = useState(() => document.visibilityState === "hidden");
  useEffect(() => {
    const update = () => setHidden(document.visibilityState === "hidden");
    document.addEventListener("visibilitychange", update);
    return () => document.removeEventListener("visibilitychange", update);
  }, []);
  return hidden;
}

/** The race's picture, or the stand-in drawn in the colours of its category. */
function SlideArt({ slide }: { slide: Slide }) {
  const [failed, setFailed] = useState(false);
  const src = slideArt(slide.image, failed);
  return (
    <div className="live-art" aria-hidden="true">
      {src ? (
        <img
          src={src}
          alt=""
          loading="lazy"
          referrerPolicy="no-referrer"
          decoding="async"
          onError={() => setFailed(true)}
        />
      ) : (
        <span className="live-art-mark">{trackMark(slide.track)}</span>
      )}
    </div>
  );
}

function SlideView({
  slide,
  index,
  total,
  current,
  now,
  settings,
  busy,
  onAction,
}: {
  slide: Slide;
  index: number;
  total: number;
  current: boolean;
  now: number;
  settings: RaceSettings | null;
  busy: boolean;
  onAction: (s: Slide) => void;
}) {
  const { t, lang } = useT();
  const special = slide.cat === "special";
  const req = settings?.req[reqKey(slide)];
  const day = chipDay(slide.start, now, lang);
  const meta = [slide.track, slide.duration].filter(Boolean).join(" · ");
  const canAct = slide.pick !== null || slide.race !== null;
  return (
    // biome-ignore lint/a11y/useSemanticElements: the slide of an APG carousel is a named group, not a form fieldset
    <div
      role="group"
      aria-roledescription={t("home.live.slideRole")}
      aria-label={t("home.live.slideOf", { n: index + 1, total })}
      aria-hidden={!current}
      inert={!current}
      className={`live-slide cat-${slide.cat}${slide.level ? ` lvl-${slide.level}` : ""}${current ? " is-current" : ""}${slide.picked ? " is-picked" : ""}`}
    >
      <SlideArt slide={slide} />
      <div className="live-veil" aria-hidden="true" />
      <div className="live-body">
        <div className="live-badges race-badges">
          <KindBadge cat={slide.cat} level={slide.level} />
          {settings && <ReqBadges req={req} profile={settings.profile} />}
          {slide.picked && <span className="live-mine">{t("home.live.picked")}</span>}
        </div>
        <h3 className="live-name">
          <a
            className="live-link"
            href={routeHash({ page: "races" })}
            title={t("home.live.open", { name: slide.name })}
          >
            {slide.name}
          </a>
        </h3>
        {meta && <p className="live-meta">{meta}</p>}
        <div className="live-foot">
          <p className="live-when">
            <span className="live-start num">
              {special && slide.end !== null
                ? dateRange(slide.start, slide.end, lang)
                : t("home.live.startAt", { time: `${day ? `${day} ` : ""}${hm(slide.start)}` })}
            </span>
            <span className={`live-count num${isSoon(slide, now) ? " is-soon" : ""}`}>
              {countdownLabel(slide.start, slide.end, now, lang)}
            </span>
          </p>
          {canAct && (
            <button
              type="button"
              className={`home-btn live-act${slide.picked ? "" : " is-primary"}`}
              aria-pressed={slide.picked}
              disabled={busy}
              onClick={() => onAction(slide)}
            >
              {slide.picked
                ? t("home.live.remove")
                : special
                  ? t("races.special.off")
                  : t("home.live.pick")}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function CardShell({ children, aside }: { children: ReactNode; aside?: ReactNode }) {
  const { t } = useT();
  return (
    <section className="card home-card home-live">
      <div className="home-card-head">
        <h2 className="home-card-title">{t("home.live.title")}</h2>
        {aside}
      </div>
      {children}
    </section>
  );
}

/**
 * « Courses en ligne » on the home page, under the last session: the driver's races, then the
 * next starts of the schedule, one per slide, turning every 8 s (see `races/slides.ts`). Same
 * data and cache as the Courses page; a click opens it, the button picks or removes the start.
 */
export function RaceCarousel() {
  const { t } = useT();
  const titleId = useId();
  const { data, error } = useRaceSources();
  const { settings } = useRaceSettings();
  const { picks } = usePicks();
  const now = useNow(1000);
  const view = useMemo(() => (data ? buildSchedule(data) : null), [data]);
  const enabled = data?.enabled ?? null;

  const slides = useMemo(
    () =>
      buildSlides({
        races: enabled ? (view?.schedule?.races ?? []) : [],
        picks: picks ?? [],
        settings,
        now,
      }),
    [enabled, view, picks, settings, now],
  );
  const keys = slides.map((s) => s.key);

  // The current slide is followed by its race: a slide that leaves does not move the others.
  const [currentKey, setCurrentKey] = useState<string | null>(null);
  const lastIndex = useRef(0);
  const index = followIndex(currentKey, lastIndex.current, keys);
  lastIndex.current = index;

  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const hidden = useHidden();
  const reducedMotion = useMedia("(prefers-reduced-motion: reduce)");
  const auto = autoplays({ count: slides.length, hovered, focused, hidden, reducedMotion });
  const [announce, setAnnounce] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const touch = useRef<{ x: number; y: number } | null>(null);

  const goTo = (i: number, manual: boolean) => {
    const n = slides.length;
    if (!n) return;
    const j = wrap(i, n);
    const s = slides[j];
    if (!s) return;
    setCurrentKey(s.key);
    lastIndex.current = j;
    if (manual) {
      setAnnounce(t("home.live.announce", { n: j + 1, total: n, name: s.name }));
    }
  };

  // biome-ignore lint/correctness/useExhaustiveDependencies: restart the wait on every change of slide
  useEffect(() => {
    if (!auto) return;
    const id = setTimeout(() => goTo(index + 1, false), SLIDE_MS);
    return () => clearTimeout(id);
  }, [auto, index, slides.length]);

  const act = async (s: Slide) => {
    setBusy(s.pickId);
    setActionError(null);
    try {
      if (s.pick) await removePick(s.pick);
      // a special event is picked on its reminder of the day (`pickOf`)
      else if (s.race) await togglePick(s.race, s.start);
    } catch (e) {
      setActionError(isFull(e) ? t("races.error.full") : errorMessage(e));
    } finally {
      setBusy(null);
    }
  };

  const racesLink = (key: "home.live.all" | "home.live.seeRaces" | "home.live.enable") => (
    <a className="link" href={routeHash({ page: "races" })}>
      {t(key)}
    </a>
  );

  const unread = (view?.sources ?? []).filter((s) => s.unreadable || s.state === "error");
  const readable = !!view?.schedule;

  if (slides.length === 0) {
    let body: ReactNode;
    if (enabled === false) {
      body = (
        <p className="home-card-empty dim">
          {t("home.live.off")} {racesLink("home.live.enable")}
        </p>
      );
    } else if (!data || !settings || picks === null) {
      body =
        error && !data ? (
          <p className="home-card-empty form-error" role="alert">
            {t("common.serverError", { msg: error })} {racesLink("home.live.seeRaces")}
          </p>
        ) : (
          <p className="home-card-empty dim">{t("races.loading")}</p>
        );
    } else if (!readable && unread.length > 0) {
      body = (
        <p className="home-card-empty dim">
          {t("home.live.unreadable")} {racesLink("home.live.seeRaces")}
        </p>
      );
    } else {
      body = (
        <p className="home-card-empty dim">
          {t("home.live.empty")} {racesLink("home.live.seeRaces")}
        </p>
      );
    }
    return <CardShell aside={racesLink("home.live.all")}>{body}</CardShell>;
  }

  const onKeyDown = (e: KeyboardEvent<HTMLElement>) => {
    if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
      e.preventDefault();
      goTo(index + (e.key === "ArrowRight" ? 1 : -1), true);
    }
  };
  const onBlur = (e: FocusEvent<HTMLElement>) => {
    if (!e.currentTarget.contains(e.relatedTarget as Node | null)) setFocused(false);
  };
  const onTouchStart = (e: TouchEvent<HTMLElement>) => {
    const p = e.touches[0];
    touch.current = p ? { x: p.clientX, y: p.clientY } : null;
  };
  const onTouchEnd = (e: TouchEvent<HTMLElement>) => {
    const start = touch.current;
    const p = e.changedTouches[0];
    touch.current = null;
    if (!start || !p) return;
    const step = swipeStep(p.clientX - start.x, p.clientY - start.y);
    if (step) goTo(index + step, true);
  };

  return (
    <section
      className={`card home-card home-live${reducedMotion ? " is-still" : ""}`}
      aria-roledescription={t("home.live.roledesc")}
      aria-labelledby={titleId}
      onPointerEnter={(e) => e.pointerType === "mouse" && setHovered(true)}
      onPointerLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={onBlur}
      onKeyDown={onKeyDown}
      data-autoplay={auto ? "on" : "off"}
    >
      <div className="home-card-head">
        <h2 className="home-card-title" id={titleId}>
          <span>{t("home.live.title")}</span>
          <span className="sr-only">, {t("home.live.region")}</span>
        </h2>
        {racesLink("home.live.all")}
      </div>
      <div className="live-viewport" onTouchStart={onTouchStart} onTouchEnd={onTouchEnd}>
        {slides.map((s, i) => (
          <SlideView
            key={s.key}
            slide={s}
            index={i}
            total={slides.length}
            current={i === index}
            now={now}
            settings={settings}
            busy={busy === s.pickId}
            onAction={(x) => void act(x)}
          />
        ))}
      </div>
      {slides.length > 1 && (
        <div className="live-controls">
          <button
            type="button"
            className="live-arrow"
            aria-label={t("home.live.prev")}
            onClick={() => goTo(index - 1, true)}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M10 3 5 8l5 5" />
            </svg>
          </button>
          <div className="live-dots">
            {slides.map((s, i) => (
              <button
                key={s.key}
                type="button"
                className={`live-dot cat-${s.cat}${i === index ? " is-on" : ""}`}
                aria-label={t("home.live.goto", { n: i + 1, name: s.name })}
                aria-current={i === index ? "true" : undefined}
                onClick={() => goTo(i, true)}
              />
            ))}
          </div>
          <span className="live-pos num faint" aria-hidden="true">
            {index + 1} / {slides.length}
          </span>
          <button
            type="button"
            className="live-arrow"
            aria-label={t("home.live.next")}
            onClick={() => goTo(index + 1, true)}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M6 3l5 5-5 5" />
            </svg>
          </button>
        </div>
      )}
      <p className="sr-only" aria-live="polite" aria-atomic="true">
        {announce}
      </p>
      {actionError && (
        <p className="form-error live-error" role="alert">
          {actionError}
        </p>
      )}
      {enabled === false ? (
        <p className="live-note faint">
          {t("home.live.off")} {racesLink("home.live.enable")}
        </p>
      ) : (
        unread.length > 0 && (
          <p className="live-note faint">
            {t("home.live.partial", { site: unread.map((s) => SITE[s.id] ?? s.id).join(", ") })}{" "}
            {racesLink("home.live.seeRaces")}
          </p>
        )
      )}
    </section>
  );
}
