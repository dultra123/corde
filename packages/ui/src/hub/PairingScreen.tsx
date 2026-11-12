import { useState } from "react";
import { useT } from "../i18n";
import { codeDigits, formatCode, guessDeviceName, pairThisDevice } from "../net/hub-api";
import { errorMessage } from "../sessions/shared";
import { useHub } from "../store/hub";

/**
 * The home server's door (`corde-server --role hub`): shown instead of the app while this
 * device is not paired. The code is read on the server itself (`sudo corde-server pair`); once
 * accepted, the browser keeps a protected cookie and the app opens.
 */
export function PairingScreen() {
  const { t } = useT();
  const [code, setCode] = useState("");
  const [name, setName] = useState(() => guessDeviceName() ?? t("hub.pair.defaultName"));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ready = codeDigits(code).length === 8 && !busy;

  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      await pairThisDevice(codeDigits(code), name.trim());
      await useHub.getState().check();
    } catch (e) {
      setError(errorMessage(e));
      setCode("");
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="hub-door">
      <form
        className="card hub-door-card"
        onSubmit={(e) => {
          e.preventDefault();
          if (ready) void submit();
        }}
      >
        <svg className="hub-door-logo" viewBox="0 0 32 32" aria-hidden="true">
          <path
            d="M23.39 9.11A9 9 0 1 0 23.39 22.89"
            fill="none"
            stroke="currentColor"
            strokeWidth="5"
            strokeLinecap="round"
          />
          <circle cx="14.7" cy="16" r="2.8" fill="currentColor" />
        </svg>
        <h1 className="hub-door-title">{t("hub.pair.title")}</h1>
        <p className="dim">{t("hub.pair.lead")}</p>
        <ol className="hub-door-steps">
          <li>
            {t("hub.pair.step1")}
            <code className="hub-cmd">sudo corde-server pair</code>
          </li>
          <li>{t("hub.pair.step2")}</li>
        </ol>
        <label className="hub-field">
          <span>{t("hub.pair.code")}</span>
          <input
            className="input hub-code-input num"
            value={formatCode(code)}
            onChange={(e) => setCode(e.target.value)}
            inputMode="numeric"
            autoComplete="one-time-code"
            placeholder="1234 5678"
            aria-describedby="hub-pair-hint"
            maxLength={9}
            // biome-ignore lint/a11y/noAutofocus: the only thing to do on this screen
            autoFocus
          />
        </label>
        <label className="hub-field">
          <span>{t("hub.pair.name")}</span>
          <input
            className="input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            maxLength={60}
          />
        </label>
        <button type="submit" className="home-btn is-primary" disabled={!ready}>
          {busy ? t("hub.pair.busy") : t("hub.pair.submit")}
        </button>
        {error && (
          <p className="form-error" role="alert">
            {error}
          </p>
        )}
        <p id="hub-pair-hint" className="faint hub-door-hint">
          {t("hub.pair.hint")}
        </p>
      </form>
    </main>
  );
}
