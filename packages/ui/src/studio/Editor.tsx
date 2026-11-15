import { useEffect, useState } from "react";
import { useThrottled } from "../dash/live";
import { flagsOf } from "../dash/model";
import type { PageProps } from "../dash/parts";
import { exportDash } from "../dashdef/archive";
import { isBuiltin } from "../dashdef/builtin";
import { aspectOf, shapeOf } from "../dashdef/model";
import {
  addPage,
  movePage,
  pasteElements,
  removePage,
  renameDefinition,
  renamePage,
} from "../dashdef/ops";
import { DashPage } from "../dashdef/Renderer";
import { saveDash, useDashes } from "../dashdef/store";
import { type TranslationKey, useT } from "../i18n";
import { navigate, routeHash } from "../nav/routes";
import { connection } from "../net/connection";
import { connectionPhase, type TelemetryState, useTelemetry } from "../store/telemetry";
import { Canvas } from "./Canvas";
import { copiedElements, copyElements, isDirty, readDraft, useEditor } from "./editor-store";
import { FormatControl } from "./Format";
import { Inspector, Palette } from "./Inspector";
import { saveMessage } from "./Library";

const IDLE: Record<string, TranslationKey | null> = {
  connecting: "status.connecting",
  offline: "status.offline",
  "waiting-game": "status.waitingGame",
  monitor: "status.monitor",
};

function liveData(s: TelemetryState): PageProps & { idleKey: TranslationKey | null } {
  const player = s.frame?.player ?? null;
  const idleKey = player ? null : (IDLE[connectionPhase(s)] ?? null);
  return {
    frame: s.frame,
    laps: s.laps,
    standings: s.frame?.standings ?? [],
    flags: flagsOf(s.frame?.session ?? null, player, s.frame?.standings ?? []),
    active: true,
    idle: null,
    idleKey,
  };
}

