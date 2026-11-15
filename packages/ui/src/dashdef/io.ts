import { BUILTIN_ID, type DashDefinition, type DashProblem, parseDefinition } from "./model";
import { freshId, slug } from "./ops";

/**
 * Sharing a dashboard as a file. The definition holds no device name, no address of a server,
 * no path; the author only when the driver typed one. Since 1.3 it travels in a `.cordedash`
 * archive with its pictures and videos (`archive.ts`); a `.cordedash.json` of 1.2 (the
 * definition alone) is still read. An imported file goes through the same validation as
 * everything else, and gets a new id when its own is taken (or is « corde »).
 */

export const FILE_SUFFIX = ".cordedash.json";

export function fileName(def: Pick<DashDefinition, "name">): string {
  return `${slug(def.name)}${FILE_SUFFIX}`;
}

/** The text of the file: exactly the fields of the definition, nothing else. */
export function exportText(def: DashDefinition): string {
  const out: DashDefinition = {
    v: def.v,
    id: def.id,
    name: def.name,
    ...(def.author ? { author: def.author } : {}),
    format: { ...def.format },
    theme: { ...def.theme },
    pages: def.pages.map((p) => ({
      id: p.id,
      name: p.name,
      cols: p.cols,
      rows: p.rows,
      ...(p.background ? { background: { ...p.background } } : {}),
      elements: p.elements.map((e) => ({
        id: e.id,
        type: e.type,
        x: e.x,
        y: e.y,
        w: e.w,
        h: e.h,
        props: e.props,
        ...(e.style ? { style: { ...e.style } } : {}),
        ...(e.show ? { show: { ...e.show } } : {}),
        ...(e.thresholds?.length ? { thresholds: e.thresholds.map((t) => ({ ...t })) } : {}),
      })),
    })),
    updated_ms: def.updated_ms,
  };
  return `${JSON.stringify(out, null, 2)}\n`;
}

export type Imported = { ok: true; def: DashDefinition } | { ok: false; problem: DashProblem };

/** Reads a file; the definition keeps its id unless it is taken. */
export function importText(text: string, taken: Iterable<string>): Imported {
  const checked = parseDefinition(text);
  if (!checked.ok) return checked;
  const used = new Set(taken);
  const def = checked.def;
  const id = def.id === BUILTIN_ID || used.has(def.id) ? freshId(def.name, used) : def.id;
  return { ok: true, def: { ...def, id, updated_ms: Date.now() } };
}
