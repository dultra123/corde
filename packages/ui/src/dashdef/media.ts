import { useEffect, useState } from "react";
import { authHeaders } from "../net/auth";
import { validAssetId } from "./model";
import { useDashes } from "./store";

/**
 * Pictures and videos of the dashboards (ADR 0016). They come only from files the driver picks
 * on the PC; they are stored on the PC (`/api/dash-assets`) and a definition cites them by id.
 *
 * Before a file is sent:
 * - a **picture** (PNG, JPEG, WebP) is drawn again in the browser (a canvas) and encoded anew:
 *   what the camera wrote in it (EXIF, the GPS position, the camera, the date) does not survive,
 *   and its larger side is brought to 2560 px at most. A GIF is kept as it is (its animation);
 * - a **video** MP4 / MOV keeps its pictures only: its metadata boxes (`udta`, `meta`, XMP, the
 *   boxes of the camera maker) are emptied in place and renamed `free`, and every other track is
 *   removed with its data — the sound (a dashboard video is always silent), the timed data of an
 *   action camera (its GPS trace), text. Same size, nothing moves, it still plays. A WebM is kept
 *   as it is, its sound included.
 * The type is read from the first bytes of the file, never from its name.
 */

export type MediaExt = "png" | "jpg" | "webp" | "gif" | "mp4" | "webm";
export type MediaKind = "image" | "video";

export const MAX_IMAGE_BYTES = 4 * 1024 * 1024;
export const MAX_VIDEO_BYTES = 30 * 1024 * 1024;
/** The larger side of a picture once encoded again. */
export const MAX_IMAGE_SIDE = 2560;

export const MIME: Record<MediaExt, string> = {
  png: "image/png",
  jpg: "image/jpeg",
  webp: "image/webp",
  gif: "image/gif",
  mp4: "video/mp4",
  webm: "video/webm",
};

/** What the file picker offers (the type is checked again from the bytes). */
export const ACCEPT_IMAGES = ".png,.jpg,.jpeg,.webp,.gif,image/png,image/jpeg,image/webp,image/gif";
export const ACCEPT_VIDEOS = ".mp4,.m4v,.mov,.webm,video/mp4,video/quicktime,video/webm";
export const ACCEPT_MEDIA = `${ACCEPT_IMAGES},${ACCEPT_VIDEOS}`;

function startsWith(b: Uint8Array, sig: readonly number[], at = 0): boolean {
  if (b.length < at + sig.length) return false;
  return sig.every((x, i) => b[at + i] === x);
}

function ascii(s: string): number[] {
  return [...s].map((c) => c.charCodeAt(0));
}

/** The type of a file from its first bytes (the same rules as the server, `dash_assets::sniff`). */
export function sniff(b: Uint8Array): MediaExt | null {
  if (startsWith(b, [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])) return "png";
  if (startsWith(b, [0xff, 0xd8, 0xff])) return "jpg";
  if (startsWith(b, ascii("GIF87a")) || startsWith(b, ascii("GIF89a"))) return "gif";
  if (startsWith(b, ascii("RIFF")) && startsWith(b, ascii("WEBP"), 8)) return "webp";
  if (b.length >= 12 && startsWith(b, ascii("ftyp"), 4)) {
    const size = ((b[0] ?? 0) << 24) | ((b[1] ?? 0) << 16) | ((b[2] ?? 0) << 8) | (b[3] ?? 0);
    if (size >= 8 && size <= 4096) return "mp4";
  }
  if (startsWith(b, [0x1a, 0x45, 0xdf, 0xa3])) {
    const head = b.subarray(0, 64);
    const webm = ascii("webm");
    for (let i = 0; i + 4 <= head.length; i += 1) if (startsWith(head, webm, i)) return "webm";
  }
  return null;
}

export function kindOfExt(ext: MediaExt): MediaKind {
  return ext === "mp4" || ext === "webm" ? "video" : "image";
}

/** The kind a media id names (`…png` → image), `null` for anything else. */
export function kindOfId(id: unknown): MediaKind | null {
  if (!validAssetId(id)) return null;
  return kindOfExt(id.slice(id.lastIndexOf(".") + 1) as MediaExt);
}

export function maxBytes(ext: MediaExt): number {
  return kindOfExt(ext) === "video" ? MAX_VIDEO_BYTES : MAX_IMAGE_BYTES;
}

// ---------------------------------------------------------------- MP4 metadata

