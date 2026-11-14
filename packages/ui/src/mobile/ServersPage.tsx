import { type FormEvent, useState } from "react";
import { type TranslationKey, useT } from "../i18n";
import { codeDigits, formatCode } from "../net/hub-api";
import { appController, useApp } from "./app-state";
import { traitsOf } from "./device";
import type { ProbeResult, ServerEntry } from "./servers";

const ERR_KEYS: Record<string, TranslationKey> = {
  empty: "servers.err.empty",
  invalid: "servers.err.invalid",
  scheme: "servers.err.scheme",
  public_http: "servers.err.public_http",
  credentials: "servers.err.credentials",
  duplicate: "servers.err.duplicate",
  full: "servers.err.full",
};

function probeText(r: ProbeResult, t: ReturnType<typeof useT>["t"]): string {
  if (r.kind === "ok") {
    return t("servers.test.ok", { version: r.version, role: t(`mobile.role.${r.role}`) });
  }
  return t(`mobile.status.${r.kind}`) + ("message" in r && r.message ? ` — ${r.message}` : "");
}

function PairForm({ server }: { server: ServerEntry }) {
  const { t } = useT();
  const [code, setCode] = useState("");
  const [name, setName] = useState(() => traitsOf(useApp.getState().os).deviceName);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (codeDigits(code).length !== 8) {
      setMsg({ ok: false, text: t("servers.pair.invalid") });
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      const r = await appController().pair(server.id, code, name);
      if (r.ok) {
        setCode("");
        setMsg({ ok: true, text: t("servers.pair.ok", { name: r.deviceName }) });
      } else if (r.code === "rate_limited" && r.retryAfterS) {
        setMsg({ ok: false, text: t("servers.pair.wait", { n: r.retryAfterS }) });
      } else if (r.code === "unreachable") {
        setMsg({ ok: false, text: t("servers.pair.unreachable") });
      } else if (r.code === "invalid") {
        setMsg({ ok: false, text: t("servers.pair.invalid") });
      } else {
        setMsg({ ok: false, text: r.message ?? t("servers.pair.failed", { code: r.code }) });
      }
    } finally {
      setBusy(false);
    }
  };
  return (
    <form className="app-pair" onSubmit={(e) => void submit(e)}>
      <p className="app-pair-title">{t("servers.pair.title")}</p>
      <div className="field">
        <label htmlFor={`code-${server.id}`}>{t("servers.pair.code")}</label>
        <input
          id={`code-${server.id}`}
          className="input num app-code"
          inputMode="numeric"
          autoComplete="one-time-code"
          placeholder="1234 5678"
          value={formatCode(code)}
          onChange={(e) => setCode(e.target.value)}
        />
        <span className="hint">{t("servers.pair.hint")}</span>
      </div>
      <div className="field">
        <label htmlFor={`name-${server.id}`}>{t("servers.pair.name")}</label>
        <input
          id={`name-${server.id}`}
          className="input"
          maxLength={40}
          autoCapitalize="words"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <button type="submit" className="home-btn is-primary" disabled={busy}>
        {t("servers.pair.go")}
      </button>
      {msg && (
        <p className={msg.ok ? "app-ok" : "form-error"} role={msg.ok ? "status" : "alert"}>
          {msg.text}
        </p>
      )}
    </form>
  );
}

