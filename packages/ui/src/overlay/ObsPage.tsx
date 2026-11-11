import type { DashboardLayout } from "@corde/protocol";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { navigate } from "../nav/routes";
import { fetchNetwork } from "../net/system-api";
import { useAsync } from "../sessions/shared";
import { useDashboard } from "../store/dashboard";
import { listWidgets, type WidgetDef } from "../widgets";
import {
  DEFAULT_OPTIONS,
  OPACITY_MAX,
  OPACITY_MIN,
  type OverlayBg,
  type OverlayOptions,
  type OverlayTarget,
  overlayHash,
  overlayUrl,
  type PxSize,
  recommendedLayoutSize,
  recommendedWidgetSize,
  roundOption,
  SCALE_MAX,
  SCALE_MIN,
} from "./model";

/** The address of this PC as OBS on the same PC reaches it: reveals nothing. */
function localBase(port: number | null): string {
  const p = port ?? (Number(window.location.port) || 8471);
  return `http://127.0.0.1:${p}`;
}

/** Copies a text; falls back on the selection of `input` where the clipboard API is missing. */
async function copyText(text: string, input: HTMLInputElement | null): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the selection
  }
  if (!input) return false;
  input.focus();
  input.select();
  try {
    return document.execCommand("copy");
  } catch {
    return false;
  }
}

function CopyField({ url, label }: { url: string; label: string }) {
  const { t } = useT();
  const ref = useRef<HTMLInputElement>(null);
  const [state, setState] = useState<"idle" | "done" | "failed">("idle");
  useEffect(() => {
    if (state === "idle") return;
    const timer = setTimeout(() => setState("idle"), 1800);
    return () => clearTimeout(timer);
  }, [state]);
  return (
    <div className="obs-copy">
      <input
        ref={ref}
        className="input obs-url num"
        readOnly
        value={url}
        aria-label={label}
        onFocus={(e) => e.currentTarget.select()}
      />
      <button
        type="button"
        className={`btn obs-copy-btn${state === "done" ? " is-active" : ""}`}
        onClick={async () => setState((await copyText(url, ref.current)) ? "done" : "failed")}
      >
        {t(state === "done" ? "obs.copied" : state === "failed" ? "obs.copyFailed" : "obs.copy")}
      </button>
    </div>
  );
}

/** Largest box a preview may take on the page. */
const PREVIEW_BOX = { width: 360, height: 216 };

/**
 * What OBS will show, on a checkerboard that stands for the transparency: the real overlay page
 * in a frame at its advised size, scaled down. `?preview=1&demo=1`: demo data, stopped after a
 * few seconds (a still picture, no work left running on this page).
 */
function Preview({ hash, size, label }: { hash: string; size: PxSize; label: string }) {
  const k = Math.min(1, PREVIEW_BOX.width / size.width, PREVIEW_BOX.height / size.height);
  const src = `${window.location.pathname}?demo=1&preview=1${hash}`;
  return (
    <div className="obs-preview checkerboard">
      <div
        className="obs-preview-box"
        style={{ width: Math.round(size.width * k), height: Math.round(size.height * k) }}
      >
        <iframe
          className="obs-preview-frame"
          title={label}
          src={src}
          loading="lazy"
          tabIndex={-1}
          style={{ width: size.width, height: size.height, transform: `scale(${k})` }}
        />
      </div>
    </div>
  );
}

function ItemCard({
  name,
  target,
  options,
  size,
  base,
  extra,
}: {
  name: string;
  target: OverlayTarget;
  options: OverlayOptions;
  size: PxSize;
  base: string;
  extra?: ReactNode;
}) {
  const { t } = useT();
  const hash = overlayHash(target, options);
  return (
    <li className="card obs-item">
      <Preview hash={hash} size={size} label={t("obs.previewOf", { name })} />
      <div className="obs-item-text">
        <div className="obs-item-head">
          <h3 className="obs-item-name">{name}</h3>
          <span className="obs-size num" title={t("obs.sizeHint")}>
            {t("obs.size", { w: size.width, h: size.height })}
          </span>
        </div>
        <CopyField url={overlayUrl(base, target, options)} label={t("obs.urlOf", { name })} />
        {extra}
      </div>
    </li>
  );
}

function WidgetSettings({
  def,
  value,
  onChange,
}: {
  def: WidgetDef;
  value: Record<string, string>;
  onChange(v: Record<string, string>): void;
}) {
  const { t } = useT();
  if (!def.configFields?.length) return null;
  return (
    <div className="obs-item-config">
      {def.configFields.map((f) => {
        const cur = value[f.key] ?? String(def.defaultConfig?.[f.key] ?? f.options[0]?.value ?? "");
        return (
          <label key={f.key} className="obs-inline">
            <span className="dim">{t(f.labelKey)}</span>
            <select
              className="widget-config-select"
              value={cur}
              onChange={(e) => {
                const next = { ...value };
                if (e.target.value === String(def.defaultConfig?.[f.key] ?? "")) delete next[f.key];
                else next[f.key] = e.target.value;
                onChange(next);
              }}
            >
              {f.options.map((o) => (
                <option key={o.value} value={o.value}>
                  {t(o.labelKey)}
                </option>
              ))}
            </select>
          </label>
        );
      })}
    </div>
  );
}

