import type { DashboardLayout, DeviceKind } from "@corde/protocol";
import { Fragment, useEffect, useRef, useState } from "react";
import { showDash, showWidgets, useLiveView } from "../dash/view";
import { useT } from "../i18n";
import { type Route, routeHash } from "../nav/routes";
import { activeSlot, useDashboard } from "../store/dashboard";
import { IconChevron } from "./icons";

const LIVE: Route = { page: "dashboard" };

/**
 * Layout switcher + management (new / rename / duplicate / delete) in a small popover. Its
 * « Tableaux de bord » group switches the Direct page to the race dashboard (`dash/`), a
 * layout switches it back to the widgets: one click each way, the same state as the setting
 * « Affichage du Direct » (`dash/view.ts`).
 */
export function LayoutMenu() {
  const { t } = useT();
  const layouts = useDashboard((s) => s.layouts);
  const device = useDashboard((s) => s.device);
  const slot = useDashboard(activeSlot);
  const currentId = useDashboard((s) => s.current[activeSlot(s)]);
  const sync = useDashboard((s) => s.sync);
  const { selectLayout, createLayout, renameLayout, duplicateLayout, deleteLayout, setEditing } =
    useDashboard.getState();
  const [open, setOpen] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState("");
  const rootRef = useRef<HTMLDivElement>(null);
  const current = layouts.find((l) => l.id === currentId);
  const onDash = useLiveView(LIVE) === "dash";

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

  const slotOf = (l: DashboardLayout): DeviceKind =>
    l.device === "mobile" || l.device === "overlay" ? l.device : "desktop";
  // On a desktop the screen layouts and the overlays (OBS) can be chosen; the phone keeps its own.
  const choosable: DeviceKind[] = device === "mobile" ? ["mobile"] : ["desktop", "overlay"];
  const others = layouts.filter((l) => !choosable.includes(slotOf(l)));

  const item = (l: DashboardLayout) => (
    <button
      key={l.id}
      type="button"
      role="menuitemradio"
      aria-checked={!onDash && l.id === currentId}
      className={`menu-item${!onDash && l.id === currentId ? " is-current" : ""}`}
      onClick={() => {
        selectLayout(l.id);
        if (onDash) showWidgets();
        setOpen(false);
      }}
    >
      {l.name}
      <small className="num dim">{l.widgets.length}</small>
    </button>
  );

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
        <span className="layoutmenu-name">{onDash ? t("dash.corde") : (current?.name ?? "—")}</span>
        <IconChevron />
      </button>
      {open && (
        <div className="menu" role="menu">
          {choosable.map((kind) => (
            <Fragment key={kind}>
              <div className="menu-section dim">
                {t(kind === "overlay" ? "layout.device.overlayMenu" : `layout.device.${kind}`)}
              </div>
              {layouts.filter((l) => slotOf(l) === kind).map(item)}
            </Fragment>
          ))}
          <div className="menu-section dim">{t("layout.device.dashes")}</div>
          <button
            type="button"
            role="menuitemradio"
            aria-checked={onDash}
            className={`menu-item${onDash ? " is-current" : ""}`}
            onClick={() => {
              setEditing(false);
              showDash();
              setOpen(false);
            }}
          >
            {t("dash.corde")}
            <small className="dim">{t("layout.dashPages", { n: 4 })}</small>
          </button>
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
                  if (onDash) showWidgets();
                  setEditing(true);
                  setOpen(false);
                }}
              >
                {t(slot === "overlay" ? "layout.newOverlay" : "layout.new")}
              </button>
              {device === "desktop" && slot !== "overlay" && (
                <button
                  type="button"
                  className="menu-item"
                  onClick={() => {
                    createLayout(t("layout.newOverlay"), "overlay");
                    if (onDash) showWidgets();
                    setEditing(true);
                    setOpen(false);
                  }}
                >
                  {t("layout.newOverlay")}
                </button>
              )}
              {!onDash && (
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
              )}
              {!onDash && (
                <>
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
            </>
          )}
          <div className="menu-sep" />
          <a className="menu-item" href={routeHash({ page: "obs" })} onClick={() => setOpen(false)}>
            {t("layout.obsLink")}
          </a>
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