/** Top-level boxes a player needs. Anything else there (XMP, previews, camera maker boxes) is emptied. */
const TOP_KEPT = new Set([
  "ftyp",
  "styp",
  "moov",
  "mdat",
  "moof",
  "mfra",
  "sidx",
  "ssix",
  "prft",
  "pdin",
]);
/** Free space: its content is emptied, its name kept. */
const FREE_BOXES = new Set(["free", "skip", "wide"]);
/** Boxes of metadata inside `moov` and inside a track. */
const META_BOXES = new Set(["udta", "meta", "uuid"]);

export class BadVideo extends Error {}

interface Box {
  type: string;
  start: number;
  header: number;
  end: number;
}

function boxesIn(b: Uint8Array, from: number, to: number): Box[] {
  const view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const out: Box[] = [];
  let off = from;
  while (off + 8 <= to) {
    let size = view.getUint32(off);
    const type = String.fromCharCode(
      b[off + 4] ?? 0,
      b[off + 5] ?? 0,
      b[off + 6] ?? 0,
      b[off + 7] ?? 0,
    );
    let header = 8;
    if (size === 1) {
      if (off + 16 > to) throw new BadVideo("truncated box");
      const big = view.getBigUint64(off + 8);
      if (big > BigInt(Number.MAX_SAFE_INTEGER)) throw new BadVideo("box too large");
      size = Number(big);
      header = 16;
    } else if (size === 0) {
      size = to - off;
    }
    if (size < header || off + size > to) throw new BadVideo(`bad box ${type}`);
    out.push({ type, start: off, header, end: off + size });
    off += size;
  }
  return out;
}

function childrenOf(b: Uint8Array, parent: Box): Box[] {
  return boxesIn(b, parent.start + parent.header, parent.end);
}

function child(b: Uint8Array, parent: Box | undefined, type: string): Box | undefined {
  return parent && childrenOf(b, parent).find((x) => x.type === type);
}

/** What a track holds (`vide`, `soun`, `meta`, `text`…), `null` when it cannot be read. */
function handlerOf(b: Uint8Array, trak: Box): string | null {
  const hdlr = child(b, child(b, trak, "mdia"), "hdlr");
  if (!hdlr) return null;
  // version and flags (4), a type the files do not agree on (4), then the handler.
  const at = hdlr.start + hdlr.header + 8;
  if (at + 4 > hdlr.end) return null;
  return String.fromCharCode(b[at] ?? 0, b[at + 1] ?? 0, b[at + 2] ?? 0, b[at + 3] ?? 0);
}

type Range = [start: number, end: number];

/** Where the samples of a track are in the file: one range per chunk (`stco`/`co64`, `stsc`, `stsz`). */
function chunkRanges(b: Uint8Array, trak: Box, fragmented: boolean): Range[] {
  const stbl = child(b, child(b, child(b, trak, "mdia"), "minf"), "stbl");
  const stsz = child(b, stbl, "stsz");
  const stsc = child(b, stbl, "stsc");
  const offs = child(b, stbl, "stco") ?? child(b, stbl, "co64");
  if (!stsz || !stsc || !offs) {
    // A fragmented file may list no sample at all in `moov`.
    if (fragmented) return [];
    throw new BadVideo("no sample table");
  }
  const view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const body = (x: Box) => x.start + x.header;
  const need = (x: Box, n: number) => {
    if (body(x) + n > x.end) throw new BadVideo(`short ${x.type}`);
  };
  need(stsz, 12);
  const fixed = view.getUint32(body(stsz) + 4);
  const samples = view.getUint32(body(stsz) + 8);
  if (fixed === 0) need(stsz, 12 + samples * 4);
  need(stsc, 8);
  const runs = view.getUint32(body(stsc) + 4);
  need(stsc, 8 + runs * 12);
  need(offs, 8);
  const chunks = view.getUint32(body(offs) + 4);
  const wide = offs.type === "co64";
  need(offs, 8 + chunks * (wide ? 8 : 4));
  const out: Range[] = [];
  let sample = 0;
  let run = 0;
  for (let c = 0; c < chunks; c += 1) {
    while (run + 1 < runs && view.getUint32(body(stsc) + 8 + (run + 1) * 12) <= c + 1) run += 1;
    const per = runs === 0 ? 0 : view.getUint32(body(stsc) + 8 + run * 12 + 4);
    const n = Math.min(per, samples - sample);
    let size = 0;
    if (fixed !== 0) size = n * fixed;
    else for (let i = 0; i < n; i += 1) size += view.getUint32(body(stsz) + 12 + (sample + i) * 4);
    sample += n;
    const start = wide
      ? Number(view.getBigUint64(body(offs) + 8 + c * 8))
      : view.getUint32(body(offs) + 8 + c * 4);
    if (size === 0) continue;
    if (start + size > b.length) throw new BadVideo("samples out of the file");
    out.push([start, start + size]);
  }
  return out;
}

