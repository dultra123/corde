import { useRef, useState } from "react";
import { useThrottled } from "../dash/live";
import { flagsOf } from "../dash/model";
import { EMPTY_PAGE, type PageProps } from "../dash/parts";
import { cordeDefinition, isBuiltin, TEMPLATE_IDS, type TemplateId } from "../dashdef/builtin";
import { downloadFile, importText } from "../dashdef/io";
import { type DashDefinition, MAX_DASHES } from "../dashdef/model";
import { duplicateDefinition, newDefinition, renameDefinition } from "../dashdef/ops";
import { DashPage } from "../dashdef/Renderer";
import { deleteDash, type SaveResult, saveDash, useDashes } from "../dashdef/store";
import { type TranslationKey, useT } from "../i18n";
import { navigate, routeHash } from "../nav/routes";
import type { TelemetryState } from "../store/telemetry";
import { problemKey } from "./messages";

function previewData(s: TelemetryState): PageProps {
  const player = s.frame?.player ?? null;
  return {
    ...EMPTY_PAGE,
    frame: s.frame,
    laps: s.laps,
    standings: s.frame?.standings ?? [],
    flags: flagsOf(s.frame?.session ?? null, player, s.frame?.standings ?? []),
    active: true,
  };
}

const TEMPLATE_KEY: Record<TemplateId, TranslationKey> = {
  blank: "studio.template.blank",
  "corde-blocks": "studio.template.cordeBlocks",
  endurance: "studio.template.endurance",
};

export function saveMessage(r: SaveResult): TranslationKey | null {
  if (r.ok) return null;
  return r.reason === "offline"
    ? "studio.err.offline"
    : r.reason === "full"
      ? "studio.err.full"
      : r.reason === "pc-only"
        ? "studio.err.pcOnly"
        : "studio.err.refused";
}

/** The first page of a dashboard, small, with the data of the moment. */
function Thumb({ def, data }: { def: DashDefinition; data: PageProps }) {
  const { lang } = useT();
  const page = def.pages[0];
  return (
    <div className="studio-thumb dash is-canvas" aria-hidden="true" inert>
      {page && <DashPage def={def} page={page} data={data} lang={lang} />}
    </div>
  );
}

