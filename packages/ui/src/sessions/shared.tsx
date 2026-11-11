import type { SessionType, Weather } from "@corde/protocol";
import { type ReactNode, useCallback, useEffect, useRef, useState } from "react";
import { IconBack } from "../dashboard/icons";
import { type TranslationKey, useT } from "../i18n";
import { fmtNum } from "../lib/format";
import { type Route, routeHash } from "../nav/routes";
import { ApiError } from "../net/sessions-api";

/* ------------------------------------------------------------------ data hook */

export interface AsyncState<T> {
  data: T | null;
  error: string | null;
  loading: boolean;
  reload: () => void;
}

/**
 * Runs `fn` (abortable) whenever `deps` change; keeps the previous data while reloading so lists
 * do not flash. Errors are turned into a readable message.
 */
export function useAsync<T>(
  fn: (signal: AbortSignal) => Promise<T>,
  deps: unknown[],
): AsyncState<T> {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [tick, setTick] = useState(0);
  const fnRef = useRef(fn);
  fnRef.current = fn;

  // biome-ignore lint/correctness/useExhaustiveDependencies: deps are the caller's cache key
  useEffect(() => {
    const ctrl = new AbortController();
    setLoading(true);
    fnRef
      .current(ctrl.signal)
      .then((d) => {
        if (ctrl.signal.aborted) return;
        setData(d);
        setError(null);
      })
      .catch((e: unknown) => {
        if (ctrl.signal.aborted) return;
        setError(errorMessage(e));
      })
      .finally(() => {
        if (!ctrl.signal.aborted) setLoading(false);
      });
    return () => ctrl.abort();
  }, [...deps, tick]);

  const reload = useCallback(() => setTick((n) => n + 1), []);
  return { data, error, loading, reload };
}

export function errorMessage(e: unknown): string {
  if (e instanceof ApiError) return e.status ? `${e.message} (HTTP ${e.status})` : e.message;
  return e instanceof Error ? e.message : String(e);
}

/* ------------------------------------------------------------------ chrome */

interface HeaderProps {
  back?: Route;
  backLabel?: string;
  title: ReactNode;
  subtitle?: ReactNode;
  meta?: ReactNode;
  actions?: ReactNode;
}

export function PageHeader({ back, backLabel, title, subtitle, meta, actions }: HeaderProps) {
  const { t } = useT();
  return (
    <header className="page-head">
      {back && (
        <a className="page-back" href={routeHash(back)}>
          <IconBack />
          <span>{backLabel ?? t("common.back")}</span>
        </a>
      )}
      <div className="page-head-row">
        <div className="page-head-main">
          <h1 className="page-title">{title}</h1>
          {subtitle && <div className="page-subtitle dim">{subtitle}</div>}
        </div>
        {actions && <div className="page-actions">{actions}</div>}
      </div>
      {meta && <div className="page-meta">{meta}</div>}
    </header>
  );
}

export function Notice({
  kind,
  title,
  children,
  action,
}: {
  kind: "loading" | "error" | "empty";
  title?: ReactNode;
  children?: ReactNode;
  action?: ReactNode;
}) {
  return (
    <div className={`notice is-${kind}`} role={kind === "error" ? "alert" : undefined}>
      {kind === "loading" && <span className="spinner" aria-hidden="true" />}
      {title && <div className="notice-title">{title}</div>}
      {children && <div className="notice-body dim">{children}</div>}
      {action && <div className="notice-action">{action}</div>}
    </div>
  );
}

/* ------------------------------------------------------------------ labels */

export function sessionTypeKey(type: SessionType): TranslationKey {
  return `session.type.${type}` as TranslationKey;
}

export function SessionTypeTag({ type }: { type: SessionType }) {
  const { t } = useT();
  return <span className={`typetag is-${type}`}>{t(sessionTypeKey(type))}</span>;
}

/** `Sec · air 24° · piste 32°` — one line of weather. */
export function weatherSummary(
  w: Pick<Weather, "rain" | "wetness_avg" | "ambient_c" | "track_c">,
  t: (k: TranslationKey) => string,
): string {
  const cond =
    w.rain > 0.05
      ? `${t("weather.wet")} ${fmtNum(w.rain * 100, 0)} %`
      : w.wetness_avg > 0.1
        ? t("weather.damp")
        : t("weather.dry");
  return `${cond} · ${t("weather.air")} ${fmtNum(w.ambient_c, 0)}° · ${t("weather.track")} ${fmtNum(w.track_c, 0)}°`;
}

/* ------------------------------------------------------------------ mini bars */

/**
 * Four tiny vertical bars (FL FR RL RR) with a value-driven colour. Heights are relative to
 * `[min, max]`; the hottest / most worn value is printed next to them by the caller.
 */
export function MiniBars({
  values,
  min,
  max,
  color,
  title,
}: {
  values: readonly [number, number, number, number];
  min: number;
  max: number;
  color: (v: number) => string;
  title?: string;
}) {
  const span = Math.max(1e-6, max - min);
  return (
    <span className="minibars" title={title}>
      {values.map((v, i) => {
        const f = Math.max(0.08, Math.min(1, (v - min) / span));
        return (
          <span key={i} className="minibar">
            <span
              className="minibar-fill"
              style={{ height: `${f * 100}%`, background: color(v) }}
            />
          </span>
        );
      })}
    </span>
  );
}
