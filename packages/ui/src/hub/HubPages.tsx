import { useState } from "react";
import { IconLive, IconSessions } from "../dashboard/icons";
import { useT } from "../i18n";
import { fmtLapTime, fmtRelativeDay } from "../lib/format";
import { routeHash } from "../nav/routes";
import {
  fetchDevices,
  fetchHubStatus,
  type HubDevice,
  revokeDevice,
  updateDevice,
} from "../net/hub-api";
import { fetchSessions } from "../net/sessions-api";
import { errorMessage, Notice, PageHeader, useAsync } from "../sessions/shared";
import { useHub } from "../store/hub";
import { liveUrl } from "./lib";

/* ------------------------------------------------------------------ home */

/**
 * `#/home` on the home server: what it holds, when the PC last sent something, this device, the
 * latest sessions. No live state: the game runs on the PC.
 */
export function HubHomePage() {
  const { t, lang, nOf } = useT();
  const status = useAsync((signal) => fetchHubStatus(signal), []);
  const sessions = useAsync((signal) => fetchSessions({ limit: 5 }, signal), []);
  const s = status.data;
  const pc = s?.last_pc ?? null;

  return (
    <div className="home-page hub-home">
      <PageHeader
        title={t("hub.home.title")}
        subtitle={s ? t("hub.home.subtitle", { version: s.version }) : undefined}
      />
      {status.error && (
        <Notice kind="error" title={t("hub.home.error")}>
          {status.error}
        </Notice>
      )}
      <div className="hub-grid">
        <section className="card home-card hub-sync">
          <div className="home-card-head">
            <h2 className="home-card-title">{t("hub.home.sync")}</h2>
            {pc && <span className="home-when dim">{fmtRelativeDay(pc.synced_ms, lang)}</span>}
          </div>
          {pc ? (
            <p>
              {t("hub.home.syncFrom", { name: pc.device_name, version: pc.version })}
              <br />
              <span className="dim">
                {s
                  ? `${nOf(s.sessions, "hub.home.session", "hub.home.sessions")} · ${nOf(
                      s.results,
                      "hub.home.result",
                      "hub.home.results",
                    )}`
                  : ""}
              </span>
            </p>
          ) : (
            <div className="home-card-empty">
              <p>{t("hub.home.noPc")}</p>
              <ol className="hub-steps">
                <li>{t("hub.home.noPc.step1")}</li>
                <li>{t("hub.home.noPc.step2")}</li>
                <li>{t("hub.home.noPc.step3")}</li>
              </ol>
            </div>
          )}
        </section>

        <section className="card home-card hub-you">
          <div className="home-card-head">
            <h2 className="home-card-title">{t("hub.home.devices")}</h2>
            <a className="link" href={routeHash({ page: "devices" })}>
              {t("hub.home.devicesLink")}
            </a>
          </div>
          {s && (
            <p>
              {t("hub.home.you", { name: s.you.name })}
              {s.you.admin && <span className="echip is-accent hub-admin">{t("hub.admin")}</span>}
              <br />
              <span className="dim">{nOf(s.devices, "hub.home.device", "hub.home.devicesN")}</span>
            </p>
          )}
        </section>

        <section className="card home-card hub-live">
          <div className="home-card-head">
            <h2 className="home-card-title">{t("hub.home.live")}</h2>
          </div>
          <p className="dim">{t("hub.direct.short")}</p>
          {liveUrl(pc) && (
            <p>
              <a className="home-btn" href={liveUrl(pc) ?? undefined}>
                <IconLive />
                {t("hub.direct.open")}
              </a>
            </p>
          )}
        </section>

        <section className="card home-card hub-sessions">
          <div className="home-card-head">
            <h2 className="home-card-title">{t("hub.home.latest")}</h2>
            <a className="link" href={routeHash({ page: "sessions" })}>
              {t("hub.home.allSessions")}
            </a>
          </div>
          {sessions.data && sessions.data.length === 0 && (
            <p className="home-card-empty dim">{t("hub.home.noSession")}</p>
          )}
          <ul className="hub-session-list">
            {(sessions.data ?? []).map((row) => (
              <li key={row.id}>
                <a href={routeHash({ page: "session", id: row.id })} className="hub-session">
                  <IconSessions />
                  <span className="hub-session-track">{row.track_name}</span>
                  <span className="dim">{fmtRelativeDay(row.started_wall_ms, lang)}</span>
                  <span className="num">{fmtLapTime(row.best_lap_s)}</span>
                </a>
              </li>
            ))}
          </ul>
        </section>
      </div>
    </div>
  );
}

/* ------------------------------------------------------------------ direct */

