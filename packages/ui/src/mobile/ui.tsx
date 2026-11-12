import type { ReactNode } from "react";
import { IconFlag, IconHome, IconLive, IconProfile, IconSessions } from "../dashboard/icons";
import { useT } from "../i18n";
import { type Route, routeHash } from "../nav/routes";
import { useTelemetry } from "../store/telemetry";
import { appController, type ServerStatus, useApp } from "./app-state";

/** The pages that read a Corde server (everything but Home, Courses and « Mes serveurs »). */
export function needsServer(route: Route): boolean {
  return !(route.page === "home" || route.page === "races" || route.page === "servers");
}

/** The five sections, at the bottom of the screen (thumb reach, iOS habits). */
export function BottomTabs({ section }: { section: string }) {
  const { t } = useT();
  const keyboard = useApp((s) => s.keyboard);
  const tabs = [
    { id: "home", href: routeHash({ page: "home" }), icon: <IconHome /> },
    { id: "dashboard", href: routeHash({ page: "dashboard" }), icon: <IconLive /> },
    { id: "sessions", href: routeHash({ page: "sessions" }), icon: <IconSessions /> },
    { id: "races", href: routeHash({ page: "races" }), icon: <IconFlag /> },
    { id: "profile", href: routeHash({ page: "profile" }), icon: <IconProfile /> },
  ] as const;
  if (keyboard) return null;
  return (
    <nav className={`app-tabs is-${section}`} aria-label={t("mobile.tabs")}>
      {tabs.map((tab) => (
        <a
          key={tab.id}
          href={tab.href}
          className={`app-tab${section === tab.id ? " is-current" : ""}`}
          aria-current={section === tab.id ? "page" : undefined}
        >
          {tab.icon}
          <span>{t(`nav.${tab.id}`)}</span>
        </a>
      ))}
    </nav>
  );
}

function serversLink(primary: boolean, label: string) {
  return (
    <a className={`home-btn${primary ? " is-primary" : ""}`} href={routeHash({ page: "servers" })}>
      {label}
    </a>
  );
}

/** Why a page that needs a server cannot be shown, and what to do. */
export function NeedServer({ status }: { status: ServerStatus }) {
  const { t } = useT();
  const servers = useApp((s) => s.servers);
  const activeId = useApp((s) => s.activeId);
  const message = useApp((s) => s.statusMessage);
  const name = servers.find((s) => s.id === activeId)?.name ?? "";
  const retry = (
    <button
      type="button"
      className="home-btn is-primary"
      onClick={() => void appController().refresh()}
    >
      {t("mobile.gate.retry")}
    </button>
  );
  let title = "";
  let body: ReactNode = null;
  let actions: ReactNode = serversLink(true, t("mobile.gate.servers"));
  switch (status) {
    case "checking":
      return (
        <div className="notice app-gate" role="status">
          <div className="spinner" />
          <p className="notice-body">{t("mobile.gate.checking", { name })}</p>
        </div>
      );
    case "none":
      title = t("mobile.gate.none.title");
      body = <p>{t("mobile.gate.none.body")}</p>;
      break;
    case "unpaired":
      title = t("mobile.gate.unpaired.title", { name });
      body = <p>{t("mobile.gate.unpaired.body")}</p>;
      break;
    case "unreachable":
      title = t("mobile.gate.unreachable.title", { name });
      body = (
        <>
          <p>{t("mobile.gate.unreachable.body")}</p>
          <ul className="app-gate-list">
            <li>{t("mobile.gate.unreachable.on")}</li>
            <li>{t("mobile.gate.unreachable.lan")}</li>
            <li>{t("mobile.gate.unreachable.wifi")}</li>
            <li>{t("mobile.gate.unreachable.local")}</li>
          </ul>
        </>
      );
      actions = (
        <>
          {retry}
          {serversLink(false, t("mobile.gate.servers"))}
        </>
      );
      break;
    case "refused":
      title = t("mobile.gate.refused.title", { name });
      body = (
        <>
          {message && <p className="dim">« {message} »</p>}
          <p>{t("mobile.gate.refused.body")}</p>
        </>
      );
      actions = (
        <>
          {retry}
          {serversLink(false, t("mobile.gate.servers"))}
        </>
      );
      break;
    case "not_corde":
      title = t("mobile.gate.notCorde.title");
      body = <p>{t("mobile.gate.notCorde.body")}</p>;
      break;
    case "error":
      title = t("mobile.gate.error.title", { name });
      body = message ? <p className="dim">{message}</p> : null;
      actions = (
        <>
          {retry}
          {serversLink(false, t("mobile.gate.servers"))}
        </>
      );
      break;
    case "ok":
      return null;
  }
  return (
    <section className="card app-gate" aria-labelledby="app-gate-title">
      <h1 id="app-gate-title" className="app-gate-title">
        {title}
      </h1>
      <div className="app-gate-body">{body}</div>
      <div className="app-gate-actions">{actions}</div>
      <p className="faint app-gate-foot">
        <a className="link" href={routeHash({ page: "races" })}>
          {t("mobile.gate.racesFree")}
        </a>
      </p>
    </section>
  );
}

/** `useServerGate` of the slots: the page, or why not. */
export function useServerGate(route: Route): ReactNode | null {
  const status = useApp((s) => s.status);
  if (!needsServer(route) || status === "ok") return null;
  return <NeedServer status={status} />;
}

/** `useServerOnline` of the slots: the active server answers on the socket. */
export function useServerOnline(): boolean {
  const status = useApp((s) => s.status);
  const open = useTelemetry((s) => s.socket === "open");
  return status === "ok" && open;
}

/** The home page's card about the active server. */
export function HomeServerCard() {
  const { t } = useT();
  const servers = useApp((s) => s.servers);
  const activeId = useApp((s) => s.activeId);
  const status = useApp((s) => s.status);
  const active = servers.find((s) => s.id === activeId) ?? null;
  return (
    <section className="card home-card app-server-card" aria-labelledby="app-server-title">
      <div className="home-card-head">
        <h2 id="app-server-title" className="home-card-title">
          {t("mobile.home.server.title")}
        </h2>
        {active && (
          <a className="link" href={routeHash({ page: "servers" })}>
            {t("mobile.home.server.manage")}
          </a>
        )}
      </div>
      {active ? (
        <p className="app-server-line">
          <span className={`app-dot is-${status}`} aria-hidden="true" />
          <b>{active.name}</b>
          <span className="dim num">{active.label}</span>
          <span className="dim">· {t(`mobile.status.${status}`)}</span>
        </p>
      ) : (
        <>
          <p className="home-card-empty dim">{t("mobile.home.server.none")}</p>
          <div className="app-gate-actions">{serversLink(true, t("mobile.home.server.add"))}</div>
        </>
      )}
    </section>
  );
}

/** Under the reminder delays of the Courses page: what the phone's notifications will do. */
export function ReminderStatus() {
  const { t } = useT();
  const permission = useApp((s) => s.permission);
  if (permission === null) return null;
  const ask = async () => {
    const p = await appController().races.askPermission();
    useApp.setState({ permission: p });
  };
  return (
    <p className={`races-hint app-notif is-${permission}`} role="status">
      {t(`mobile.notif.${permission}`)}
      {permission === "prompt" && (
        <>
          {" "}
          <button type="button" className="link" onClick={() => void ask()}>
            {t("mobile.notif.ask")}
          </button>
        </>
      )}
    </p>
  );
}