/** The id of a track (`tkhd`), `null` when it cannot be read. */
function trackId(b: Uint8Array, trak: Box): number | null {
  const tkhd = child(b, trak, "tkhd");
  if (!tkhd) return null;
  const at = tkhd.start + tkhd.header;
  if (at + 4 > tkhd.end) return null;
  // version and flags, two dates of 4 bytes (8 in version 1), then the id.
  const idAt = at + 4 + (b[at] === 1 ? 16 : 8);
  if (idAt + 4 > tkhd.end) return null;
  return new DataView(b.buffer, b.byteOffset, b.byteLength).getUint32(idAt);
}

interface Fragments {
  /** The samples of the picture tracks. */
  kept: Range[];
  /** The fragments (`traf`) of the other tracks. */
  dropped: Box[];
  /**
   * `false` when a fragment of pictures finds its samples "after those of the fragment before
   * it" (an old layout): removing that fragment would move them, so the tracks are all kept.
   */
  removable: boolean;
}

/**
 * Where the samples of a fragmented file are (`moof` / `traf` / `tfhd` / `trun`), for the
 * picture tracks; `defaults` gives the sample size `mvex/trex` sets for a track.
 */
function fragmentRanges(
  b: Uint8Array,
  top: Box[],
  pictures: Set<number>,
  defaults: Map<number, number>,
): Fragments {
  const view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  const out: Fragments = { kept: [], dropped: [], removable: true };
  for (const moof of top) {
    if (moof.type !== "moof") continue;
    let previousEnd = moof.start;
    let first = true;
    for (const traf of childrenOf(b, moof)) {
      if (traf.type !== "traf") continue;
      const kids = childrenOf(b, traf);
      const tfhd = kids.find((x) => x.type === "tfhd");
      if (!tfhd) throw new BadVideo("no tfhd");
      let at = tfhd.start + tfhd.header;
      const need = (n: number) => {
        if (at + n > tfhd.end) throw new BadVideo("short tfhd");
      };
      need(8);
      const flags = view.getUint32(at) & 0xffffff;
      const id = view.getUint32(at + 4);
      at += 8;
      const picture = pictures.has(id);
      let base = first || (flags & 0x020000) !== 0 ? moof.start : previousEnd;
      if (flags & 0x1) {
        need(8);
        base = Number(view.getBigUint64(at));
        at += 8;
      } else if (!first && (flags & 0x020000) === 0 && picture) {
        out.removable = false;
      }
      if (flags & 0x2) at += 4;
      if (flags & 0x8) at += 4;
      let fixed = defaults.get(id);
      if (flags & 0x10) {
        need(4);
        fixed = view.getUint32(at);
      }
      let cursor = base;
      for (const trun of kids) {
        if (trun.type !== "trun") continue;
        let p = trun.start + trun.header;
        if (p + 8 > trun.end) throw new BadVideo("short trun");
        const f = view.getUint32(p) & 0xffffff;
        const count = view.getUint32(p + 4);
        p += 8;
        if (f & 0x1) {
          if (p + 4 > trun.end) throw new BadVideo("short trun");
          cursor = base + view.getInt32(p);
          p += 4;
        }
        if (f & 0x4) p += 4;
        // Per sample: its duration, its size, its flags, its time offset, each when said.
        const per =
          (f & 0x100 ? 4 : 0) + (f & 0x200 ? 4 : 0) + (f & 0x400 ? 4 : 0) + (f & 0x800 ? 4 : 0);
        if (p + count * per > trun.end) throw new BadVideo("short trun");
        let size = 0;
        if (f & 0x200) {
          const sizeAt = f & 0x100 ? 4 : 0;
          for (let i = 0; i < count; i += 1) size += view.getUint32(p + i * per + sizeAt);
        } else {
          if (fixed === undefined) throw new BadVideo("no sample size");
          size = count * fixed;
        }
        if (cursor < 0 || cursor + size > b.length) throw new BadVideo("samples out of the file");
        if (picture && size > 0) out.kept.push([cursor, cursor + size]);
        cursor += size;
      }
      previousEnd = cursor;
      first = false;
      if (!picture) out.dropped.push(traf);
    }
  }
  return out;
}