/** `#/` on the home server: the live view is the PC's. */
export function HubDirectPage() {
  const { t, lang } = useT();
  const status = useAsync((signal) => fetchHubStatus(signal), []);
  const pc = status.data?.last_pc ?? null;
  const url = liveUrl(pc);
  return (
    <div className="home-page hub-direct">
      <PageHeader title={t("hub.direct.title")} />
      <section className="card home-card">
        <p>{t("hub.direct.body")}</p>
        {url ? (
          <>
            <p className="dim">
              {t("hub.direct.lastPc", {
                name: pc?.device_name ?? "",
                when: fmtRelativeDay(pc?.synced_ms, lang),
              })}
            </p>
            <p>
              <a className="home-btn is-primary" href={url}>
                <IconLive />
                {t("hub.direct.open")}
              </a>
              <span className="num dim hub-url">{url.replace(/\/#\/$/, "")}</span>
            </p>
            <p className="faint">{t("hub.direct.sameNetwork")}</p>
          </>
        ) : (
          <p className="dim">{t("hub.direct.unknown")}</p>
        )}
      </section>
    </div>
  );
}

/* ------------------------------------------------------------------ devices */

function DeviceRow({
  device,
  canManage,
  onChanged,
}: {
  device: HubDevice;
  canManage: boolean;
  onChanged: (selfRevoked: boolean) => void;
}) {
  const { t, lang } = useT();
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(device.name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const mine = device.current || canManage;

  const run = async (action: () => Promise<unknown>, selfRevoked = false) => {
    setBusy(true);
    setError(null);
    try {
      await action();
      setEditing(false);
      setConfirm(false);
      onChanged(selfRevoked);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <li className={`card hub-device${device.current ? " is-current" : ""}`}>
      <div className="hub-device-main">
        {editing ? (
          <form
            className="hub-rename"
            onSubmit={(e) => {
              e.preventDefault();
              void run(() => updateDevice(device.id, { name }));
            }}
          >
            <input
              className="input"
              value={name}
              maxLength={60}
              onChange={(e) => setName(e.target.value)}
              aria-label={t("hub.devices.name")}
            />
            <button type="submit" className="btn" disabled={busy || !name.trim()}>
              {t("hub.devices.save")}
            </button>
            <button type="button" className="btn" onClick={() => setEditing(false)}>
              {t("common.cancel")}
            </button>
          </form>
        ) : (
          <div className="hub-device-name">
            <b>{device.name}</b>
            {device.current && <span className="echip">{t("hub.devices.this")}</span>}
            {device.admin && <span className="echip is-accent">{t("hub.admin")}</span>}
          </div>
        )}
        <div className="dim hub-device-meta">
          {t(device.kind === "app" ? "hub.devices.kind.app" : "hub.devices.kind.browser")} ·{" "}
          {t("hub.devices.paired", { when: fmtRelativeDay(device.created_ms, lang) })} ·{" "}
          {t("hub.devices.used", { when: fmtRelativeDay(device.last_used_ms, lang) })}
        </div>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
      </div>
      {mine && !editing && (
        <div className="hub-device-actions">
          <button type="button" className="btn" onClick={() => setEditing(true)} disabled={busy}>
            {t("hub.devices.rename")}
          </button>
          {canManage && !device.current && (
            <button
              type="button"
              className="btn"
              disabled={busy}
              onClick={() => void run(() => updateDevice(device.id, { admin: !device.admin }))}
            >
              {device.admin ? t("hub.devices.unadmin") : t("hub.devices.makeAdmin")}
            </button>
          )}
          {confirm ? (
            <>
              <button
                type="button"
                className="btn is-danger"
                disabled={busy}
                onClick={() => void run(() => revokeDevice(device.id), device.current)}
              >
                {t("hub.devices.confirm")}
              </button>
              <button type="button" className="btn" onClick={() => setConfirm(false)}>
                {t("common.cancel")}
              </button>
            </>
          ) : (
            <button
              type="button"
              className="btn is-danger"
              disabled={busy}
              onClick={() => setConfirm(true)}
            >
              {device.current ? t("hub.devices.forget") : t("hub.devices.revoke")}
            </button>
          )}
        </div>
      )}
    </li>
  );
}

/** `#/devices` — the devices paired with the home server: rename, revoke, administrators. */
export function DevicesPage() {
  const { t } = useT();
  const list = useAsync((signal) => fetchDevices(signal), []);
  const admin = useHub((s) => s.device?.admin === true);
  const devices = list.data?.devices ?? [];
  return (
    <div className="home-page hub-devices">
      <PageHeader
        back={{ page: "home" }}
        title={t("hub.devices.title")}
        subtitle={t("hub.devices.subtitle")}
      />
      {list.error && (
        <Notice kind="error" title={t("hub.devices.error")}>
          {list.error}
        </Notice>
      )}
      <ul className="hub-device-list">
        {devices.map((d) => (
          <DeviceRow
            key={`${d.id}-${d.name}-${d.admin}`}
            device={d}
            canManage={admin}
            onChanged={(selfRevoked) => {
              if (selfRevoked) void useHub.getState().check();
              else list.reload();
            }}
          />
        ))}
      </ul>
      <section className="card home-card hub-add">
        <h2 className="home-card-title">{t("hub.devices.add")}</h2>
        <ol className="hub-steps">
          <li>
            {t("hub.devices.add1")} <code className="hub-cmd">sudo corde-server pair</code>
          </li>
          <li>{t("hub.devices.add2")}</li>
        </ol>
        <p className="dim">{t("hub.devices.lost")}</p>
        {!admin && <p className="faint">{t("hub.devices.notAdmin")}</p>}
      </section>
    </div>
  );
}
