import { useEffect, useMemo, useRef, useState } from "react";
import { useT } from "../i18n";
import { fetchChannels } from "../net/sessions-api";
import { CATEGORY_ORDER, categoryKey, type PickableChannel, pickableChannels } from "./channels";
import { rememberUnits } from "./panels";
import { Notice, useAsync } from "./shared";

interface Props {
  open: boolean;
  onClose: () => void;
  onAdd: (channels: string[]) => void;
}

/** Modal list of every recorded channel, grouped by category, to build a custom panel. */
export function ChannelPicker({ open, onClose, onAdd }: Props) {
  const { t, lang } = useT();
  const ref = useRef<HTMLDialogElement>(null);
  const [query, setQuery] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const list = useAsync(async () => {
    const chans = await fetchChannels();
    rememberUnits(chans);
    return chans;
  }, []);

  useEffect(() => {
    const d = ref.current;
    if (!d) return;
    if (open && !d.open) {
      setQuery("");
      setPicked(new Set());
      d.showModal();
    } else if (!open && d.open) d.close();
  }, [open]);

  const entries = useMemo(
    () => (list.data ? pickableChannels(list.data, lang) : []),
    [list.data, lang],
  );
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return entries;
    return entries.filter((e) => e.label.toLowerCase().includes(q) || e.key.includes(q));
  }, [entries, query]);
  const groups = useMemo(() => {
    const by = new Map<string, PickableChannel[]>();
    for (const e of filtered) {
      const arr = by.get(e.category) ?? [];
      arr.push(e);
      by.set(e.category, arr);
    }
    return CATEGORY_ORDER.filter((c) => by.has(c)).map((c) => ({
      category: c,
      items: by.get(c) ?? [],
    }));
  }, [filtered]);

  const toggle = (key: string) =>
    setPicked((cur) => {
      const next = new Set(cur);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  const submit = () => {
    const channels = entries.filter((e) => picked.has(e.key)).flatMap((e) => e.channels);
    if (channels.length === 0) return;
    onAdd(channels);
    onClose();
  };

  const count = entries.filter((e) => picked.has(e.key)).reduce((a, e) => a + e.channels.length, 0);

  return (
    <dialog
      className="sheet is-wide picker"
      ref={ref}
      onClose={onClose}
      aria-label={t("lap.picker.title")}
    >
      <div className="sheet-head">
        <span>{t("lap.picker.title")}</span>
        <span className="dim picker-count num">
          {count > 0 ? t("lap.picker.selected", { n: count }) : ""}
        </span>
      </div>
      <div className="sheet-body">
        <input
          className="input picker-search"
          placeholder={t("lap.picker.search")}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck={false}
        />
        <div className="hint">{t("lap.picker.hint")}</div>
        <div className="picker-list">
          {list.error && (
            <Notice kind="error" title={t("common.serverError", { msg: list.error })} />
          )}
          {list.loading && !list.data && <Notice kind="loading" />}
          {list.data && groups.length === 0 && <Notice kind="empty">{t("lap.picker.none")}</Notice>}
          {groups.map((g) => (
            <fieldset key={g.category} className="picker-group">
              <legend>{t(categoryKey(g.category))}</legend>
              {g.items.map((e) => (
                <label key={e.key} className={`picker-item${picked.has(e.key) ? " is-on" : ""}`}>
                  <input
                    type="checkbox"
                    checked={picked.has(e.key)}
                    onChange={() => toggle(e.key)}
                  />
                  <span className="picker-label">{e.label}</span>
                  <span className="picker-meta faint num">
                    {e.perWheel ? "×4 " : ""}
                    {e.unit}
                  </span>
                </label>
              ))}
            </fieldset>
          ))}
        </div>
      </div>
      <div className="sheet-foot">
        <button type="button" className="btn" onClick={onClose}>
          {t("common.cancel")}
        </button>
        <button type="button" className="btn is-active" onClick={submit} disabled={count === 0}>
          {t("lap.picker.add")}
        </button>
      </div>
    </dialog>
  );
}
