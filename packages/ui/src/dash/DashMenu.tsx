import { useEffect, useRef } from "react";
import { type TranslationKey, useT } from "../i18n";
import { appSlots } from "../mobile/slots";
import { navigate, type Route } from "../nav/routes";
import { connection } from "../net/connection";
import { useTelemetry } from "../store/telemetry";
import { DASH_PAGES, type DashPage } from "./model";
import { useDashPage } from "./store";
import { showWidgets } from "./view";

export const PAGE_TITLE: Record<DashPage, TranslationKey> = {
  course: "dash.page.course",
  tyres: "dash.page.tyres",
  energy: "dash.page.energy",
  timing: "dash.page.timing",
};

interface Props {
  open: boolean;
  onClose: () => void;
  onSettings: () => void;
}

/**
 * The sheet behind the ☰ button: the four pages, back to the widgets, the settings, the other
 * sections of Corde, and the way out of the demonstration.
 */
export function DashMenu({ open, onClose, onSettings }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const index = useDashPage((s) => s.index);
  const show = useDashPage((s) => s.show);
  const demo = useTelemetry((s) => s.demo);

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
          {DASH_PAGES.map((page, i) => (
            <button
              key={page}
              type="button"
              className={`dmenu-item${i === index ? " is-current" : ""}`}
              aria-current={i === index ? "page" : undefined}
              onClick={() => {
                show(i);
                onClose();
              }}
            >
              <span className="num dmenu-n">{i + 1}</span>
              {t(PAGE_TITLE[page])}
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
        <div className="dmenu-sep" />
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