function OptionsCard({
  options,
  onChange,
}: {
  options: OverlayOptions;
  onChange(o: OverlayOptions): void;
}) {
  const { t } = useT();
  const set = (patch: Partial<OverlayOptions>) => onChange({ ...options, ...patch });
  const bgs: OverlayBg[] = ["none", "glass", "solid"];
  return (
    <section className="card obs-options" aria-labelledby="obs-options-title">
      <div className="card-head">
        <h2 id="obs-options-title">{t("obs.options.title")}</h2>
        <button type="button" className="link" onClick={() => onChange(DEFAULT_OPTIONS)}>
          {t("obs.options.reset")}
        </button>
      </div>
      <div className="obs-options-grid">
        <fieldset className="obs-field">
          <legend className="dim">{t("obs.options.bg")}</legend>
          <div className="obs-seg">
            {bgs.map((bg) => (
              <button
                key={bg}
                type="button"
                aria-pressed={options.bg === bg}
                className={`btn${options.bg === bg ? " is-active" : ""}`}
                onClick={() => set({ bg })}
              >
                {t(`obs.bg.${bg}`)}
              </button>
            ))}
          </div>
          <p className="obs-hint faint">{t(`obs.bg.${options.bg}.hint`)}</p>
        </fieldset>
        <label className="obs-field">
          <span className="dim">
            {t("obs.options.opacity")} <b className="num">{Math.round(options.opacity * 100)} %</b>
          </span>
          <input
            type="range"
            min={OPACITY_MIN * 100}
            max={OPACITY_MAX * 100}
            step={5}
            value={Math.round(options.opacity * 100)}
            onChange={(e) => set({ opacity: roundOption(Number(e.target.value) / 100) })}
          />
        </label>
        <label className="obs-field">
          <span className="dim">
            {t("obs.options.scale")} <b className="num">{Math.round(options.scale * 100)} %</b>
          </span>
          <input
            type="range"
            min={SCALE_MIN * 100}
            max={SCALE_MAX * 100}
            step={5}
            value={Math.round(options.scale * 100)}
            onChange={(e) => set({ scale: roundOption(Number(e.target.value) / 100) })}
          />
        </label>
        <div className="obs-field">
          <label className="obs-check">
            <input
              type="checkbox"
              checked={options.hideOffTrack}
              onChange={(e) => set({ hideOffTrack: e.target.checked })}
            />
            <span>{t("obs.options.hide")}</span>
          </label>
          <label className="obs-check">
            <input
              type="checkbox"
              checked={options.title}
              onChange={(e) => set({ title: e.target.checked })}
            />
            <span>{t("obs.options.titles")}</span>
          </label>
          <label className="obs-inline">
            <span className="dim">{t("obs.options.lang")}</span>
            <select
              className="widget-config-select"
              value={options.lang ?? ""}
              onChange={(e) =>
                set({
                  lang: e.target.value === "fr" || e.target.value === "en" ? e.target.value : null,
                })
              }
            >
              <option value="">{t("obs.lang.app")}</option>
              <option value="fr">Français</option>
              <option value="en">English</option>
            </select>
          </label>
        </div>
      </div>
    </section>
  );
}

/**
 * Where OBS reaches Corde. This PC (`127.0.0.1`) by default: it works when OBS runs on the
 * gaming PC and reveals nothing on a stream. The network address (second streaming PC) is shown
 * only on request, like the phone card of the home page, and is never remembered.
 */
