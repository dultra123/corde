import { useEffect, useRef, useState } from "react";
import { useT } from "../i18n";
import { useDashboard } from "../store/dashboard";
import { IconChevron } from "./icons";

/** Layout switcher + management (new / rename / duplicate / delete) in a small popover. */
export function LayoutMenu() {
  const { t } = useT();
  const layouts = useDashboard((s) => s.layouts);
  const device = useDashboard((s) => s.device);
  const currentId = useDashboard((s) => s.current[s.device]);
  const sync = useDashboard((s) => s.sync);
  const { selectLayout, createLayout, renameLayout, duplicateLayout, deleteLayout, setEditing } =
    useDashboard.getState();
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState("");
  const rootRef = useRef<HTMLDivElement>(null);
  const current = layouts.find((l) => l.id === currentId);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: PointerEvent) => {
      if (rootRef.current && !rootRef.current.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("pointerdown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const mine = layouts.filter((l) => l.device === device);
  const others = layouts.filter((l) => l.device !== device);

  const submitRename = () => {
    if (current) renameLayout(current.id, name);
    setRenaming(false);
  };

  return (
    <div className="layoutmenu" ref={rootRef}>
      <button
        type="button"
        className="layoutmenu-btn"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        aria-haspopup="menu"
      >
        <span className="layoutmenu-name">{current?.name ?? "—"}</span>
        <IconChevron />
      </button>
      {open && (
        <div className="menu" role="menu">
          <div className="menu-section dim">{t(`layout.device.${device}`)}</div>
          {mine.map((l) => (
            <button
              key={l.id}
              type="button"
              role="menuitemradio"
              aria-checked={l.id === currentId}
              className={`menu-item${l.id === currentId ? " is-current" : ""}`}
              onClick={() => {
                selectLayout(l.id);
                setOpen(false);
              }}
            >
              {l.name}
              <small className="num dim">{l.widgets.length}</small>
            </button>
          ))}
          {others.length > 0 && (
            <>
              <div className="menu-section dim">
                {t(`layout.device.${device === "mobile" ? "desktop" : "mobile"}`)}
              </div>
              {others.map((l) => (
                <div key={l.id} className="menu-item is-disabled">
                  {l.name}
                  <small className="num dim">{l.widgets.length}</small>
                </div>
              ))}
            </>
          )}
          <div className="menu-sep" />
          {renaming ? (
            <form
              className="menu-form"
              onSubmit={(e) => {
                e.preventDefault();
                submitRename();
              }}
            >
              <input
                className="input"
                value={name}
                onChange={(e) => setName(e.target.value)}
                aria-label={t("layout.name")}
                // biome-ignore lint/a11y/noAutofocus: the field appears on an explicit user action
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
                className="menu-item"
                onClick={() => {
                  createLayout(t("layout.new"));
                  setEditing(true);
                  setOpen(false);
                }}
              >
                {t("layout.new")}
              </button>
              <button
                type="button"
                className="menu-item"
                onClick={() => {
                  setName(current?.name ?? "");
                  setRenaming(true);
                }}
              >
                {t("layout.rename")}
              </button>
              <button
                type="button"
                className="menu-item"
                onClick={() => {
                  if (current) duplicateLayout(current.id);
                  setOpen(false);
                }}
              >
                {t("layout.duplicate")}
              </button>
              <button
                type="button"
                className="menu-item is-danger"
                onClick={() => {
                  if (current && window.confirm(t("layout.deleteConfirm")))
                    deleteLayout(current.id);
                  setOpen(false);
                }}
              >
                {t("layout.delete")}
              </button>
            </>
          )}
          <div className="menu-sep" />
          <div className="menu-section faint">
            {sync === "synced"
              ? t("layout.synced")
              : sync === "syncing"
                ? t("layout.syncing")
                : t("layout.localOnly")}
          </div>
        </div>
      )}
    </div>
  );
}
