import { strToU8, zipSync } from "fflate";
import { describe, expect, it, vi } from "vitest";
import {
  buildArchive,
  DEFINITION_ENTRY,
  type ImportTools,
  importFile,
  MAX_ENTRIES,
  readArchive,
  remapAssets,
} from "./archive";
import { exportText } from "./io";
import { sniff } from "./media";
import { type DashDefinition, validateDefinition } from "./model";

/**
 * Sharing with the pictures (ADR 0016): a `.cordedash` is a ZIP made and read in the browser.
 * Exported then imported, a dashboard comes back the same; an archive made to harm (too many
 * entries, an entry that expands without end, a name that is a path, a file that is not what
 * its name says) is refused. Every file here is made by the test.
 */

const id = (n: number, ext: string) => `${n.toString(16).padStart(64, "0")}.${ext}`;
const PNG_ID = id(1, "png");
const MP4_ID = id(2, "mp4");
const GIF_ID = id(3, "gif");

const PNG = new Uint8Array([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 1, 2, 3, 4]);
const MP4 = new Uint8Array([0, 0, 0, 12, ...strToU8("ftypisom"), 0, 0, 0, 8, ...strToU8("mdat")]);
const GIF = strToU8("GIF89a\u0001\u0000\u0001\u0000");

function dash(): DashDefinition {
  return {
    v: 1,
    id: "logo",
    name: "Logo & fond",
    author: "pilote",
    format: { preset: "phone-landscape", width: 852, height: 393 },
    theme: {
      background: "#000000",
      panel: "#0d1116",
      line: "#1f2731",
      text: "#f2f5f8",
      dim: "#93a0ae",
      accent: "#ffb454",
      font: "wide",
    },
    pages: [
      {
        id: "p1",
        name: "Course",
        cols: 24,
        rows: 12,
        background: { asset: MP4_ID, fit: "cover", opacity: 0.6 },
        elements: [
          {
            id: "e1",
            type: "image",
            x: 0,
            y: 0,
            w: 4,
            h: 4,
            props: { asset: PNG_ID, fit: "contain" },
          },
          {
            id: "e2",
            type: "value",
            x: 4,
            y: 0,
            w: 6,
            h: 3,
            props: { field: "fuel_laps" },
            thresholds: [{ op: "lt", value: 2, color: "#ff5a5a" }],
            style: { bg: "#112233", radius: 6 },
          },
          {
            id: "e3",
            type: "alert",
            x: 0,
            y: 9,
            w: 10,
            h: 2,
            props: { text: "LIMITEUR" },
            show: { when: "limiter" },
          },
        ],
      },
    ],
    updated_ms: 7,
  };
}

const files: Record<string, Uint8Array> = { [PNG_ID]: PNG, [MP4_ID]: MP4 };
const read = async (i: string) => files[i] ?? null;

/** Tools that keep each media as it is and give it back its own id. */
function identity(): ImportTools & { sent: Uint8Array[] } {
  const sent: Uint8Array[] = [];
  return {
    sent,
    prepare: async (b) => {
      const ext = sniff(b);
      return ext
        ? { ok: true, bytes: b, ext, kind: ext === "mp4" || ext === "webm" ? "video" : "image" }
        : { ok: false, reason: "type" };
    },
    upload: async (b) => {
      sent.push(b);
      const found = Object.entries(files).find(([, v]) => v.length === b.length);
      return found ? { ok: true, id: found[0] } : { ok: false, reason: "refused" };
    },
  };
}

describe("export then import", () => {
  it("gives the same dashboard, its media with it", async () => {
    const built = await buildArchive(dash(), read);
    expect(built.missing).toEqual([]);
    expect(built.hasVideo).toBe(true);
    const r = readArchive(built.bytes);
    expect(r.ok).toBe(true);
    if (!r.ok) return;
    expect([...r.media.keys()].sort()).toEqual([PNG_ID, MP4_ID].sort());
    expect(r.media.get(PNG_ID)).toEqual(PNG);
    const tools = identity();
    const back = await importFile(built.bytes, [], tools);
    expect(back.ok).toBe(true);
    if (!back.ok) return;
    expect(back.media).toBe(2);
    expect(back.hasVideo).toBe(true);
    expect(tools.sent).toHaveLength(2);
    expect({ ...back.def, updated_ms: 7 }).toEqual(dash());
  });

  it("a media sent again under a new id is cited by it; one not found is left out", async () => {
    const built = await buildArchive(dash(), async (i) => (i === PNG_ID ? PNG : null));
    expect(built.missing).toEqual([MP4_ID]);
    expect(built.hasVideo).toBe(false);
    const NEW = id(9, "png");
    const back = await importFile(built.bytes, ["logo"], {
      prepare: identity().prepare,
      upload: async () => ({ ok: true, id: NEW }),
    });
    expect(back.ok).toBe(true);
    if (!back.ok) return;
    expect(back.def.id).not.toBe("logo");
    expect(back.def.pages[0]?.elements[0]?.props.asset).toBe(NEW);
    // Not in the file: kept as an id, shown as an empty place.
    expect(back.def.pages[0]?.background?.asset).toBe(MP4_ID);
    expect(validateDefinition(back.def).ok).toBe(true);
  });

  it("the pictures go through the same preparation as a file picked on the PC", async () => {
    const built = await buildArchive(dash(), read);
    const prepare = vi.fn(identity().prepare);
    await importFile(built.bytes, [], { prepare, upload: identity().upload });
    expect(prepare).toHaveBeenCalledTimes(2);
    const refused = await importFile(built.bytes, [], {
      prepare: async () => ({ ok: false, reason: "too-large" }),
      upload: identity().upload,
    });
    expect(refused).toEqual({ ok: false, problem: { code: "media", at: expect.any(String) } });
    const offline = await importFile(built.bytes, [], {
      prepare: identity().prepare,
      upload: async () => ({ ok: false, reason: "pc-only" }),
    });
    expect(offline).toEqual({ ok: false, upload: "pc-only" });
  });

  it("still reads a `.cordedash.json` of 1.2", async () => {
    const d = dash();
    const old: DashDefinition = {
      ...d,
      theme: { ...d.theme, font: undefined },
      pages: [{ id: "p1", name: "Course", cols: 24, rows: 12, elements: [] }],
    };
    delete old.theme.font;
    const r = await importFile(strToU8(exportText(old)), [], identity());
    expect(r.ok).toBe(true);
    expect(r.ok && r.media).toBe(0);
    expect(await importFile(strToU8("{ nope"), [], identity())).toMatchObject({
      ok: false,
      problem: { code: "not-json" },
    });
  });

  it("swaps the ids of the media, nothing else", () => {
    const d = remapAssets(dash(), new Map([[PNG_ID, GIF_ID]]));
    expect(d.pages[0]?.elements[0]?.props.asset).toBe(GIF_ID);
    expect(d.pages[0]?.background?.asset).toBe(MP4_ID);
    expect(remapAssets(dash(), new Map())).toEqual(dash());
  });
});

