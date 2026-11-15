import { useEffect, useRef, useState } from "react";
import { useThrottled } from "../dash/live";
import { flagsOf } from "../dash/model";
import { EMPTY_PAGE, type PageProps } from "../dash/parts";
import { exportDash, importFile } from "../dashdef/archive";
import { cordeDefinition, isBuiltin, TEMPLATE_IDS, type TemplateId } from "../dashdef/builtin";
import { MediaMotion } from "../dashdef/draw";
import { type DashDefinition, type DashFormat, MAX_DASHES, presetFormat } from "../dashdef/model";
import { duplicateDefinition, newDefinition, renameDefinition } from "../dashdef/ops";
import { DashPage } from "../dashdef/Renderer";
import { deleteDash, type SaveResult, saveDash, useDashes } from "../dashdef/store";
import { type TranslationKey, useT } from "../i18n";
import { navigate, routeHash } from "../nav/routes";
import type { TelemetryState } from "../store/telemetry";
import { FormatFields, formatWords } from "./Format";
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

const MEDIA_UPLOAD_ERR: Record<string, TranslationKey> = {
  type: "studio.media.err.type",
  "too-large": "studio.media.err.too-large",
  full: "studio.media.err.full",
  "pc-only": "studio.media.err.pc-only",
  offline: "studio.media.err.offline",
  refused: "studio.media.err.refused",
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

/** The format a new dashboard starts with: the one the models were drawn for. */
const NEW_FORMAT: DashFormat = presetFormat("phone-landscape");

/**
 * « Nouveau tableau de bord »: the model to start from and the format of the dashboard (ADR
 * 0018), chosen before anything is created. The format can be changed at any time afterwards
 * (the « Format » control of the editor).
 */
function NewDashDialog({
  open,
  onClose,
  onCreate,
}: {
  open: boolean;
  onClose: () => void;
  onCreate: (model: TemplateId, format: DashFormat) => void;
}) {
  const { t } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const [model, setModel] = useState<TemplateId>("blank");
  const [format, setFormat] = useState<DashFormat>(NEW_FORMAT);
  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      // Each opening starts from the same choice: nothing is left of the one before.
      setModel("blank");
      setFormat(NEW_FORMAT);
      d.showModal();
    } else if (!open && d.open) d.close();
  }, [open]);
  return (
    <dialog
      className="sheet studio-new"
      ref={ref}
      onClose={onClose}
      aria-label={t("studio.newTitle")}
    >
      {open && (
        <form
          onSubmit={(e) => {
            e.preventDefault();
            onCreate(model, format);
          }}
        >
          <div className="sheet-head">{t("studio.newTitle")}</div>
          <div className="sheet-body">
            <fieldset className="fmt-set studio-new-models">
              <legend className="ins-sub">{t("studio.newModel")}</legend>
              {TEMPLATE_IDS.map((tid) => (
                <label key={tid} className="studio-new-model">
                  <input
                    type="radio"
                    name="studio-new-model"
                    value={tid}
                    checked={model === tid}
                    onChange={() => setModel(tid)}
                  />
                  <span>{t(TEMPLATE_KEY[tid])}</span>
                </label>
              ))}
            </fieldset>
            <FormatFields format={format} onChange={setFormat} />
          </div>
          <div className="sheet-foot">
            <button type="button" className="btn" onClick={onClose}>
              {t("common.cancel")}
            </button>
            <button type="submit" className="btn is-primary">
              {`${t("studio.create")} · ${formatWords(format, t)}`}
            </button>
          </div>
        </form>
      )}
    </dialog>
  );
}

/** The first page of a dashboard, small, with the data of the moment. */
function Thumb({ def, data }: { def: DashDefinition; data: PageProps }) {
  const { lang } = useT();
  const page = def.pages[0];
  return (
    <div className="studio-thumb dash is-canvas" aria-hidden="true" inert>
      {/* A small preview: its videos show their first image only. */}
      <MediaMotion.Provider value={false}>
        {page && <DashPage def={def} page={page} data={data} lang={lang} />}
      </MediaMotion.Provider>
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
    if (f.size > 128 * 1024 * 1024) {
      setMessage({
        text: `${t("studio.importRefused")} ${t("studio.problem.too-large")}`,
        bad: true,
      });
      return;
    }
    say("studio.importing", false);
    const r = await importFile(new Uint8Array(await f.arrayBuffer()), taken);
    if (!r.ok) {
      setMessage({
        text:
          "problem" in r
            ? `${t("studio.importRefused")} ${t(problemKey(r.problem))}`
            : t(MEDIA_UPLOAD_ERR[r.upload] ?? "studio.media.err.refused"),
        bad: true,
      });
      return;
    }
    const saved = await saveDash(r.def);
    if (!saved.ok) {
      say(saveMessage(saved));
      return;
    }
    if (r.media > 0) say("studio.importedMedia", false, { name: r.def.name, n: r.media });
    else say("studio.imported", false, { name: r.def.name });
  };

  const onExport = async (d: DashDefinition) => {
    say("studio.exporting", false);
    const r = await exportDash(d);
    const done = t(r.hasVideo ? "studio.exportedVideo" : "studio.exported", { name: d.name });
    const missing = r.missing.length
      ? ` ${t("studio.exportMissing", { n: r.missing.length })}`
      : "";
    setMessage({ text: `${done}${missing}`, bad: r.missing.length > 0 });
  };

  return (
    <div className="studio">
      <header className="studio-bar">
        <h1 className="studio-title">
          {t("studio.title")} <span className="dim">· {t("studio.subtitle")}</span>
        </h1>
        <div className="studio-bar-right">
          <button
            type="button"
            className="btn is-primary"
            aria-haspopup="dialog"
            disabled={full}
            onClick={() => setNewOpen(true)}
          >
            {t("studio.new")}
          </button>
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
            accept=".cordedash,.json,application/zip,application/json"
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
                    {" · "}
                    <span className="num">{formatWords(d.format, t)}</span>
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
                    <button type="button" className="btn" onClick={() => void onExport(d)}>
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
      <NewDashDialog
        open={newOpen}
        onClose={() => setNewOpen(false)}
        onCreate={(model, format) => {
          setNewOpen(false);
          void store(newDefinition(model, taken, lang, format), "studio.created", true);
        }}
      />
    </div>
  );
}