/** Sets to zero what is in `from..to` and in none of the ranges (sorted by their start). */
function zeroOutside(b: Uint8Array, from: number, to: number, kept: Range[]): void {
  let at = from;
  for (const [s, e] of kept) {
    if (e <= at) continue;
    if (s >= to) break;
    if (s > at) b.fill(0, at, s);
    at = e;
    if (at >= to) return;
  }
  b.fill(0, at, to);
}

export interface StrippedVideo {
  bytes: Uint8Array;
  /** Boxes of metadata emptied. */
  cleared: number;
  /** Tracks removed: the sound, timed data (the GPS trace of an action camera), text. */
  tracks: number;
}

/**
 * Keeps the pictures of an MP4 / MOV and nothing else, **in place** (a copy is returned):
 * - each box of metadata (`udta`, `meta`, `uuid`: the position, the date, the camera, XMP) of
 *   the file, of `moov` and of each track, and each top-level box a player does not need, is
 *   renamed `free` and set to zero;
 * - each track that is not a picture track — the sound, the timed data of an action camera (its
 *   GPS trace), text, timecode — is removed the same way (in a fragmented file, its fragments
 *   too), and in the media data every byte that is not a sample of a picture track is set to
 *   zero (their samples, and anything left there).
 * The size of the file and the place of every sample kept do not change: the video plays the
 * same, silent as a dashboard shows it. Throws [`BadVideo`] when the boxes cannot be read, or
 * when the file has no picture track (a sound file, a HEIC photo).
 */
export function stripMp4Metadata(input: Uint8Array): StrippedVideo {
  const b = new Uint8Array(input);
  const view = new DataView(b.buffer, b.byteOffset, b.byteLength);
  let cleared = 0;
  const free = ascii("free");
  const empty = (box: Box) => b.fill(0, box.start + box.header, box.end);
  const blank = (box: Box) => {
    b.set(free, box.start + 4);
    empty(box);
  };
  const clean = (boxes: Box[]) => {
    for (const box of boxes) {
      if (FREE_BOXES.has(box.type)) empty(box);
      else if (META_BOXES.has(box.type)) {
        blank(box);
        cleared += 1;
      }
    }
  };

  const top = boxesIn(b, 0, b.length);
  const moov = top.find((x) => x.type === "moov");
  if (!moov) throw new BadVideo("no moov");
  const inMoov = childrenOf(b, moov);
  const traks = inMoov.filter((x) => x.type === "trak");
  const video = traks.filter((t) => handlerOf(b, t) === "vide");
  if (video.length === 0) throw new BadVideo("no picture track");
  const others = traks.filter((t) => !video.includes(t));
  const mvex = inMoov.find((x) => x.type === "mvex");
  const fragmented = mvex !== undefined || top.some((x) => x.type === "moof");

  // What the picture tracks point at: never touched, wherever it is.
  const kept = video.flatMap((t) => chunkRanges(b, t, fragmented));
  let fragments: Fragments = { kept: [], dropped: [], removable: true };
  const pictures = new Set(video.map((t) => trackId(b, t)).filter((id) => id !== null));
  const trexes = mvex ? childrenOf(b, mvex).filter((x) => x.type === "trex") : [];
  // In a `trex` and a `tfra`: version and flags, then the id of the track.
  const idIn = (box: Box) =>
    box.start + box.header + 8 <= box.end ? view.getUint32(box.start + box.header + 4) : null;
  if (fragmented) {
    const defaults = new Map<number, number>();
    for (const x of trexes) {
      const at = x.start + x.header;
      if (at + 24 <= x.end) defaults.set(view.getUint32(at + 4), view.getUint32(at + 16));
    }
    fragments = fragmentRanges(b, top, pictures, defaults);
    kept.push(...fragments.kept);
  }
  kept.sort((x, y) => x[0] - y[0]);
  const holdsSamples = (box: Box) => kept.some(([s, e]) => s < box.end && e > box.start);

  for (const box of top) {
    if (TOP_KEPT.has(box.type)) continue;
    if (FREE_BOXES.has(box.type)) {
      if (!holdsSamples(box)) empty(box);
    } else if (!holdsSamples(box)) {
      blank(box);
      cleared += 1;
    }
  }
  clean(inMoov);
  for (const t of traks) clean(childrenOf(b, t));

  let tracks = 0;
  if (fragments.removable) {
    for (const m of top) if (m.type === "mdat") zeroOutside(b, m.start + m.header, m.end, kept);
    for (const t of others) {
      blank(t);
      tracks += 1;
    }
    // A picture track no longer points at a track that is gone (timecode, chapters).
    for (const t of video) {
      const tref = child(b, t, "tref");
      if (tref && others.length > 0) blank(tref);
    }
    // In a fragmented file: their fragments, what `mvex` says of them, their indexes.
    for (const traf of fragments.dropped) blank(traf);
    const gone = (box: Box) => {
      const id = idIn(box);
      return id === null || !pictures.has(id);
    };
    for (const x of trexes) if (gone(x)) blank(x);
    for (const box of top) {
      if (box.type === "sidx" && gone(box)) blank(box);
      if (box.type === "mfra")
        for (const x of childrenOf(b, box)) if (x.type === "tfra" && gone(x)) blank(x);
    }
  }
  return { bytes: b, cleared, tracks };
}

