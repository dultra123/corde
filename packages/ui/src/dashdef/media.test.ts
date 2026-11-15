import { describe, expect, it, vi } from "vitest";
import {
  BadVideo,
  kindOfId,
  MAX_IMAGE_BYTES,
  MAX_VIDEO_BYTES,
  prepareMedia,
  sniff,
  stripMp4Metadata,
} from "./media";

/**
 * The pictures and videos the driver picks (ADR 0016): read by their first bytes, a picture
 * encoded again (no metadata), the metadata boxes of an MP4 emptied without moving anything.
 * Every file here is made by the test itself.
 */

const enc = new TextEncoder();
const bytes = (s: string) => enc.encode(s);

function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

function u32(n: number): Uint8Array {
  const b = new Uint8Array(4);
  new DataView(b.buffer).setUint32(0, n);
  return b;
}

/** An ISO box: size, type, content. */
function box(type: string, ...content: Uint8Array[]): Uint8Array {
  const body = concat(...content);
  return concat(u32(8 + body.length), bytes(type), body);
}

/** A box with a 64-bit size. */
function bigBox(type: string, ...content: Uint8Array[]): Uint8Array {
  const body = concat(...content);
  const size = new Uint8Array(8);
  new DataView(size.buffer).setBigUint64(0, BigInt(16 + body.length));
  return concat(u32(1), bytes(type), size, body);
}

const XMP_UUID = new Uint8Array([
  0xbe, 0x7a, 0xcf, 0xcb, 0x97, 0xa9, 0x42, 0xe8, 0x9c, 0x71, 0x99, 0x94, 0x91, 0xe3, 0xaf, 0xac,
]);
const WHERE = "+46.2044+006.1432/";
const SAMPLES = "frame data that must not move";

/** A small MP4 as a phone writes one: a position in `moov/udta`, `moov/meta`, XMP, a track name. */
function phoneVideo(): Uint8Array {
  return concat(
    box("ftyp", bytes("isom"), u32(512), bytes("isomiso2mp41")),
    box(
      "moov",
      box("mvhd", new Uint8Array(100)),
      box("trak", box("tkhd", new Uint8Array(84)), box("udta", box("name", bytes("Camera 1")))),
      box("udta", box("©xyz", bytes(WHERE))),
      box(
        "meta",
        new Uint8Array(4),
        box("hdlr", new Uint8Array(25)),
        box("ilst", bytes(`com.apple.quicktime.location.ISO6709 ${WHERE}`)),
      ),
    ),
    box("uuid", XMP_UUID, bytes(`<x:xmpmeta>exif:GPSLatitude ${WHERE}</x:xmpmeta>`)),
    bigBox("udta", bytes(`date 2026-11-15 ${WHERE}`)),
    box("mdat", bytes(SAMPLES)),
  );
}

function has(hay: Uint8Array, needle: string): boolean {
  return new TextDecoder("latin1").decode(hay).includes(needle);
}

function topTypes(b: Uint8Array): string[] {
  const out: string[] = [];
  const v = new DataView(b.buffer, b.byteOffset, b.byteLength);
  let o = 0;
  while (o + 8 <= b.length) {
    let size = v.getUint32(o);
    if (size === 1) size = Number(v.getBigUint64(o + 8));
    out.push(new TextDecoder("latin1").decode(b.subarray(o + 4, o + 8)));
    o += size;
  }
  return out;
}

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13]);
const JPEG = new Uint8Array([0xff, 0xd8, 0xff, 0xe1, 0, 16, ...bytes("Exif\0\0GPS 46N")]);
const GIF = bytes("GIF89a\x01\x00\x01\x00");
const WEBP = concat(bytes("RIFF"), u32(4), bytes("WEBPVP8 "));
const WEBM = new Uint8Array([0x1a, 0x45, 0xdf, 0xa3, 0x9f, 0x42, 0x82, 0x84, ...bytes("webm")]);

describe("file types", () => {
  it("are read from the first bytes (the same rules as the server)", () => {
    expect(sniff(PNG)).toBe("png");
    expect(sniff(JPEG)).toBe("jpg");
    expect(sniff(GIF)).toBe("gif");
    expect(sniff(WEBP)).toBe("webp");
    expect(sniff(phoneVideo())).toBe("mp4");
    expect(sniff(WEBM)).toBe("webm");
    for (const bad of [
      "<svg xmlns='http://www.w3.org/2000/svg'><script>alert(1)</script></svg>",
      "<html></html>",
      "ID3\u0004",
      "OggS",
      "RIFF\u0010\0\0\0WAVEfmt ",
      "%PDF-1.7",
      "PK\u0003\u0004",
      "",
    ])
      expect(sniff(bytes(bad)), bad).toBeNull();
    expect(kindOfId(`${"a".repeat(64)}.webm`)).toBe("video");
    expect(kindOfId(`${"a".repeat(64)}.gif`)).toBe("image");
    expect(kindOfId("https://x.org/a.png")).toBeNull();
  });
});

