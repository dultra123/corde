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
const WHERE = "+47.9497+000.2073/";
const FRAME_1 = "frame data that must not move";
const FRAME_2 = "second chunk of pictures";
const VOICE = "the voice of the driver, recorded with the clip";
const TRACE = `gps trace of an action camera ${WHERE}`;

/** A box that starts with a version and flags. */
function fullBox(type: string, ...content: Uint8Array[]): Uint8Array {
  return box(type, u32(0), ...content);
}

/** A track: its handler, where its chunks are and the sizes of the samples of each chunk. */
function track(
  handler: string,
  chunks: { at: number; sizes: number[] }[],
  id = 1,
  ...extra: Uint8Array[]
): Uint8Array {
  const sizes = chunks.flatMap((c) => c.sizes);
  return box(
    "trak",
    fullBox("tkhd", u32(0), u32(0), u32(id), new Uint8Array(68)),
    ...extra,
    box(
      "mdia",
      box("mdhd", new Uint8Array(24)),
      fullBox("hdlr", u32(0), bytes(handler), new Uint8Array(13)),
      box(
        "minf",
        box(
          "stbl",
          fullBox("stsd", u32(0)),
          fullBox("stsz", u32(0), u32(sizes.length), ...sizes.map(u32)),
          fullBox(
            "stsc",
            u32(chunks.length),
            ...chunks.flatMap((c, i) => [u32(i + 1), u32(c.sizes.length), u32(1)]),
          ),
          fullBox("stco", u32(chunks.length), ...chunks.map((c) => u32(c.at))),
        ),
      ),
    ),
  );
}

interface Clip {
  bytes: Uint8Array;
  /** Where each piece of media data starts in the file. */
  at: { frame1: number; voice: number; trace: number; frame2: number };
}

/**
 * A small MP4 as a phone or an action camera writes one: pictures in two chunks, a sound track
 * and a track of timed data between them, a position in `moov/udta`, `moov/meta`, XMP, a track
 * name, a box of the camera maker.
 */