// ---------------------------------------------------------------- pictures

/** Encodes a picture again in the browser: no metadata, the larger side ≤ 2560 px. */
export type Reencoder = (bytes: Uint8Array, ext: "png" | "jpg" | "webp") => Promise<Uint8Array>;

function canvasOf(w: number, h: number): HTMLCanvasElement {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  return c;
}

function toBlob(c: HTMLCanvasElement, type: string, quality?: number): Promise<Blob | null> {
  return new Promise((resolve) => c.toBlob(resolve, type, quality));
}

/** The default re-encoder: a canvas of the browser. */
export const canvasReencoder: Reencoder = async (bytes, ext) => {
  const blob = new Blob([bytes as BlobPart], { type: MIME[ext] });
  // The orientation the camera wrote is applied to the pixels (it will not be in the file).
  const bitmap = await createImageBitmap(blob, { imageOrientation: "from-image" });
  const scale = Math.min(1, MAX_IMAGE_SIDE / Math.max(bitmap.width, bitmap.height));
  const w = Math.max(1, Math.round(bitmap.width * scale));
  const h = Math.max(1, Math.round(bitmap.height * scale));
  const canvas = canvasOf(w, h);
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no canvas");
  ctx.drawImage(bitmap, 0, 0, w, h);
  bitmap.close?.();
  // The same kind of file when the browser can write it; a photo stays a photo (JPEG), a logo
  // keeps its transparency (PNG, else WebP).
  const tries: [string, number | undefined][] =
    ext === "jpg"
      ? [
          ["image/jpeg", 0.9],
          ["image/jpeg", 0.8],
        ]
      : ext === "webp"
        ? [
            ["image/webp", 0.9],
            ["image/png", undefined],
            ["image/jpeg", 0.85],
          ]
        : [
            ["image/png", undefined],
            ["image/webp", 0.9],
            ["image/jpeg", 0.85],
          ];
  let last: Uint8Array | null = null;
  for (const [type, q] of tries) {
    const out = await toBlob(canvas, type, q);
    if (!out) continue;
    const bytes2 = new Uint8Array(await out.arrayBuffer());
    // A browser that cannot write this type gives a PNG: checked by its bytes.
    if (sniff(bytes2) === null) continue;
    last = bytes2;
    if (bytes2.length <= MAX_IMAGE_BYTES) return bytes2;
  }
  if (last) return last;
  throw new Error("encode failed");
};

export type Prepared =
  | { ok: true; bytes: Uint8Array; ext: MediaExt; kind: MediaKind }
  | { ok: false; reason: "type" | "too-large" | "unreadable" };

/**
 * Reads a file the driver picked, and makes it ready to be sent: its type from its bytes,
 * a picture encoded again (no metadata, 2560 px at most), the metadata of an MP4 emptied.
 */