describe("the metadata of a video", () => {
  it("are emptied in place: same size, the samples where they were, no position left", () => {
    const input = phoneVideo();
    expect(has(input, WHERE)).toBe(true);
    const { bytes: out, cleared } = stripMp4Metadata(input);
    expect(out.length).toBe(input.length);
    expect(has(out, WHERE)).toBe(false);
    expect(has(out, "Camera 1")).toBe(false);
    expect(has(out, "2026-11-15")).toBe(false);
    expect(cleared).toBe(5);
    // Every other byte is where it was: the media data first.
    const at = new TextDecoder("latin1").decode(input).indexOf(SAMPLES);
    expect(new TextDecoder("latin1").decode(out).indexOf(SAMPLES)).toBe(at);
    expect(topTypes(out)).toEqual(["ftyp", "moov", "free", "free", "mdat"]);
    // The boxes that play are untouched; the original file too.
    expect(has(out, "isomiso2mp41")).toBe(true);
    expect(has(input, WHERE)).toBe(true);
    // Done twice: nothing more.
    expect(stripMp4Metadata(out).bytes).toEqual(out);
  });

  it("a file whose boxes cannot be read is refused", () => {
    const v = phoneVideo();
    const broken = v.slice(0, v.length - 4);
    expect(() => stripMp4Metadata(broken)).toThrow(BadVideo);
    const huge = concat(u32(0xffffff00), bytes("ftypisom"), new Uint8Array(8));
    expect(() => stripMp4Metadata(huge)).toThrow(BadVideo);
  });
});

describe("a file picked is prepared before it is sent", () => {
  const reencoded = concat(PNG, bytes("clean"));

  it("a picture is encoded again (its metadata do not survive); a GIF is kept", async () => {
    const reencode = vi.fn(async () => reencoded);
    const r = await prepareMedia(JPEG, reencode);
    expect(reencode).toHaveBeenCalledWith(JPEG, "jpg");
    expect(r).toEqual({ ok: true, bytes: reencoded, ext: "png", kind: "image" });
    for (const [b, ext] of [
      [PNG, "png"],
      [WEBP, "webp"],
    ] as const) {
      reencode.mockClear();
      await prepareMedia(b, reencode);
      expect(reencode).toHaveBeenCalledWith(b, ext);
    }
    reencode.mockClear();
    const gif = await prepareMedia(GIF, reencode);
    expect(reencode).not.toHaveBeenCalled();
    expect(gif).toEqual({ ok: true, bytes: GIF, ext: "gif", kind: "image" });
  });

  it("an MP4 loses its metadata, a WebM is kept as it is", async () => {
    const r = await prepareMedia(phoneVideo(), vi.fn());
    expect(r.ok && r.kind).toBe("video");
    expect(r.ok && has(r.bytes, WHERE)).toBe(false);
    const w = await prepareMedia(WEBM, vi.fn());
    expect(w).toEqual({ ok: true, bytes: WEBM, ext: "webm", kind: "video" });
  });

  it("refuses another type, a file too large, a picture that cannot be encoded", async () => {
    expect(await prepareMedia(bytes("<svg/>"), vi.fn())).toEqual({ ok: false, reason: "type" });
    const bigGif = new Uint8Array(MAX_IMAGE_BYTES + 1);
    bigGif.set(GIF);
    expect(await prepareMedia(bigGif, vi.fn())).toEqual({ ok: false, reason: "too-large" });
    const bigVideo = new Uint8Array(MAX_VIDEO_BYTES + 1);
    bigVideo.set(phoneVideo());
    expect(await prepareMedia(bigVideo, vi.fn())).toEqual({ ok: false, reason: "too-large" });
    const tooBigAfter = new Uint8Array(MAX_IMAGE_BYTES + 1);
    tooBigAfter.set(PNG);
    expect(await prepareMedia(PNG, async () => tooBigAfter)).toEqual({
      ok: false,
      reason: "too-large",
    });
    expect(
      await prepareMedia(PNG, async () => {
        throw new Error("decode");
      }),
    ).toEqual({ ok: false, reason: "unreadable" });
    // An encoder that would give back something else than a picture.
    expect(await prepareMedia(PNG, async () => bytes("<svg/>"))).toEqual({
      ok: false,
      reason: "unreadable",
    });
  });
});
