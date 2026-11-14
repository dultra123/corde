import { useEffect, useRef } from "react";
import { isBuiltin } from "../dashdef/builtin";
import type { DashDefinition, DashPageDef } from "../dashdef/model";
import { useDashes } from "../dashdef/store";
import { type TranslationKey, useT } from "../i18n";
import { appSlots } from "../mobile/slots";
import { navigate, type Route } from "../nav/routes";
import { connection } from "../net/connection";
import { useTelemetry } from "../store/telemetry";
import type { DashPage } from "./model";
import { useDashPage } from "./store";
import { showWidgets } from "./view";

export const PAGE_TITLE: Record<DashPage, TranslationKey> = {
  course: "dash.page.course",
  tyres: "dash.page.tyres",
  energy: "dash.page.energy",
  timing: "dash.page.timing",
};

/** The title of a page: translated for the pages of « Corde », its own name otherwise. */
export function pageTitle(
  def: DashDefinition,
  page: DashPageDef,
  t: (k: TranslationKey) => string,
): string {
  if (isBuiltin(def) && page.id in PAGE_TITLE) return t(PAGE_TITLE[page.id as DashPage]);
  return page.name;
}

interface Props {
  def: DashDefinition;
  open: boolean;
  onClose: () => void;
  onSettings: () => void;
}

/**
 * The sheet behind the ☰ button: the four pages, back to the widgets, the settings, the other
 * sections of Corde, and the way out of the demonstration.
 */
export function DashMenu({ def, open, onClose, onSettings }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const index = useDashPage((s) => s.index);
  const show = useDashPage((s) => s.show);
  const demo = useTelemetry((s) => s.demo);
  const defs = useDashes((s) => s.defs);
  const select = useDashes((s) => s.select);
  const studio = !appSlots.ServersPage && !isPhoneLike();

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      if (typeof d.showModal === "function") d.showModal();
      else d.setAttribute("open", "");
    } else if (!open && d.open) {
      if (typeof d.close === "function") d.close();
      else d.removeAttribute("open");
    }
  }, [open]);

  const go = (route: Route) => {
    onClose();
    navigate(route);
  };
  const toWidgets = () => {
    showWidgets();
    go({ page: "dashboard" });
  };
  const links: { key: TranslationKey; route: Route }[] = [
    { key: "nav.home", route: { page: "home" } },
    { key: "nav.races", route: { page: "races" } },
    { key: "nav.sessions", route: { page: "sessions" } },
    { key: "nav.profile", route: { page: "profile" } },
  ];
  if (appSlots.ServersPage) links.push({ key: "servers.title", route: { page: "servers" } });

  return (
    <dialog
      ref={ref}
      className="dmenu"
      aria-label={t("dash.menu")}
      onClose={onClose}
      onClick={(e) => {
        // A tap on the backdrop (outside the panel) closes the sheet.
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dmenu-panel">
        <div className="dmenu-head">
          <span className="dmenu-title">{t("dash.title")}</span>
          <button
            type="button"
            className="dmenu-close"
            onClick={onClose}
            aria-label={t("dash.close")}
          >
            ✕
          </button>
        </div>
        <div className="dmenu-pages">
          {def.pages.map((page, i) => (
            <button
              key={page.id}
              type="button"
              className={`dmenu-item${i === index ? " is-current" : ""}`}
              aria-current={i === index ? "page" : undefined}
              onClick={() => {
                show(i);
                onClose();
              }}
            >
              <span className="num dmenu-n">{i + 1}</span>
              {pageTitle(def, page, t)}
            </button>
          ))}
        </div>
        {demo === "manual" && (
          <button
            type="button"
            className="dmenu-item is-demo"
            onClick={() => {
              onClose();
              connection.setDemo(false);
            }}
          >
            {t("dash.quitDemo")}
          </button>
        )}
        {defs.length > 0 && (
          <>
            <div className="dmenu-sep" />
            <div className="dmenu-section">{t("studio.used")}</div>
            <div className="dmenu-pages">
              {[{ id: "corde", name: t("dash.corde") }, ...defs].map((d) => (
                <button
                  key={d.id}
                  type="button"
                  aria-current={d.id === def.id ? "true" : undefined}
                  className={`dmenu-item${d.id === def.id ? " is-current" : ""}`}
                  onClick={() => {
                    select(d.id);
                    onClose();
                  }}
                >
                  {d.name}
                </button>
              ))}
            </div>
          </>
        )}
        <div className="dmenu-sep" />
        {studio && (
          <button type="button" className="dmenu-item" onClick={() => go({ page: "studio" })}>
            {t("studio.open")}
          </button>
        )}
        <button type="button" className="dmenu-item" onClick={toWidgets}>
          {t("dash.toWidgets")}
        </button>
        <button
          type="button"
          className="dmenu-item"
          onClick={() => {
            onClose();
            onSettings();
          }}
        >
          {t("topbar.settings")}
        </button>
        <div className="dmenu-sep" />
        <nav className="dmenu-nav" aria-label={t("nav.label")}>
          {links.map((l) => (
            <button key={l.key} type="button" className="dmenu-item" onClick={() => go(l.route)}>
              {t(l.key)}
            </button>
          ))}
        </nav>
      </div>
    </dialog>
  );
}

/** A phone (or anything with a finger and a small screen): no Studio there. */
function isPhoneLike(): boolean {
  if (typeof window === "undefined") return false;
  const coarse = window.matchMedia?.("(pointer: coarse)").matches ?? false;
  return coarse && Math.min(window.innerWidth, window.innerHeight) <= 500;
}