function zip(entries: Record<string, Uint8Array>, level: 0 | 9 = 9): Uint8Array {
  const out: Record<string, [Uint8Array, { level: 0 | 9 }]> = {};
  for (const [k, v] of Object.entries(entries)) out[k] = [v, { level }];
  return zipSync(out);
}

const def = () => strToU8(exportText(dash()));

describe("an archive made to harm is refused", () => {
  it("an entry that expands past its bound, even when its header lies", () => {
    const bomb = new Uint8Array(40 * 1024 * 1024);
    bomb.set(PNG);
    const z = zip({ [DEFINITION_ENTRY]: def(), [`media/${PNG_ID}`]: bomb });
    expect(z.length).toBeLessThan(200 * 1024);
    expect(readArchive(z)).toMatchObject({ ok: false, problem: { code: "media" } });
    // The size written in the local header set to 10 bytes: stopped while reading.
    const lying = z.slice();
    const view = new DataView(lying.buffer);
    for (let o = 0; o + 30 < lying.length; o += 1) {
      if (view.getUint32(o, true) !== 0x04034b50) continue;
      const nameLen = view.getUint16(o + 26, true);
      const name = new TextDecoder().decode(lying.subarray(o + 30, o + 30 + nameLen));
      if (name.startsWith("media/")) view.setUint32(o + 22, 10, true);
    }
    expect(readArchive(lying)).toMatchObject({ ok: false });
  });

  it("a definition too large, too many entries, no definition", () => {
    const big = strToU8(`{"v":1,"pad":"${"x".repeat(600 * 1024)}"}`);
    expect(readArchive(zip({ [DEFINITION_ENTRY]: big }))).toMatchObject({
      ok: false,
      problem: { code: "too-large" },
    });
    const many: Record<string, Uint8Array> = { [DEFINITION_ENTRY]: def() };
    for (let i = 0; i < MAX_ENTRIES; i += 1) many[`notes/${i}.txt`] = strToU8("x");
    expect(readArchive(zip(many))).toMatchObject({ ok: false, problem: { code: "archive" } });
    expect(readArchive(zip({ "other.json": def() }))).toMatchObject({
      ok: false,
      problem: { code: "archive" },
    });
    expect(readArchive(strToU8("not a zip"))).toMatchObject({
      ok: false,
      problem: { code: "archive" },
    });
  });

  it("names other than the expected ones are ignored, never read", () => {
    const r = readArchive(
      zip({
        [DEFINITION_ENTRY]: def(),
        "../../evil.txt": strToU8("x"),
        "media/../../x.png": PNG,
        "media/logo.png": PNG,
        [`media/${id(5, "svg")}`]: strToU8("<svg onload=alert(1)>"),
        [`MEDIA/${PNG_ID}`]: PNG,
      }),
    );
    expect(r.ok).toBe(true);
    expect(r.ok && r.media.size).toBe(0);
  });

  it("a media that is not what its name says", () => {
    const r = readArchive(zip({ [DEFINITION_ENTRY]: def(), [`media/${PNG_ID}`]: GIF }));
    expect(r).toMatchObject({ ok: false, problem: { code: "media" } });
    const svg = readArchive(
      zip({ [DEFINITION_ENTRY]: def(), [`media/${PNG_ID}`]: strToU8("<svg></svg>") }),
    );
    expect(svg).toMatchObject({ ok: false, problem: { code: "media" } });
  });

  it("a definition that breaks the rules, inside an archive", async () => {
    const bad = dash();
    const first = bad.pages[0]?.elements[0];
    if (first) first.props.asset = "https://exemple.org/logo.png";
    const z = zip({ [DEFINITION_ENTRY]: strToU8(JSON.stringify(bad)) });
    // Refused as an address (`props`), before anything else is looked at.
    expect(await importFile(z, [], identity())).toMatchObject({
      ok: false,
      problem: { code: "props" },
    });
  });
});
