import { useEffect, useState } from "react";
import { linkStateKey, linkTone, pendingCount } from "../hub/lib";
import { useT } from "../i18n";
import { isDesktop, isLocalPage } from "../lib/desktop";
import { fmtRelativeDay } from "../lib/format";
import { routeHash } from "../nav/routes";
import {
  codeDigits,
  fetchHubLink,
  formatCode,
  type HubLinkStatus,
  pairHubLink,
  resendHubLink,
  revokeDevice,
  syncHubLink,
  unpairHubLink,
} from "../net/hub-api";
import { errorMessage } from "../sessions/shared";
import { useHub } from "../store/hub";

const POLL_MS = 2000;

/**
 * « Serveur Corde » in the PC's settings (from this PC only, like every setting): the home
 * server's address, pairing with a code read on the server, the state of the sending, « Tout
 * renvoyer » and « Dissocier ». The device token stays on the server side of the API: this
 * section never sees it.
 */
export function HubLinkSection({ open }: { open: boolean }) {
  const { t, lang } = useT();
  const role = useHub((s) => s.role);
  const onThisPc = role !== "hub" && (isDesktop() || isLocalPage());
  const [link, setLink] = useState<HubLinkStatus | null>(null);
  const [url, setUrl] = useState("");
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirmUnpair, setConfirmUnpair] = useState(false);

  useEffect(() => {
    if (!open || !onThisPc) return;
    let stop = false;
    const load = () =>
      fetchHubLink()
        .then((l) => {
          if (stop) return;
          setLink(l);
          setUrl((u) => u || (l.url ?? ""));
        })
        .catch(() => {
          // a server without the route (older) or no answer: the section stays as it is
        });
    void load();
    const timer = setInterval(load, POLL_MS);
    return () => {
      stop = true;
      clearInterval(timer);
    };
  }, [open, onThisPc]);

  if (!onThisPc) return null;

  const act = async (fn: () => Promise<HubLinkStatus | unknown>) => {
    setBusy(true);
    setError(null);
    try {
      const r = await fn();
      if (r && typeof r === "object" && "state" in r) setLink(r as HubLinkStatus);
      else setLink(await fetchHubLink());
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  const paired = link?.paired === true;
  const pending = link ? pendingCount(link) : null;
  return (
    <>
      <div className="sheet-section">{t("hub.link.title")}</div>
      <div className="field hub-link">
        <span className="hint">{t("hub.link.lead")}</span>
        {paired && link ? (
          <>
            <div className="hub-link-state">
              <span className={`hub-dot is-${linkTone(link.state)}`} aria-hidden="true" />
              <b>{t(linkStateKey(link.state))}</b>
              <span className="num dim">{link.url}</span>
            </div>
            <dl className="hub-link-facts">
              <dt>{t("hub.link.lastSync")}</dt>
              <dd>{link.last_sync_ms ? fmtRelativeDay(link.last_sync_ms, lang) : "—"}</dd>
              <dt>{t("hub.link.pending")}</dt>
              <dd className="num">{pending === null ? "—" : pending}</dd>
              {link.server_version && (
                <>
                  <dt>{t("hub.link.version")}</dt>
                  <dd className="num">{link.server_version}</dd>
                </>
              )}
            </dl>
            {link.message && <span className="hint is-error">{link.message}</span>}
            <div className="hub-link-actions">
              <button
                type="button"
                className="btn"
                disabled={busy}
                onClick={() => void act(syncHubLink)}
              >
                {t("hub.link.syncNow")}
              </button>
              <button
                type="button"
                className="btn"
                disabled={busy}
                onClick={() => void act(resendHubLink)}
                title={t("hub.link.resendHint")}
              >
                {t("hub.link.resend")}
              </button>
              {confirmUnpair ? (
                <>
                  <button
                    type="button"
                    className="btn is-danger"
                    disabled={busy}
                    onClick={() => {
                      setConfirmUnpair(false);
                      void act(unpairHubLink);
                    }}
                  >
                    {t("hub.link.unpairConfirm")}
                  </button>
                  <button type="button" className="btn" onClick={() => setConfirmUnpair(false)}>
                    {t("common.cancel")}
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  className="btn is-danger"
                  disabled={busy}
                  onClick={() => setConfirmUnpair(true)}
                >
                  {t("hub.link.unpair")}
                </button>
              )}
            </div>
          </>
        ) : (
          <div className="hub-link-pair">
            <label htmlFor="hub-url">{t("hub.link.url")}</label>
            <input
              id="hub-url"
              className="input num"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              placeholder="192.168.1.50"
              inputMode="url"
              autoCapitalize="off"
              autoCorrect="off"
              spellCheck={false}
            />
            <label htmlFor="hub-code">{t("hub.link.code")}</label>
            <div className="hub-link-row">
              <input
                id="hub-code"
                className="input num hub-code-input"
                value={formatCode(code)}
                onChange={(e) => setCode(e.target.value)}
                inputMode="numeric"
                placeholder="1234 5678"
                maxLength={9}
              />
              <button
                type="button"
                className="btn is-active"
                disabled={busy || !url.trim() || codeDigits(code).length !== 8}
                onClick={() =>
                  void act(async () => {
                    const r = await pairHubLink(url.trim(), codeDigits(code));
                    setCode("");
                    return r;
                  })
                }
              >
                {busy ? t("hub.pair.busy") : t("hub.link.pair")}
              </button>
            </div>
            <span className="hint">{t("hub.link.codeHint")}</span>
          </div>
        )}
        {error && (
          <span className="hint is-error" role="alert">
            {error}
          </span>
        )}
      </div>
    </>
  );
}

/** On the home server: this device, the devices page, unpairing this device. */
export function HubDeviceSection() {
  const { t } = useT();
  const role = useHub((s) => s.role);
  const device = useHub((s) => s.device);
  const [error, setError] = useState<string | null>(null);
  if (role !== "hub" || !device) return null;
  return (
    <>
      <div className="sheet-section">{t("hub.link.title")}</div>
      <div className="field">
        <span className="hint">
          {t("hub.home.you", { name: device.name })}
          {device.admin ? ` · ${t("hub.admin")}` : ""}
        </span>
        <div className="sheet-app-row">
          <a className="link" href={routeHash({ page: "devices" })}>
            {t("hub.home.devicesLink")}
          </a>
          <button
            type="button"
            className="btn is-danger"
            onClick={() => {
              revokeDevice(device.id)
                .then(() => useHub.getState().check())
                .catch((e: unknown) => setError(errorMessage(e)));
            }}
          >
            {t("hub.devices.forget")}
          </button>
        </div>
        {error && <span className="hint is-error">{error}</span>}
      </div>
    </>
  );
}
