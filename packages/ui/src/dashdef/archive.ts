import { strFromU8, strToU8, Unzip, UnzipInflate, UnzipPassThrough, zipSync } from "fflate";
import { exportText, type Imported, importText } from "./io";
import {
  fetchMedia,
  kindOfExt,
  type MediaExt,
  maxBytes,
  type Prepared,
  prepareMedia,
  sniff,
  type UploadResult,
  uploadMedia,
} from "./media";
import {
  BUILTIN_ID,
  type DashDefinition,
  type DashProblem,
  isAssetKey,
  MAX_DASH_BYTES,
  parseDefinition,
  referencedAssets,
  validAssetId,
} from "./model";
import { freshId, slug } from "./ops";

/**
 * Sharing a dashboard with its pictures and videos (ADR 0016): a `.cordedash` file is a ZIP
 * archive, made and read in the browser, holding `dashboard.json` (the definition, as the
 * `.cordedash.json` files of 1.2) and `media/<id>` for each media it cites.
 *
 * Reading one is strict: at most `MAX_ENTRIES` entries, each entry and the whole bounded once
 * decompressed (read in small steps, stopped as soon as a bound is passed: no « zip bomb »),
 * every other name ignored without being decompressed, each media checked by its first bytes
 * (the type its name says) and its size, then prepared exactly like a file picked on the PC
 * (pictures encoded again) before it is sent to the PC.
 */

export const ARCHIVE_SUFFIX = ".cordedash";
export const DEFINITION_ENTRY = "dashboard.json";
export const MEDIA_PREFIX = "media/";
/** The largest file read. */
export const MAX_ARCHIVE_BYTES = 128 * 1024 * 1024;
/** Most entries in an archive (anything, ignored ones included). */
export const MAX_ENTRIES = 64;
/** The definition, decompressed. */
export const MAX_DEFINITION_ENTRY = MAX_DASH_BYTES * 2;
/** Everything decompressed. */
export const MAX_ARCHIVE_TOTAL = 128 * 1024 * 1024;
/** Fed to the decompressor in steps of this size, so that one step cannot expand far. */
const STEP = 16 * 1024;
/** A fixed date in the archive: the file does not say when it was made. */
const ZIP_DATE = new Date(2026, 0, 1);

export function archiveName(def: Pick<DashDefinition, "name">, slugOf: (s: string) => string) {
  return `${slugOf(def.name)}${ARCHIVE_SUFFIX}`;
}

export function isZip(bytes: Uint8Array): boolean {
  return bytes[0] === 0x50 && bytes[1] === 0x4b && bytes[2] === 0x03 && bytes[3] === 0x04;
}

// ---------------------------------------------------------------- writing

export interface Built {
  bytes: Uint8Array;
  /** Media cited but not found on the PC (left out). */
  missing: string[];
  /** The archive holds a video (shared as it is: the driver is told). */
  hasVideo: boolean;
}

/** The archive of a definition and the media it cites. */
export async function buildArchive(
  def: DashDefinition,
  read: (id: string) => Promise<Uint8Array | null> = fetchMedia,
): Promise<Built> {
  const files: Record<string, Uint8Array | [Uint8Array, { level: 0 }]> = {
    [DEFINITION_ENTRY]: strToU8(exportText(def)),
  };
  const missing: string[] = [];
  let hasVideo = false;
  for (const id of referencedAssets(def)) {
    const bytes = await read(id);
    if (!bytes) {
      missing.push(id);
      continue;
    }
    if (id.endsWith(".mp4") || id.endsWith(".webm")) hasVideo = true;
    // Pictures and videos are compressed already: stored as they are.
    files[`${MEDIA_PREFIX}${id}`] = [bytes, { level: 0 }];
  }
  return { bytes: zipSync(files, { level: 6, mtime: ZIP_DATE }), missing, hasVideo };
}

