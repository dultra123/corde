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
 * - a **video** MP4 / MOV has its metadata boxes (`udta`, `meta`, XMP) emptied in place, renamed
 *   `free`, same size: nothing else of the file moves, it still plays. A WebM is kept as it is.
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

/** XMP stored in a `uuid` box (Adobe): the same metadata as a photo's, a position included. */
const XMP_UUID = [
  0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac,
];

/** Boxes of metadata, wherever they are met (top level, `moov`, `trak`). */
const META_BOXES = new Set(["udta", "meta"]);
/** Boxes whose children are looked at. */
const CONTAINERS = new Set(["moov", "trak"]);

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

function isXmp(b: Uint8Array, box: Box): boolean {
  return box.type === "uuid" && startsWith(b, XMP_UUID, box.start + box.header);
}

/**
 * Empties the metadata boxes of an MP4 / MOV **in place** (a copy is returned): each `udta`,
 * `meta` and XMP `uuid` box of the top level, of `moov` and of each `trak` is renamed `free` and
 * its content is set to zero. The size of the file and the place of every other box do not
 * change (the offsets of the samples stay right): the video plays the same, without its
 * position, its camera or its date. Throws [`BadVideo`] when the boxes cannot be read.
 */
export function stripMp4Metadata(input: Uint8Array): { bytes: Uint8Array; cleared: number } {
  const b = new Uint8Array(input);
  let cleared = 0;
  const free = ascii("free");
  const blank = (box: Box) => {
    b.set(free, box.start + 4);
    b.fill(0, box.start + box.header, box.end);
    cleared += 1;
  };
  const walk = (from: number, to: number, depth: number) => {
    for (const box of boxesIn(b, from, to)) {
      if (META_BOXES.has(box.type) || isXmp(b, box)) blank(box);
      else if (CONTAINERS.has(box.type) && depth < 2)
        walk(box.start + box.header, box.end, depth + 1);
    }
  };
  walk(0, b.length, 0);
  return { bytes: b, cleared };
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