function AddressCard({
  lan,
  setLan,
  lanBase,
  setLanBase,
}: {
  lan: boolean;
  setLan(on: boolean): void;
  lanBase: string | null;
  setLanBase(base: string | null): void;
}) {
  const { t } = useT();
  const net = useAsync((signal) => (lan ? fetchNetwork(signal) : Promise.resolve(null)), [lan]);
  const urls = net.data?.listening_on_lan ? net.data.urls : [];
  useEffect(() => {
    if (!lan) setLanBase(null);
    else if (urls.length > 0 && (!lanBase || !urls.includes(lanBase))) setLanBase(urls[0] ?? null);
  }, [lan, urls, lanBase, setLanBase]);
  return (
    <section className="card obs-address" aria-labelledby="obs-address-title">
      <div className="card-head">
        <h2 id="obs-address-title">{t("obs.address.title")}</h2>
      </div>
      <div className="obs-seg">
        <button
          type="button"
          aria-pressed={!lan}
          className={`btn${!lan ? " is-active" : ""}`}
          onClick={() => setLan(false)}
        >
          {t("obs.address.local")}
        </button>
        <button
          type="button"
          aria-pressed={lan}
          className={`btn${lan ? " is-active" : ""}`}
          onClick={() => setLan(true)}
        >
          {t("obs.address.lan")}
        </button>
      </div>
      <p className="obs-hint dim">
        {!lan
          ? t("obs.address.localHint")
          : net.loading
            ? t("common.loading")
            : urls.length > 0
              ? t("obs.address.lanHint")
              : net.data?.lan_access_setting
                ? t("obs.address.lanRestart")
                : t("obs.address.lanOff")}
      </p>
      {lan && urls.length > 1 && (
        <p className="obs-hint dim">
          {t("obs.address.other")}{" "}
          {urls
            .filter((u) => u !== lanBase)
            .map((u) => (
              <button key={u} type="button" className="link num" onClick={() => setLanBase(u)}>
                {u.replace(/^https?:\/\//, "")}
              </button>
            ))}
        </p>
      )}
    </section>
  );
}

/**
 * Page « OBS » (`#/obs`): every widget and every overlay layout with a preview on a
 * checkerboard, the address to paste into an OBS browser source, its advised size and the
 * options of the address. Reached from the dashboard's layout menu and from the home page.
 */
export function ObsPage() {
  const { t } = useT();
  const layouts = useDashboard((s) => s.layouts);
  const overlays = layouts.filter((l) => l.device === "overlay");
  const [options, setOptions] = useState<OverlayOptions>(DEFAULT_OPTIONS);
  const [widgetOpts, setWidgetOpts] = useState<Record<string, Record<string, string>>>({});
  const [lan, setLan] = useState(false);
  const [lanBase, setLanBase] = useState<string | null>(null);
  const local = useAsync((signal) => fetchNetwork(signal), []);
  const base = lan && lanBase ? lanBase : localBase(local.data?.port ?? null);
  const canEdit = typeof window !== "undefined" && window.innerWidth >= 700;

  const edit = (l: DashboardLayout) => {
    useDashboard.getState().selectLayout(l.id);
    navigate({ page: "dashboard" });
  };
  const create = () => {
    const st = useDashboard.getState();
    st.setDevice("desktop");
    st.createLayout(t("layout.newOverlay"), "overlay");
    st.setEditing(true);
    st.setPaletteOpen(true);
    navigate({ page: "dashboard" });
  };

  return (
    <div className="obs-page">
      <header className="page-head obs-head">
        <h1 className="obs-title">{t("obs.title")}</h1>
        <p className="dim obs-lead">{t("obs.lead")}</p>
      </header>

      <div className="obs-top">
        <section className="card obs-steps" aria-labelledby="obs-steps-title">
          <div className="card-head">
            <h2 id="obs-steps-title">{t("obs.steps.title")}</h2>
          </div>
          <ol className="obs-steps-list">
            {([1, 2, 3] as const).map((n) => (
              <li key={n}>
                <b>{t(`obs.steps.${n}.title`)}</b>
                <span className="dim">{t(`obs.steps.${n}.text`)}</span>
              </li>
            ))}
          </ol>
          <p className="obs-hint faint">{t("obs.steps.css")}</p>
        </section>
        <AddressCard lan={lan} setLan={setLan} lanBase={lanBase} setLanBase={setLanBase} />
      </div>

      <OptionsCard options={options} onChange={setOptions} />

      <section aria-labelledby="obs-layouts-title">
        <div className="obs-section-head">
          <h2 id="obs-layouts-title">{t("obs.layouts.title")}</h2>
          {canEdit && (
            <button type="button" className="btn" onClick={create}>
              {t("layout.newOverlay")}
            </button>
          )}
        </div>
        <p className="obs-hint dim">{t("obs.layouts.hint")}</p>
        {overlays.length === 0 ? (
          <p className="dim">{t("obs.layouts.none")}</p>
        ) : (
          <ul className="obs-list">
            {overlays.map((l) => (
              <ItemCard
                key={l.id}
                name={l.name}
                target={{ kind: "layout", id: l.id }}
                options={options}
                size={recommendedLayoutSize(l)}
                base={base}
                extra={
                  <p className="obs-item-meta dim">
                    {t("obs.layouts.count", { n: l.widgets.length })}
                    {canEdit && (
                      <>
                        {" · "}
                        <button type="button" className="link" onClick={() => edit(l)}>
                          {t("obs.layouts.edit")}
                        </button>
                      </>
                    )}
                  </p>
                }
              />
            ))}
          </ul>
        )}
      </section>

      <section aria-labelledby="obs-widgets-title">
        <div className="obs-section-head">
          <h2 id="obs-widgets-title">{t("obs.widgets.title")}</h2>
        </div>
        <p className="obs-hint dim">{t("obs.widgets.hint")}</p>
        <ul className="obs-list">
          {listWidgets().map((def) => {
            const own = widgetOpts[def.id] ?? {};
            return (
              <ItemCard
                key={def.id}
                name={t(def.titleKey)}
                target={{ kind: "widget", id: def.id }}
                options={{ ...options, widget: own }}
                size={recommendedWidgetSize(def, options.scale)}
                base={base}
                extra={
                  <WidgetSettings
                    def={def}
                    value={own}
                    onChange={(v) => setWidgetOpts((prev) => ({ ...prev, [def.id]: v }))}
                  />
                }
              />
            );
          })}
        </ul>
      </section>
    </div>
  );
}
