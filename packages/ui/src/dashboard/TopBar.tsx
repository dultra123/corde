import { useEffect, useState } from "react";
import { useAutoPicksDash } from "../dash/view";
import { useT } from "../i18n";
import { hasUpdate } from "../lib/updates";
import { routeHash, type Section } from "../nav/routes";
import { connection } from "../net/connection";
import { useDashboard } from "../store/dashboard";
import { useHub } from "../store/hub";
import { useSettings } from "../store/settings";
import { useTelemetry } from "../store/telemetry";
import { useUpdates } from "../store/updates";
import {
  IconCheck,
  IconDemo,
  IconEdit,
  IconExitFullscreen,
  IconFlag,
  IconFullscreen,
  IconGauge,
  IconHome,
  IconLive,
  IconPlus,
  IconProfile,
  IconSessions,
  IconSettings,
} from "./icons";
import { LayoutMenu } from "./LayoutMenu";
import { StatusPill } from "./StatusPill";

function useFullscreen(): [boolean, () => void] {
  const [on, setOn] = useState(
    () => typeof document !== "undefined" && !!document.fullscreenElement,
  );
  useEffect(() => {
    const h = () => setOn(!!document.fullscreenElement);
    document.addEventListener("fullscreenchange", h);
    return () => document.removeEventListener("fullscreenchange", h);
  }, []);
  const toggle = () => {
    if (document.fullscreenElement) void document.exitFullscreen();
    else void document.documentElement.requestFullscreen?.();
  };
  return [on, toggle];
}

/** The top-level tabs: home, live dashboard, recorded sessions, online races, driver profile. */
function NavTabs({ section }: { section: Section }) {
  const { t } = useT();
  const tabs = [
    { id: "home" as const, href: routeHash({ page: "home" }), icon: <IconHome /> },
    { id: "dashboard" as const, href: routeHash({ page: "dashboard" }), icon: <IconLive /> },
    { id: "sessions" as const, href: routeHash({ page: "sessions" }), icon: <IconSessions /> },
    { id: "races" as const, href: routeHash({ page: "races" }), icon: <IconFlag /> },
    { id: "profile" as const, href: routeHash({ page: "profile" }), icon: <IconProfile /> },
  ];
  return (
    <nav className="navtabs" aria-label={t("nav.label")}>
      {tabs.map((tab) => (
        <a
          key={tab.id}
          href={tab.href}
          className={`navtab${section === tab.id ? " is-current" : ""}`}
          aria-current={section === tab.id ? "page" : undefined}
          title={t(`nav.${tab.id}`)}
        >
          {tab.icon}
          <span className="navtab-label">{t(`nav.${tab.id}`)}</span>
        </a>
      ))}
    </nav>
  );
}

interface Props {
  onSettings: () => void;
  /** Which section is shown: the dashboard gets its layout / edit / demo controls. */
  section?: Section;
  /** The race dashboard is shown in the page: no widget editing. */
  dash?: boolean;
}

export function TopBar({ onSettings, section = "dashboard", dash = false }: Props) {
  const { t } = useT();
  const editing = useDashboard((s) => s.editing);
  const paletteOpen = useDashboard((s) => s.paletteOpen);
  const setEditing = useDashboard((s) => s.setEditing);
  const setPaletteOpen = useDashboard((s) => s.setPaletteOpen);
  const demo = useTelemetry((s) => s.demo);
  const [fs, toggleFs] = useFullscreen();
  const updateKnown = useUpdates((s) => hasUpdate(s.status));
  const demoManual = demo === "manual";
  // On the home server `#/` explains where the live view is: no dashboard controls.
  const onHub = useHub((s) => s.role === "hub");
  const onDashboard = section === "dashboard" && !onHub;
  // A phone (or the phone application) that chose the widgets: the way back to its dashboard.
  // `auto` never picks the phone dashboard on a PC, so a PC never shows this button.
  const autoDash = useAutoPicksDash();
  const backToDash = onDashboard && autoDash && !editing && !dash;
  const widgetControls = onDashboard && !dash;

  return (
    <header className={`topbar is-${section}`}>
      <div className="topbar-left">
        <a className="brand" href={routeHash({ page: "home" })} title={t("app.name")}>
          <svg viewBox="0 0 32 32" aria-hidden="true">
            <path
              d="M23.39 9.11A9 9 0 1 0 23.39 22.89"
              fill="none"
              stroke="currentColor"
              strokeWidth="5"
              strokeLinecap="round"
            />
            <circle cx="14.7" cy="16" r="2.8" fill="currentColor" />
          </svg>
          <span className="sr-only">{t("app.name")}</span>
        </a>
        <NavTabs section={section} />
        {onDashboard && <LayoutMenu />}
      </div>
      <div className="topbar-center">
        <StatusPill onClick={onSettings} />
      </div>
      <div className="topbar-right">
        {backToDash && (
          <button
            type="button"
            className="btn"
            onClick={() => useSettings.getState().update({ liveView: "auto" })}
            title={t("topbar.toDash")}
          >
            <IconGauge />
            <span className="btn-label">{t("topbar.toDash")}</span>
          </button>
        )}
        {widgetControls && editing && (
          <button
            type="button"
            className={`btn is-icon${paletteOpen ? " is-active" : ""}`}
            onClick={() => setPaletteOpen(!paletteOpen)}
            aria-label={t("topbar.addWidget")}
            title={t("topbar.addWidget")}
          >
            <IconPlus />
          </button>
        )}
        {widgetControls && (
          <button
            type="button"
            className={`btn${editing ? " is-active" : ""}`}
            onClick={() => {
              setEditing(!editing);
              if (!editing) setPaletteOpen(true);
            }}
            aria-pressed={editing}
            aria-label={editing ? t("topbar.done") : t("topbar.edit")}
            title={editing ? t("topbar.done") : t("topbar.edit")}
          >
            {editing ? <IconCheck /> : <IconEdit />}
            <span className="btn-label">{editing ? t("topbar.done") : t("topbar.edit")}</span>
          </button>
        )}
        {onDashboard && (
          <button
            type="button"
            className={`btn is-icon${demoManual ? " is-active" : ""}`}
            onClick={() => connection.setDemo(!demoManual)}
            aria-pressed={demoManual}
            aria-label={demoManual ? t("topbar.demoOff") : t("topbar.demoOn")}
            title={demoManual ? t("topbar.demoOff") : t("topbar.demoOn")}
          >
            <IconDemo />
          </button>
        )}
        <button
          type="button"
          className="btn is-icon is-fullscreen"
          onClick={toggleFs}
          aria-label={fs ? t("topbar.exitFullscreen") : t("topbar.fullscreen")}
          title={fs ? t("topbar.exitFullscreen") : t("topbar.fullscreen")}
        >
          {fs ? <IconExitFullscreen /> : <IconFullscreen />}
        </button>
        <button
          type="button"
          className={`btn is-icon${updateKnown ? " has-badge" : ""}`}
          onClick={onSettings}
          aria-label={updateKnown ? t("topbar.settingsUpdate") : t("topbar.settings")}
          title={updateKnown ? t("topbar.settingsUpdate") : t("topbar.settings")}
        >
          <IconSettings />
          {updateKnown && <span className="btn-badge" aria-hidden="true" />}
        </button>
      </div>
    </header>
  );
}