/** The whole dashboard over the window, the pages with ← → or the dots; Esc to leave. */
function Preview({ onClose }: { onClose: () => void }) {
  const { t, lang } = useT();
  const def = useEditor((s) => s.h?.present);
  const [index, setIndex] = useState(() => {
    const { h, pageId } = useEditor.getState();
    return Math.max(0, h?.present.pages.findIndex((p) => p.id === pageId) ?? 0);
  });
  const data = useThrottled(liveData);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
      else if (e.key === "ArrowRight") setIndex((i) => i + 1);
      else if (e.key === "ArrowLeft") setIndex((i) => Math.max(0, i - 1));
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  if (!def) return null;
  const i = Math.min(index, def.pages.length - 1);
  const page = def.pages[i];
  // As on the screen it is made for: a format of its own takes the whole window, a shape has
  // black around it (`styles/dash.css`).
  const look =
    (aspectOf(def) !== null ? " is-format" : "") +
    (shapeOf(def.format) !== "rect" ? " is-shaped" : "");
  return (
    <div className={`dash studio-preview${look}`} role="dialog" aria-label={t("studio.preview")}>
      <div className="dash-pages">
        <section className="dash-page">
          {page && <DashPage def={def} page={page} data={data} lang={lang} />}
        </section>
      </div>
      <button
        type="button"
        className="dash-menu studio-preview-close"
        onClick={onClose}
        aria-label={t("studio.closePreview")}
      >
        ✕
      </button>
      <div className="dash-foot">
        <div className="dash-dots" role="tablist" aria-label={t("dash.pages")}>
          {def.pages.map((p, j) => (
            <button
              key={p.id}
              type="button"
              role="tab"
              aria-selected={j === i}
              aria-label={p.name}
              className={j === i ? "is-on" : undefined}
              onClick={() => setIndex(j)}
            >
              <i />
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

function PageTabs() {
  const { t } = useT();
  const def = useEditor((s) => s.h?.present);
  const pageId = useEditor((s) => s.pageId);
  const { apply, setPage } = useEditor.getState();
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState("");
  if (!def) return null;
  const page = def.pages.find((p) => p.id === pageId);
  return (
    <div className="studio-tabs" role="tablist" aria-label={t("studio.pagesLabel")}>
      {def.pages.map((p) => (
        <button
          key={p.id}
          type="button"
          role="tab"
          aria-selected={p.id === pageId}
          className={`studio-tab${p.id === pageId ? " is-on" : ""}`}
          onClick={() => setPage(p.id)}
        >
          {p.name}
          <small className="num dim">{p.elements.length}</small>
        </button>
      ))}
      <button
        type="button"
        className="studio-tab is-add"
        disabled={def.pages.length >= 12}
        onClick={() =>
          apply((d) => {
            const next = addPage(d, `${t("studio.pageN")} ${d.pages.length + 1}`);
            const last = next.pages[next.pages.length - 1];
            if (last && next !== d) queueMicrotask(() => setPage(last.id));
            return next;
          })
        }
      >
        + {t("studio.addPage")}
      </button>
      {page && (
        <div className="studio-tabs-tools">
          {renaming ? (
            <form
              className="menu-form"
              onSubmit={(e) => {
                e.preventDefault();
                apply((d) => renamePage(d, page.id, name.replace(/[<>]/g, "")));
                setRenaming(false);
              }}
            >
              <input
                className="input"
                value={name}
                maxLength={40}
                aria-label={t("studio.pageName")}
                onChange={(e) => setName(e.target.value)}
                // biome-ignore lint/a11y/noAutofocus: shown on an explicit click
                autoFocus
              />
              <button type="submit" className="btn">
                {t("settings.save")}
              </button>
            </form>
          ) : (
            <>
              <button
                type="button"
                className="btn"
                onClick={() => {
                  setName(page.name);
                  setRenaming(true);
                }}
              >
                {t("studio.renamePage")}
              </button>
              <button
                type="button"
                className="btn is-icon"
                aria-label={t("studio.pageLeft")}
                title={t("studio.pageLeft")}
                onClick={() => apply((d) => movePage(d, page.id, -1))}
              >
                ←
              </button>
              <button
                type="button"
                className="btn is-icon"
                aria-label={t("studio.pageRight")}
                title={t("studio.pageRight")}
                onClick={() => apply((d) => movePage(d, page.id, 1))}
              >
                →
              </button>
              <button
                type="button"
                className="btn is-danger"
                disabled={def.pages.length <= 1}
                onClick={() => {
                  if (window.confirm(t("studio.removePageConfirm", { name: page.name })))
                    apply((d) => removePage(d, page.id));
                }}
              >
                {t("studio.removePage")}
              </button>
            </>
          )}
        </div>
      )}
    </div>
  );
}

export function DashEditor({ id }: { id: string }) {
  const { t } = useT();
  const defs = useDashes((s) => s.defs);
  const sync = useDashes((s) => s.sync);
  const h = useEditor((s) => s.h);
  const pageId = useEditor((s) => s.pageId);
  const dirty = useEditor(isDirty);
  const pastLen = useEditor((s) => s.h?.past.length ?? 0);
  const futureLen = useEditor((s) => s.h?.future.length ?? 0);
  const demo = useTelemetry((s) => s.demo);
  const data = useThrottled(liveData);
  const [message, setMessage] = useState<{ text: string; bad: boolean } | null>(null);
  const [draftShown, setDraftShown] = useState(false);
  const [preview, setPreview] = useState(false);
  const [formatOpen, setFormatOpen] = useState(false);
  const saved = defs.find((d) => d.id === id) ?? null;
  const loaded = h?.present.id === id;

  // Open the dashboard (or its draft, when one was left unsaved).
  useEffect(() => {
    if (loaded) return;
    const draft = readDraft(id);
    const base = draft ?? saved;
    if (!base) return;
    useEditor.getState().open(base, saved);
    setDraftShown(draft !== null && JSON.stringify(draft) !== JSON.stringify(saved));
  }, [id, saved, loaded]);

  useEffect(() => () => useEditor.getState().close(), []);

  const onExport = async () => {
    const cur = useEditor.getState().h?.present;
    if (!cur) return;
    setMessage({ text: t("studio.exporting"), bad: false });
    const r = await exportDash(cur);
    const done = t(r.hasVideo ? "studio.exportedVideo" : "studio.exported", { name: cur.name });
    const missing = r.missing.length
      ? ` ${t("studio.exportMissing", { n: r.missing.length })}`
      : "";
    setMessage({ text: `${done}${missing}`, bad: r.missing.length > 0 });
  };

  const save = async () => {
    const cur = useEditor.getState().h?.present;
    if (!cur) return;
    const r = await saveDash(cur);
    if (r.ok) {
      useEditor.getState().markSaved(cur);
      setDraftShown(false);
      setMessage({ text: t("studio.savedHere"), bad: false });
    } else {
      const key = saveMessage(r);
      setMessage({ text: key ? t(key) : "", bad: true });
    }
  };

  // Ctrl+Z / Ctrl+Y (Ctrl+Maj+Z) / Ctrl+S / Ctrl+C / Ctrl+V / Ctrl+A, outside the text fields.
  // biome-ignore lint/correctness/useExhaustiveDependencies: `save` reads the store
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey) || preview) return;
      const k = e.key.toLowerCase();
      if (k === "s") {
        e.preventDefault();
        void save();
        return;
      }
      if ((e.target as HTMLElement | null)?.closest?.("input, textarea, select")) return;
      const st = useEditor.getState();
      const page = st.h?.present.pages.find((p) => p.id === st.pageId);
      if (k === "c" && page && st.selection.length > 0) {
        e.preventDefault();
        copyElements(page.elements.filter((x) => st.selection.includes(x.id)));
        return;
      }
      if (k === "v" && page) {
        const copies = copiedElements();
        if (copies.length === 0) return;
        e.preventDefault();
        let ids: string[] = [];
        st.apply((d) => {
          const r = pasteElements(d, page.id, copies);
          ids = r.ids;
          return r.def;
        });
        if (ids.length) useEditor.getState().selectMany(ids);
        return;
      }
      if (k === "a" && page) {
        e.preventDefault();
        st.selectMany(page.elements.map((x) => x.id));
        return;
      }
      if (k === "z" && !e.shiftKey) {
        e.preventDefault();
        useEditor.getState().undo();
      } else if (k === "y" || (k === "z" && e.shiftKey)) {
        e.preventDefault();
        useEditor.getState().redo();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [preview]);

  if (isBuiltin({ id })) {
    return (
      <div className="studio">
        <p className="studio-msg">{t("studio.builtinReadOnly")}</p>
        <a className="btn" href={routeHash({ page: "studio" })}>
          {t("studio.backToLibrary")}
        </a>
      </div>
    );
  }
  if (!h || !loaded) {
    return (
      <div className="studio">
        <p className="studio-msg">
          {sync === "ready" || sync === "offline" ? t("studio.notFound") : t("studio.loading")}
        </p>
        <a className="btn" href={routeHash({ page: "studio" })}>
          {t("studio.backToLibrary")}
        </a>
      </div>
    );
  }
  const def = h.present;
  const page = def.pages.find((p) => p.id === pageId) ?? def.pages[0];
  if (!page) return null;
  const demoManual = demo === "manual";

  return (
    <div className="studio is-editor">
      <header className="studio-bar">
        <button
          type="button"
          className="btn"
          onClick={() => navigate({ page: "studio" })}
          title={dirty ? t("studio.leaveDirty") : undefined}
        >
          ← {t("studio.library")}
        </button>
        <input
          className="input studio-name"
          value={def.name}
          maxLength={60}
          aria-label={t("studio.name")}
          onChange={(e) => {
            const v = e.target.value.replace(/[<>]/g, "");
            if (v.trim()) useEditor.getState().apply((d) => renameDefinition(d, v));
          }}
        />
        <FormatControl def={def} open={formatOpen} onOpenChange={setFormatOpen} />
        <div className="studio-bar-right">
          <button
            type="button"
            className="btn"
            disabled={pastLen === 0}
            onClick={() => useEditor.getState().undo()}
            title="Ctrl+Z"
          >
            {t("studio.undo")}
          </button>
          <button
            type="button"
            className="btn"
            disabled={futureLen === 0}
            onClick={() => useEditor.getState().redo()}
            title="Ctrl+Y"
          >
            {t("studio.redo")}
          </button>
          <button
            type="button"
            className={`btn${demoManual ? " is-active" : ""}`}
            aria-pressed={demoManual}
            onClick={() => connection.setDemo(!demoManual)}
          >
            {t("studio.demo")}
          </button>
          <button type="button" className="btn" onClick={() => setPreview(true)}>
            {t("studio.preview")}
          </button>
          <button type="button" className="btn" onClick={() => void onExport()}>
            {t("studio.export")}
          </button>
          <button
            type="button"
            className={`btn${dirty ? " is-primary" : ""}`}
            onClick={() => void save()}
            title="Ctrl+S"
          >
            {dirty ? t("studio.save") : t("studio.savedState")}
          </button>
        </div>
      </header>
      {(draftShown || message || data.idleKey) && (
        <div className="studio-notes">
          {draftShown && (
            <p className="studio-msg">
              {t("studio.draftRestored")}{" "}
              {useEditor.getState().saved && (
                <button
                  type="button"
                  className="btn is-small"
                  onClick={() => {
                    if (!window.confirm(t("studio.discardConfirm"))) return;
                    useEditor.getState().discardDraft();
                    setDraftShown(false);
                    setMessage({ text: t("studio.draftDiscarded"), bad: false });
                  }}
                >
                  {t("studio.discardDraft")}
                </button>
              )}
            </p>
          )}
          <p className={`studio-msg${message?.bad ? " is-bad" : ""}`} role="status">
            {message?.text ?? ""}
          </p>
          {data.idleKey && !demoManual && (
            <p className="studio-msg dim">
              {t(data.idleKey)} — {t("studio.demoHint")}
            </p>
          )}
        </div>
      )}
      <PageTabs />
      <div className="studio-edit">
        <Palette page={page} />
        <Canvas def={def} page={page} data={data} />
        <Inspector def={def} page={page} onFormat={() => setFormatOpen(true)} />
      </div>
      {preview && <Preview onClose={() => setPreview(false)} />}
    </div>
  );
}