function ServerCard({ server }: { server: ServerEntry }) {
  const { t } = useT();
  const activeId = useApp((s) => s.activeId);
  const status = useApp((s) => s.status);
  const paired = useApp((s) => s.paired.includes(server.id));
  const active = activeId === server.id;
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);
  const run = async (what: string, fn: () => Promise<void>) => {
    setBusy(what);
    setNote(null);
    try {
      await fn();
    } finally {
      setBusy(null);
    }
  };
  const isHub = server.role === "hub" || (active && status === "unpaired");
  return (
    <li className={`card app-server${active ? " is-active" : ""}`}>
      <div className="app-server-head">
        <div className="app-server-id">
          {renaming !== null ? (
            <input
              className="input"
              value={renaming}
              maxLength={40}
              aria-label={t("servers.rename")}
              onChange={(e) => setRenaming(e.target.value)}
            />
          ) : (
            <b className="app-server-name">{server.name}</b>
          )}
          <span className="dim num">{server.label}</span>
        </div>
        <div className="app-server-tags">
          {server.role && <span className="app-tag">{t(`mobile.role.${server.role}`)}</span>}
          {active && <span className="app-tag is-active">{t("servers.active")}</span>}
        </div>
      </div>
      {active && (
        <p className="app-server-line">
          <span className={`app-dot is-${status}`} aria-hidden="true" />
          {t(`mobile.status.${status}`)}
        </p>
      )}
      {note && (
        <p className="dim app-note" role="status">
          {note}
        </p>
      )}
      <div className="app-server-actions">
        {!active && (
          <button
            type="button"
            className="home-btn is-primary"
            disabled={busy !== null}
            onClick={() => void run("use", () => appController().setActive(server.id))}
          >
            {t("servers.use")}
          </button>
        )}
        <button
          type="button"
          className="home-btn"
          disabled={busy !== null}
          onClick={() =>
            void run("test", async () => {
              const r = await appController().test(server.id);
              setNote(probeText(r, t));
            })
          }
        >
          {busy === "test" ? t("servers.testing") : t("servers.test")}
        </button>
        {renaming === null ? (
          <button type="button" className="home-btn" onClick={() => setRenaming(server.name)}>
            {t("servers.rename")}
          </button>
        ) : (
          <button
            type="button"
            className="home-btn"
            onClick={() =>
              void run("rename", async () => {
                await appController().rename(server.id, renaming);
                setRenaming(null);
              })
            }
          >
            {t("servers.save")}
          </button>
        )}
        {!confirm ? (
          <button type="button" className="home-btn" onClick={() => setConfirm(true)}>
            {t("servers.remove")}
          </button>
        ) : (
          <>
            <button
              type="button"
              className="home-btn is-danger"
              disabled={busy !== null}
              onClick={() => void run("remove", () => appController().remove(server.id))}
            >
              {t("servers.removeConfirm")}
            </button>
            <button type="button" className="home-btn" onClick={() => setConfirm(false)}>
              {t("servers.cancel")}
            </button>
          </>
        )}
      </div>
      {isHub && !paired && <PairForm server={server} />}
      {paired && (
        <p className="app-server-paired dim">
          {t("servers.paired")}{" "}
          <button
            type="button"
            className="link"
            disabled={busy !== null}
            onClick={() =>
              void run("unpair", async () => {
                const r = await appController().unpair(server.id);
                setNote(
                  t(r.revokedOnServer ? "servers.unpaired.revoked" : "servers.unpaired.local"),
                );
              })
            }
          >
            {t("servers.unpair")}
          </button>
        </p>
      )}
    </li>
  );
}

function AddForm() {
  const { t } = useT();
  const [address, setAddress] = useState("");
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const scan = appController().scan;
  const add = async (value: string) => {
    setBusy(true);
    setError(null);
    try {
      const r = await appController().add(value, name);
      if (r.ok) {
        setAddress("");
        setName("");
      } else setError(t(ERR_KEYS[r.error] ?? "servers.err.invalid"));
    } finally {
      setBusy(false);
    }
  };
  const onScan = async () => {
    if (!scan) return;
    const text = await scan();
    if (!text) {
      setError(t("servers.add.scanFailed"));
      return;
    }
    setAddress(text);
    await add(text);
  };
  return (
    <form
      className="card app-add"
      onSubmit={(e) => {
        e.preventDefault();
        void add(address);
      }}
    >
      <h2 className="home-card-title">{t("servers.add.title")}</h2>
      <div className="field">
        <label htmlFor="server-address">{t("servers.add.address")}</label>
        <input
          id="server-address"
          className="input num"
          inputMode="url"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
          placeholder="192.168.1.20:8471"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
        <span className="hint">{t("servers.add.addressHint")}</span>
      </div>
      <div className="field">
        <label htmlFor="server-name">{t("servers.add.name")}</label>
        <input
          id="server-name"
          className="input"
          maxLength={40}
          placeholder={t("servers.add.namePlaceholder")}
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      <div className="app-server-actions">
        <button type="submit" className="home-btn is-primary" disabled={busy}>
          {t("servers.add.go")}
        </button>
        {scan && (
          <button type="button" className="home-btn" disabled={busy} onClick={() => void onScan()}>
            {t("servers.add.scan")}
          </button>
        )}
      </div>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}

/** `#/servers` — « Mes serveurs ». */
export function ServersPage() {
  const { t } = useT();
  const servers = useApp((s) => s.servers);
  return (
    <div className="app-servers">
      <header className="page-head">
        <div className="page-head-row">
          <div className="page-head-main">
            <h1 className="page-title">{t("servers.title")}</h1>
            <p className="page-subtitle dim">{t("servers.subtitle")}</p>
          </div>
        </div>
      </header>
      {servers.length === 0 ? (
        <p className="dim">{t("servers.empty")}</p>
      ) : (
        <ul className="app-server-list">
          {servers.map((s) => (
            <ServerCard key={s.id} server={s} />
          ))}
        </ul>
      )}
      <AddForm />
      <p className="faint app-privacy">{t("servers.privacy")}</p>
    </div>
  );
}