/** Hands a file to the browser (a download, as the CSV of a lap). */
export function downloadBytes(name: string, bytes: Uint8Array, mime: string): void {
  const url = URL.createObjectURL(new Blob([bytes as BlobPart], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Exports a dashboard as `<name>.cordedash`, its media with it. */
export async function exportDash(def: DashDefinition): Promise<Omit<Built, "bytes">> {
  const built = await buildArchive(def);
  downloadBytes(archiveName(def, slug), built.bytes, "application/zip");
  return { missing: built.missing, hasVideo: built.hasVideo };
}

// ---------------------------------------------------------------- reading

export type Read =
  | { ok: true; json: string; media: Map<string, Uint8Array> }
  | { ok: false; problem: DashProblem };

class Stop extends Error {
  constructor(readonly problem: DashProblem) {
    super(problem.code);
  }
}

/** The kind of an entry that is read, `null` for one that is ignored. */
function wanted(name: string): "definition" | MediaExt | null {
  if (name === DEFINITION_ENTRY) return "definition";
  if (!name.startsWith(MEDIA_PREFIX)) return null;
  const id = name.slice(MEDIA_PREFIX.length);
  return validAssetId(id) ? (id.slice(id.lastIndexOf(".") + 1) as MediaExt) : null;
}

/** Reads an archive within every bound (see the top of this file). */
export function readArchive(bytes: Uint8Array): Read {
  if (bytes.length > MAX_ARCHIVE_BYTES)
    return { ok: false, problem: { code: "too-large", at: "" } };
  if (!isZip(bytes)) return { ok: false, problem: { code: "archive", at: "" } };
  const chunks = new Map<string, Uint8Array[]>();
  const sizes = new Map<string, number>();
  const done = new Set<string>();
  let entries = 0;
  let total = 0;
  const st: { failure: DashProblem | null } = { failure: null };
  const unzip = new Unzip((file) => {
    entries += 1;
    if (entries > MAX_ENTRIES) throw new Stop({ code: "archive", at: "entries" });
    const kind = wanted(file.name);
    if (kind === null || chunks.has(file.name)) return; // ignored: never decompressed
    const limit = kind === "definition" ? MAX_DEFINITION_ENTRY : maxBytes(kind);
    // The size the archive announces is checked first, then the real one, step by step.
    if ((file.originalSize ?? 0) > limit) {
      throw new Stop({ code: kind === "definition" ? "too-large" : "media", at: file.name });
    }
    chunks.set(file.name, []);
    sizes.set(file.name, 0);
    file.ondata = (err, data, final) => {
      if (st.failure) return;
      if (err) {
        st.failure = { code: "archive", at: file.name };
        return;
      }
      const n = (sizes.get(file.name) ?? 0) + data.length;
      total += data.length;
      if (n > limit) {
        st.failure = { code: kind === "definition" ? "too-large" : "media", at: file.name };
        file.terminate();
        return;
      }
      if (total > MAX_ARCHIVE_TOTAL) {
        st.failure = { code: "too-large", at: "" };
        file.terminate();
        return;
      }
      sizes.set(file.name, n);
      chunks.get(file.name)?.push(data);
      if (final) done.add(file.name);
    };
    file.start();
  });
  unzip.register(UnzipInflate);
  unzip.register(UnzipPassThrough);
  try {
    for (let i = 0; i < bytes.length && !st.failure; i += STEP) {
      const end = Math.min(bytes.length, i + STEP);
      unzip.push(bytes.subarray(i, end), end === bytes.length);
    }
  } catch (e) {
    if (e instanceof Stop) return { ok: false, problem: e.problem };
    return { ok: false, problem: { code: "archive", at: "" } };
  }
  if (st.failure) return { ok: false, problem: st.failure };
  const join = (name: string): Uint8Array => {
    const parts = chunks.get(name) ?? [];
    const out = new Uint8Array(sizes.get(name) ?? 0);
    let off = 0;
    for (const p of parts) {
      out.set(p, off);
      off += p.length;
    }
    return out;
  };
  if (!done.has(DEFINITION_ENTRY)) return { ok: false, problem: { code: "archive", at: "" } };
  let json: string;
  try {
    json = strFromU8(join(DEFINITION_ENTRY));
  } catch {
    return { ok: false, problem: { code: "not-json", at: "" } };
  }
  const media = new Map<string, Uint8Array>();
  for (const name of done) {
    if (name === DEFINITION_ENTRY) continue;
    const id = name.slice(MEDIA_PREFIX.length);
    const bytes2 = join(name);
    // The type its name says, by its first bytes.
    const ext = sniff(bytes2);
    if (!ext || ext !== id.slice(id.lastIndexOf(".") + 1)) {
      return { ok: false, problem: { code: "media", at: name } };
    }
    media.set(id, bytes2);
  }
  return { ok: true, json, media };
}

// ---------------------------------------------------------------- import

/** The ids of the media in a definition replaced (`map`), the others kept. */
export function remapAssets(def: DashDefinition, map: ReadonlyMap<string, string>): DashDefinition {
  if (map.size === 0) return def;
  const swap = (v: unknown) => (typeof v === "string" && map.has(v) ? map.get(v) : v);
  return {
    ...def,
    pages: def.pages.map((p) => ({
      ...p,
      ...(p.background
        ? { background: { ...p.background, asset: swap(p.background.asset) as string | undefined } }
        : {}),
      elements: p.elements.map((e) => ({
        ...e,
        props: Object.fromEntries(
          Object.entries(e.props).map(([k, v]) => [k, isAssetKey(k) ? swap(v) : v]),
        ),
      })),
    })),
  };
}

export interface ImportTools {
  prepare: (bytes: Uint8Array) => Promise<Prepared>;
  upload: (bytes: Uint8Array) => Promise<UploadResult>;
}

export type ImportedArchive =
  | (Imported & { ok: true; media: number; hasVideo: boolean })
  | { ok: false; problem: DashProblem }
  | { ok: false; upload: Exclude<UploadResult, { ok: true }>["reason"] };

/**
 * Imports a `.cordedash` (or a `.cordedash.json` of 1.2): the definition checked like any
 * other, its media prepared like a file picked on the PC, sent to the PC, and cited by their
 * new id.
 */
export async function importFile(
  bytes: Uint8Array,
  taken: Iterable<string>,
  tools: ImportTools = { prepare: (b) => prepareMedia(b), upload: uploadMedia },
): Promise<ImportedArchive> {
  if (!isZip(bytes)) {
    if (bytes.length > MAX_DASH_BYTES * 4)
      return { ok: false, problem: { code: "too-large", at: "" } };
    let text: string;
    try {
      text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    } catch {
      return { ok: false, problem: { code: "not-json", at: "" } };
    }
    const r = importText(text, taken);
    return r.ok ? { ...r, media: 0, hasVideo: false } : r;
  }
  const read = readArchive(bytes);
  if (!read.ok) return read;
  const checked = parseDefinition(read.json);
  if (!checked.ok) return checked;
  const map = new Map<string, string>();
  let hasVideo = false;
  for (const id of referencedAssets(checked.def)) {
    const raw = read.media.get(id);
    if (!raw) continue; // not in the file: an empty place, as a media missing on the PC
    const prepared = await tools.prepare(raw);
    if (!prepared.ok) return { ok: false, problem: { code: "media", at: id } };
    if (kindOfExt(prepared.ext) === "video") hasVideo = true;
    const sent = await tools.upload(prepared.bytes);
    if (!sent.ok) return { ok: false, upload: sent.reason };
    map.set(id, sent.id);
  }
  const used = new Set(taken);
  const def = remapAssets(checked.def, map);
  const id = def.id === BUILTIN_ID || used.has(def.id) ? freshId(def.name, used) : def.id;
  return { ok: true, def: { ...def, id, updated_ms: Date.now() }, media: map.size, hasVideo };
}