export async function prepareMedia(
  input: Uint8Array,
  reencode: Reencoder = canvasReencoder,
): Promise<Prepared> {
  const ext = sniff(input);
  if (!ext) return { ok: false, reason: "type" };
  const kind = kindOfExt(ext);
  try {
    if (ext === "gif") {
      return input.length > MAX_IMAGE_BYTES
        ? { ok: false, reason: "too-large" }
        : { ok: true, bytes: input, ext, kind };
    }
    if (ext === "png" || ext === "jpg" || ext === "webp") {
      // A picture far too large to be a dashboard's: refused before it is decoded.
      if (input.length > 64 * 1024 * 1024) return { ok: false, reason: "too-large" };
      const out = await reencode(input, ext);
      const outExt = sniff(out);
      if (!outExt || kindOfExt(outExt) !== "image") return { ok: false, reason: "unreadable" };
      if (out.length > MAX_IMAGE_BYTES) return { ok: false, reason: "too-large" };
      return { ok: true, bytes: out, ext: outExt, kind };
    }
    if (input.length > MAX_VIDEO_BYTES) return { ok: false, reason: "too-large" };
    if (ext === "mp4") return { ok: true, bytes: stripMp4Metadata(input).bytes, ext, kind };
    return { ok: true, bytes: input, ext, kind };
  } catch {
    return { ok: false, reason: "unreadable" };
  }
}

// ---------------------------------------------------------------- server

export type UploadResult =
  | { ok: true; id: string }
  | { ok: false; reason: "offline" | "pc-only" | "type" | "too-large" | "full" | "refused" };

function base(): string | null {
  return useDashes.getState().base;
}

/** Sends a prepared media to the PC (this PC only); its id comes back. */
export async function uploadMedia(bytes: Uint8Array): Promise<UploadResult> {
  const b = base();
  if (b === null) return { ok: false, reason: "offline" };
  try {
    const res = await fetch(`${b}/api/dash-assets`, {
      method: "POST",
      headers: { "Content-Type": "application/octet-stream", ...authHeaders() },
      body: bytes as BodyInit,
    });
    if (res.status === 403) return { ok: false, reason: "pc-only" };
    if (res.status === 415) return { ok: false, reason: "type" };
    if (res.status === 413) return { ok: false, reason: "too-large" };
    if (res.status === 409) return { ok: false, reason: "full" };
    if (!res.ok) return { ok: false, reason: "refused" };
    const json = (await res.json()) as { id?: unknown };
    return validAssetId(json.id) ? { ok: true, id: json.id } : { ok: false, reason: "refused" };
  } catch {
    return { ok: false, reason: "offline" };
  }
}

/** The bytes of a media of the PC (to put it in a shared file), `null` when it is not there. */
export async function fetchMedia(id: string): Promise<Uint8Array | null> {
  const b = base();
  if (b === null || !validAssetId(id)) return null;
  try {
    const res = await fetch(`${b}/api/dash-assets/${id}`, { headers: authHeaders() });
    if (!res.ok) return null;
    return new Uint8Array(await res.arrayBuffer());
  } catch {
    return null;
  }
}

/**
 * Where the page reads a media. Straight from the server when no token is needed (this PC, a
 * phone on the PC: the browser asks for byte ranges itself, as an iPhone wants for a video);
 * through `fetch` with the token of the home server otherwise (a page cannot put a header on
 * an `<img>`), kept as a local object for the visit.
 */
const blobs = new Map<string, Promise<string | null>>();

export function mediaSrc(id: string, server: string): string | Promise<string | null> {
  const headers = authHeaders();
  const direct = `${server}/api/dash-assets/${id}`;
  if (!("Authorization" in headers)) return direct;
  const key = `${server}|${id}`;
  let p = blobs.get(key);
  if (!p) {
    p = fetch(direct, { headers })
      .then(async (r) => (r.ok ? URL.createObjectURL(await r.blob()) : null))
      .catch(() => null);
    blobs.set(key, p);
    // A failure is not kept: the next showing asks again.
    void p.then((u) => {
      if (u === null) blobs.delete(key);
    });
  }
  return p;
}

/** The address of a media for an `<img>` / `<video>`, `null` while unknown or absent. */
export function useMediaSrc(id: unknown): string | null {
  const server = useDashes((s) => s.base);
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    if (!validAssetId(id) || server === null) {
      setSrc(null);
      return;
    }
    const s = mediaSrc(id, server);
    if (typeof s === "string") {
      setSrc(s);
      return;
    }
    let live = true;
    setSrc(null);
    void s.then((u) => {
      if (live) setSrc(u);
    });
    return () => {
      live = false;
    };
  }, [id, server]);
  return src;
}