function clip(opts: { tref?: boolean; noPictures?: boolean } = {}): Clip {
  const build = (at: Clip["at"]) => {
    const head = concat(
      box("ftyp", bytes("isom"), u32(512), bytes("isomiso2mp41")),
      box(
        "moov",
        box("mvhd", new Uint8Array(100)),
        track(
          opts.noPictures ? "soun" : "vide",
          [
            { at: at.frame1, sizes: [10, FRAME_1.length - 10] },
            { at: at.frame2, sizes: [FRAME_2.length] },
          ],
          1,
          box("udta", box("name", bytes("Camera 1"))),
          ...(opts.tref ? [box("tref", box("tmcd", u32(2)))] : []),
        ),
        track("soun", [{ at: at.voice, sizes: [VOICE.length] }], 2),
        track("meta", [{ at: at.trace, sizes: [TRACE.length] }], 3),
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
    );
    return concat(head, box("mdat", bytes(FRAME_1), bytes(VOICE), bytes(TRACE), bytes(FRAME_2)));
  };
  // The offsets are 4 bytes each whatever they hold: the file is laid out once to know them.
  const draft = build({ frame1: 0, voice: 0, trace: 0, frame2: 0 });
  const frame1 = draft.length - (FRAME_1.length + VOICE.length + TRACE.length + FRAME_2.length);
  const voice = frame1 + FRAME_1.length;
  const trace = voice + VOICE.length;
  const at = { frame1, voice, trace, frame2: trace + TRACE.length };
  return { bytes: build(at), at };
}

function phoneVideo(): Uint8Array {
  return clip().bytes;
}

/**
 * A fragmented MP4 as a capture program writes one: no sample listed in `moov`, the pictures
 * and the sound in a `moof` each time, in this order of fragments. `implicit`: the second
 * fragment finds its samples "after those of the first" (an old layout) instead of from `moof`.
 */
function fragmentedClip(order: ("vide" | "soun")[], implicit = false): Clip {
  const ids = { vide: 1, soun: 2 };
  const data = { vide: FRAME_1, soun: VOICE };
  const build = (offsets: number[]) => {
    const moof = box(
      "moof",
      fullBox("mfhd", u32(1)),
      ...order.map((kind, i) =>
        box(
          "traf",
          // default-base-is-moof, or nothing said at all.
          concat(u32(16), bytes("tfhd"), u32(implicit ? 0 : 0x020000), u32(ids[kind])),
          implicit && i > 0
            ? // sample-size only: the data follows the fragment before.
              concat(u32(20), bytes("trun"), u32(0x200), u32(1), u32(data[kind].length))
            : // data-offset and sample-size.
              concat(
                u32(24),
                bytes("trun"),
                u32(0x201),
                u32(1),
                u32(offsets[i] ?? 0),
                u32(data[kind].length),
              ),
        ),
      ),
    );
    return concat(
      box("ftyp", bytes("iso5"), u32(512), bytes("iso5iso6mp41")),
      box(
        "moov",
        box("mvhd", new Uint8Array(100)),
        track("vide", [], ids.vide),
        track("soun", [], ids.soun),
        box(
          "mvex",
          fullBox("trex", u32(ids.vide), u32(1), u32(0), u32(0), u32(0)),
          fullBox("trex", u32(ids.soun), u32(1), u32(0), u32(0), u32(0)),
        ),
        box("udta", box("©xyz", bytes(WHERE))),
      ),
      moof,
      box("mdat", ...order.map((kind) => bytes(data[kind]))),
    );
  };
  const draft = build([0, 0]);
  const moofAt = new TextDecoder("latin1").decode(draft).indexOf("moof") - 4;
  const first = draft.length - order.reduce((n, kind) => n + data[kind].length, 0);
  const second = first + data[order[0] ?? "vide"].length;
  const bytesOut = build([first - moofAt, second - moofAt]);
  const at = { frame1: 0, voice: 0, trace: 0, frame2: 0 };
  if (order[0] === "vide") {
    at.frame1 = first;
    at.voice = second;
  } else {
    at.voice = first;
    at.frame1 = second;
  }
  return { bytes: bytesOut, at };
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

function text(b: Uint8Array, from: number, len: number): string {
  return new TextDecoder("latin1").decode(b.subarray(from, from + len));
}

function isZero(b: Uint8Array, from: number, len: number): boolean {
  return b.subarray(from, from + len).every((x) => x === 0);
}

describe("a video keeps its pictures and nothing else", () => {
  it("metadata are emptied in place: same size, the pictures where they were, no position left", () => {
    const { bytes: input, at } = clip();
    expect(has(input, WHERE)).toBe(true);
    const { bytes: out, cleared, tracks } = stripMp4Metadata(input);
    expect(out.length).toBe(input.length);
    expect(has(out, WHERE)).toBe(false);
    expect(has(out, "Camera 1")).toBe(false);
    expect(has(out, "2026-11-15")).toBe(false);
    expect(cleared).toBe(5);
    expect(tracks).toBe(2);
    // The pictures are where they were, byte for byte.
    expect(text(out, at.frame1, FRAME_1.length)).toBe(FRAME_1);
    expect(text(out, at.frame2, FRAME_2.length)).toBe(FRAME_2);
    expect(topTypes(out)).toEqual(["ftyp", "moov", "free", "free", "mdat"]);
    // The boxes that play are untouched; the original file too.
    expect(has(out, "isomiso2mp41")).toBe(true);
    expect(has(out, "vide")).toBe(true);
    expect(has(input, WHERE)).toBe(true);
    // Done twice: nothing more.
    expect(stripMp4Metadata(out).bytes).toEqual(out);
  });

  it("the sound and the timed data of the camera are removed, with their samples", () => {
    const { bytes: input, at } = clip();
    expect(has(input, VOICE)).toBe(true);
    const { bytes: out } = stripMp4Metadata(input);
    expect(has(out, VOICE)).toBe(false);
    expect(has(out, "gps trace")).toBe(false);
    expect(isZero(out, at.voice, VOICE.length + TRACE.length)).toBe(true);
    // Their tracks are gone: a player sees pictures only.
    expect(has(out, "soun")).toBe(false);
    expect(has(out, "stco")).toBe(true);
  });

  it("a picture track no longer points at a track that is gone", () => {
    const { bytes: input } = clip({ tref: true });
    expect(has(input, "tref")).toBe(true);
    const { bytes: out } = stripMp4Metadata(input);
    expect(has(out, "tref")).toBe(false);
    expect(has(out, "tmcd")).toBe(false);
  });

  it("boxes a player does not need (camera maker, preview) are emptied, free space too", () => {
    const { bytes: input } = clip();
    const more = concat(
      input,
      box("CNTH", bytes(`thumbnail ${WHERE}`)),
      box("free", bytes(`left over ${WHERE}`)),
      box("skip", bytes("serial 0123456789")),
    );
    const { bytes: out } = stripMp4Metadata(more);
    expect(has(out, WHERE)).toBe(false);
    expect(has(out, "thumbnail")).toBe(false);
    expect(has(out, "serial")).toBe(false);
    expect(topTypes(out)).toEqual(["ftyp", "moov", "free", "free", "mdat", "free", "free", "skip"]);
  });

  it("a fragmented file loses its sound too, fragment by fragment", () => {
    for (const order of [
      ["vide", "soun"],
      ["soun", "vide"],
    ] as const) {
      const { bytes: input, at } = fragmentedClip([...order]);
      const { bytes: out, tracks } = stripMp4Metadata(input);
      expect(out.length, order.join()).toBe(input.length);
      expect(tracks, order.join()).toBe(1);
      expect(text(out, at.frame1, FRAME_1.length), order.join()).toBe(FRAME_1);
      expect(isZero(out, at.voice, VOICE.length), order.join()).toBe(true);
      expect(has(out, "soun")).toBe(false);
      expect(has(out, "©xyz")).toBe(false);
      // One fragment and one `trex` left: those of the pictures.
      const latin = new TextDecoder("latin1").decode(out);
      expect(latin.split("traf").length - 1).toBe(1);
      expect(latin.split("trex").length - 1).toBe(1);
      expect(stripMp4Metadata(out).bytes).toEqual(out);
    }
  });

  it("an old fragmented layout, where removing the sound would move the pictures, keeps its tracks", () => {
    const { bytes: input, at } = fragmentedClip(["soun", "vide"], true);
    const { bytes: out, tracks } = stripMp4Metadata(input);
    expect(tracks).toBe(0);
    expect(text(out, at.voice, VOICE.length)).toBe(VOICE);
    expect(text(out, at.frame1, FRAME_1.length)).toBe(FRAME_1);
    // Its metadata are emptied all the same.
    expect(has(out, "©xyz")).toBe(false);
    // The pictures first: nothing depends on the sound, it is removed.
    const ok = fragmentedClip(["vide", "soun"], true);
    const r = stripMp4Metadata(ok.bytes);
    expect(r.tracks).toBe(1);
    expect(isZero(r.bytes, ok.at.voice, VOICE.length)).toBe(true);
    expect(text(r.bytes, ok.at.frame1, FRAME_1.length)).toBe(FRAME_1);
  });

  it("a file whose boxes cannot be read, or with no picture, is refused", () => {
    const v = phoneVideo();
    const broken = v.slice(0, v.length - 4);
    expect(() => stripMp4Metadata(broken)).toThrow(BadVideo);
    const huge = concat(u32(0xffffff00), bytes("ftypisom"), new Uint8Array(8));
    expect(() => stripMp4Metadata(huge)).toThrow(BadVideo);
    // A sound file (M4A) and a HEIC photo also start with `ftyp`.
    expect(() => stripMp4Metadata(clip({ noPictures: true }).bytes)).toThrow(BadVideo);
    const heic = concat(box("ftyp", bytes("heic"), u32(0), bytes("mif1heic")), box("meta", u32(0)));
    expect(() => stripMp4Metadata(heic)).toThrow(BadVideo);
    // Pictures whose samples are said to be out of the file.
    const lost = concat(
      box("ftyp", bytes("isom"), u32(512), bytes("isomiso2mp41")),
      box(
        "moov",
        box("mvhd", new Uint8Array(100)),
        track("vide", [{ at: 1_000_000, sizes: [10] }]),
      ),
      box("mdat", bytes(FRAME_1)),
    );
    expect(() => stripMp4Metadata(lost)).toThrow(BadVideo);
    // Pictures with no table of their samples.
    const bare = concat(
      box("ftyp", bytes("isom"), u32(512), bytes("isomiso2mp41")),
      box(
        "moov",
        box("trak", box("mdia", fullBox("hdlr", u32(0), bytes("vide"), new Uint8Array(13)))),
      ),
      box("mdat", bytes(FRAME_1)),
    );
    expect(() => stripMp4Metadata(bare)).toThrow(BadVideo);
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

  it("an MP4 loses its metadata and its sound, a WebM is kept as it is", async () => {
    const r = await prepareMedia(phoneVideo(), vi.fn());
    expect(r.ok && r.kind).toBe("video");
    expect(r.ok && has(r.bytes, WHERE)).toBe(false);
    expect(r.ok && has(r.bytes, VOICE)).toBe(false);
    // A sound file is not a video, even with the first bytes of one.
    expect(await prepareMedia(clip({ noPictures: true }).bytes, vi.fn())).toEqual({
      ok: false,
      reason: "unreadable",
    });
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