export function DashLibrary() {
  const { t, lang, nOf } = useT();
  const defs = useDashes((s) => s.defs);
  const selected = useDashes((s) => s.selected);
  const sync = useDashes((s) => s.sync);
  const data = useThrottled(previewData, 1);
  const [message, setMessage] = useState<{ text: string; bad: boolean } | null>(null);
  const [renaming, setRenaming] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [newOpen, setNewOpen] = useState(false);
  const file = useRef<HTMLInputElement>(null);
  const corde = cordeDefinition(lang);
  const all = [corde, ...defs];
  const taken = defs.map((d) => d.id);
  const full = defs.length >= MAX_DASHES;

  const say = (key: TranslationKey | null, bad = true, params?: Record<string, string | number>) =>
    setMessage(key ? { text: t(key, params), bad } : null);

  const store = async (def: DashDefinition, okKey: TranslationKey, open = false) => {
    const r = await saveDash(def);
    if (!r.ok) {
      say(saveMessage(r));
      return;
    }
    say(okKey, false, { name: def.name });
    if (open) navigate({ page: "studio", id: def.id });
  };

  const onImport = async (f: File | undefined) => {
    if (!f) return;
    if (f.size > 1024 * 1024) {
      say("studio.problem.too-large");
      return;
    }
    const text = await f.text();
    const r = importText(text, taken);
    if (!r.ok) {
      setMessage({
        text: `${t("studio.importRefused")} ${t(problemKey(r.problem))}`,
        bad: true,
      });
      return;
    }
    await store(r.def, "studio.imported");
  };

  return (
    <div className="studio">
      <header className="studio-bar">
        <a className="btn" href={routeHash({ page: "dashboard" })}>
          ← {t("studio.backToLive")}
        </a>
        <h1 className="studio-title">
          {t("studio.title")} <span className="dim">· {t("studio.subtitle")}</span>
        </h1>
        <div className="studio-bar-right">
          <div className="layoutmenu">
            <button
              type="button"
              className="btn is-primary"
              aria-haspopup="menu"
              aria-expanded={newOpen}
              disabled={full}
              onClick={() => setNewOpen((o) => !o)}
            >
              {t("studio.new")}
            </button>
            {newOpen && (
              <div className="menu is-right" role="menu">
                {TEMPLATE_IDS.map((tid) => (
                  <button
                    key={tid}
                    type="button"
                    role="menuitem"
                    className="menu-item"
                    onClick={() => {
                      setNewOpen(false);
                      void store(newDefinition(tid, taken, lang), "studio.created", true);
                    }}
                  >
                    {t(TEMPLATE_KEY[tid])}
                  </button>
                ))}
              </div>
            )}
          </div>
          <button
            type="button"
            className="btn"
            disabled={full}
            onClick={() => file.current?.click()}
          >
            {t("studio.import")}
          </button>
          <input
            ref={file}
            type="file"
            accept=".json,application/json"
            hidden
            onChange={(e) => {
              void onImport(e.target.files?.[0]);
              e.target.value = "";
            }}
          />
        </div>
      </header>

      <main className="studio-lib">
        <p className="dim studio-intro">{t("studio.intro")}</p>
        {sync === "offline" && <p className="studio-msg is-bad">{t("studio.err.offline")}</p>}
        {full && <p className="studio-msg is-bad">{t("studio.err.full")}</p>}
        <p className={`studio-msg${message?.bad ? " is-bad" : ""}`} role="status">
          {message?.text ?? ""}
        </p>
        <ul className="studio-cards">
          {all.map((d) => {
            const builtin = isBuiltin(d);
            const used = d.id === selected || (builtin && !defs.some((x) => x.id === selected));
            return (
              <li key={d.id} className={`studio-card${used ? " is-used" : ""}`}>
                <Thumb def={d} data={data} />
                <div className="studio-card-body">
                  {renaming === d.id ? (
                    <form
                      className="menu-form"
                      onSubmit={(e) => {
                        e.preventDefault();
                        setRenaming(null);
                        const next = renameDefinition(d, name);
                        if (next !== d) void store(next, "studio.saved");
                      }}
                    >
                      <input
                        className="input"
                        value={name}
                        maxLength={60}
                        aria-label={t("studio.name")}
                        onChange={(e) => setName(e.target.value)}
                        // biome-ignore lint/a11y/noAutofocus: shown on an explicit click
                        autoFocus
                      />
                      <button type="submit" className="btn">
                        {t("settings.save")}
                      </button>
                    </form>
                  ) : (
                    <h2 className="studio-card-name">{d.name}</h2>
                  )}
                  <p className="dim studio-card-meta">
                    {builtin
                      ? t("studio.builtin")
                      : d.author
                        ? `${t("studio.by")} ${d.author} · `
                        : ""}
                    {builtin ? " · " : ""}
                    {nOf(d.pages.length, "studio.page", "studio.pages")}
                    {used && <span className="studio-badge">{t("studio.usedHere")}</span>}
                  </p>
                  <div className="studio-card-actions">
                    {!builtin && (
                      <a className="btn is-primary" href={routeHash({ page: "studio", id: d.id })}>
                        {t("studio.edit")}
                      </a>
                    )}
                    {!used && (
                      <button
                        type="button"
                        className="btn"
                        onClick={() => {
                          useDashes.getState().select(d.id);
                          say("studio.nowUsed", false, { name: d.name });
                        }}
                      >
                        {t("studio.use")}
                      </button>
                    )}
                    <button
                      type="button"
                      className="btn"
                      disabled={full}
                      onClick={() =>
                        void store(duplicateDefinition(d, taken, lang), "studio.duplicated", true)
                      }
                    >
                      {builtin ? t("studio.duplicateToEdit") : t("studio.duplicate")}
                    </button>
                    {!builtin && (
                      <button
                        type="button"
                        className="btn"
                        onClick={() => {
                          setName(d.name);
                          setRenaming(d.id);
                        }}
                      >
                        {t("studio.rename")}
                      </button>
                    )}
                    <button type="button" className="btn" onClick={() => downloadFile(d)}>
                      {t("studio.export")}
                    </button>
                    {!builtin && (
                      <button
                        type="button"
                        className="btn is-danger"
                        onClick={async () => {
                          if (!window.confirm(t("studio.deleteConfirm", { name: d.name }))) return;
                          const r = await deleteDash(d.id);
                          say(r.ok ? "studio.deleted" : saveMessage(r), !r.ok, { name: d.name });
                        }}
                      >
                        {t("studio.delete")}
                      </button>
                    )}
                  </div>
                </div>
              </li>
            );
          })}
        </ul>
      </main>
    </div>
  );
}
