import type { GameId } from "@corde/protocol";
import { gameTraits } from "../lib/games";
import type { Lang } from "../store/settings";
import {
  BUILTIN_ID,
  DASH_VERSION,
  type DashCondition,
  type DashDefinition,
  type DashElement,
  type DashPageDef,
  DEFAULT_COLS,
  DEFAULT_ROWS,
  DEFAULT_THEME,
  PRESETS,
  presetFormat,
} from "./model";

/**
 * The dashboard shipped with Corde (the four pages of 1.1.0, each one element « page entière »:
 * drawn exactly as before) and the models a new dashboard can start from.
 */

type Spot = [
  type: string,
  x: number,
  y: number,
  w: number,
  h: number,
  props?: Record<string, unknown>,
  show?: DashCondition,
];

function els(spots: Spot[]): DashElement[] {
  return spots.map(([type, x, y, w, h, props, show], i) => ({
    id: `e${i + 1}`,
    type,
    x,
    y,
    w,
    h,
    props: props ?? {},
    ...(show ? { show } : {}),
  }));
}

function pg(id: string, name: string, spots: Spot[], rows = DEFAULT_ROWS): DashPageDef {
  return { id, name, cols: DEFAULT_COLS, rows, elements: els(spots) };
}

/** Page ids of the dashboard « Corde »: the names 1.1.0 remembered (`corde.dash.page`). */
export const CORDE_PAGES = ["course", "tyres", "energy", "timing"] as const;

const CORDE_NAMES: Record<Lang, Record<(typeof CORDE_PAGES)[number], string>> = {
  fr: {
    course: "Course",
    tyres: "Pneus & freins",
    energy: "Carburant & énergie",
    timing: "Chronos & écarts",
  },
  en: { course: "Race", tyres: "Tyres & brakes", energy: "Fuel & energy", timing: "Timing & gaps" },
};

/**
 * The pages of « Corde » for a game: a page the game publishes nothing for is left out (no
 * « Pneus & freins » for a game that gives neither while driving). All of them without a game.
 */
export function cordePages(game?: GameId | null): (typeof CORDE_PAGES)[number][] {
  const traits = gameTraits(game);
  return CORDE_PAGES.filter((p) => p !== "tyres" || traits.tyres || traits.brakes);
}

export function cordeDefinition(lang: Lang = "fr", game?: GameId | null): DashDefinition {
  return {
    v: DASH_VERSION,
    id: BUILTIN_ID,
    name: "Corde",
    format: { preset: "fill", ...PRESETS.fill },
    theme: { ...DEFAULT_THEME },
    pages: cordePages(game).map((p) =>
      pg(p, CORDE_NAMES[lang][p], [[`corde.page.${p}`, 0, 0, DEFAULT_COLS, DEFAULT_ROWS]]),
    ),
    updated_ms: 0,
  };
}

export const CORDE: DashDefinition = cordeDefinition("fr");

export function isBuiltin(def: Pick<DashDefinition, "id">): boolean {
  return def.id === BUILTIN_ID;
}

// ---------------------------------------------------------------- models

export type TemplateId = "blank" | "corde-blocks" | "endurance" | "moza-wheel";

export const TEMPLATE_IDS: TemplateId[] = ["blank", "corde-blocks", "endurance", "moza-wheel"];

/** The rows of a page drawn for the strip of a wheel: 24 × 8 roughly square cells. */
const STRIP_ROWS = 8;

function base(name: string, pages: DashPageDef[]): Omit<DashDefinition, "id"> {
  return {
    v: DASH_VERSION,
    name,
    format: { preset: "phone-landscape", ...PRESETS["phone-landscape"] },
    theme: { ...DEFAULT_THEME },
    pages,
    updated_ms: 0,
  };
}

/** A model, without its id (`ops.newDefinition` gives one). */
export function template(id: TemplateId, lang: Lang = "fr"): Omit<DashDefinition, "id"> {
  const fr = lang === "fr";
  switch (id) {
    case "blank":
      return base(fr ? "Nouveau tableau de bord" : "New dashboard", [
        pg("p1", fr ? "Page 1" : "Page 1", []),
      ]);
    case "corde-blocks":
      return base(fr ? "Corde en blocs" : "Corde in blocks", [
        pg("course", CORDE_NAMES[lang].course, [
          ["corde.rev", 0, 0, 24, 1],
          ["corde.position", 0, 1, 8, 3],
          ["corde.laps", 0, 4, 8, 6],
          ["corde.gear", 8, 1, 7, 9],
          ["corde.delta", 15, 1, 9, 3],
          ["corde.lap-current", 15, 4, 9, 2],
          ["corde.sectors", 15, 6, 9, 2],
          ["corde.gaps", 15, 8, 9, 2],
          ["corde.car-strip", 0, 10, 24, 2],
        ]),
        pg("tyres", CORDE_NAMES[lang].tyres, [
          ["corde.tyre", 0, 0, 10, 6, { corner: "fl" }],
          ["corde.tyres-centre", 10, 0, 4, 12],
          ["corde.tyre", 14, 0, 10, 6, { corner: "fr" }],
          ["corde.tyre", 0, 6, 10, 6, { corner: "rl" }],
          ["corde.tyre", 14, 6, 10, 6, { corner: "rr" }],
        ]),
        pg("energy", CORDE_NAMES[lang].energy, [
          ["corde.fuel", 0, 0, 9, 12],
          ["corde.energy", 9, 0, 7, 6],
          ["corde.hybrid", 9, 6, 7, 6],
          ["corde.session", 16, 0, 8, 6],
          ["corde.weather", 16, 6, 8, 6],
        ]),
        pg("timing", CORDE_NAMES[lang].timing, [
          ["corde.sector-table", 0, 0, 11, 12],
          ["corde.last-laps", 11, 0, 6, 12],
          ["corde.around", 17, 0, 7, 12],
        ]),
      ]);
    case "endurance":
      return base(fr ? "Endurance" : "Endurance", [
        pg("p1", fr ? "Relais" : "Stint", [
          ["corde.rev", 0, 0, 24, 1, { flags: true }],
          ["corde.gear", 0, 1, 6, 8],
          ["corde.delta", 6, 1, 10, 3],
          ["value", 16, 1, 4, 3, { field: "position" }],
          ["value", 20, 1, 4, 3, { field: "laps_left" }],
          ["value", 6, 4, 5, 2, { field: "lap_current" }],
          ["value", 11, 4, 5, 2, { field: "lap_best" }],
          ["value", 16, 4, 4, 2, { field: "gap_ahead" }],
          ["value", 20, 4, 4, 2, { field: "gap_behind" }],
          ["bar", 6, 6, 9, 1, { field: "throttle", showValue: false, framed: false }],
          ["bar", 15, 6, 9, 1, { field: "brake", showValue: false, framed: false }],
          ["value", 6, 7, 5, 2, { field: "fuel_l" }],
          ["value", 11, 7, 5, 2, { field: "fuel_laps" }],
          ["value", 16, 7, 4, 2, { field: "virtual_energy" }],
          ["value", 20, 7, 4, 2, { field: "session_left" }],
          ["value", 0, 9, 3, 3, { field: "tc" }],
          ["value", 3, 9, 3, 3, { field: "abs" }],
          ["value", 6, 9, 4, 3, { field: "brake_bias" }],
          ["value", 10, 9, 3, 3, { field: "motor_map" }],
          ["corde.mini-tyres", 13, 9, 4, 3],
          ["value", 17, 9, 3, 3, { field: "track_c" }],
          ["clock", 20, 9, 4, 3, { mode: "local" }],
        ]),
        pg("p2", fr ? "Stratégie" : "Strategy", [
          ["corde.fuel", 0, 0, 9, 12],
          ["corde.energy", 9, 0, 7, 6],
          ["corde.session", 9, 6, 7, 6],
          ["corde.around", 16, 0, 8, 7],
          ["corde.weather", 16, 7, 8, 5],
        ]),
      ]);
    case "moza-wheel": {
      // For the screen of a MOZA wheel: plain elements (and the gear block, which has a plain
      // form there) over data that screen is given, so that « Exporter pour le volant MOZA »
      // (`mzdash.ts`) writes all of it.
      const corners = (field: string, x: number): Spot[] => [
        ["value", x, 0, 4, 4, { field: `${field}_fl`, align: "center" }],
        ["value", x + 4, 0, 4, 4, { field: `${field}_fr`, align: "center" }],
        ["value", x, 4, 4, 4, { field: `${field}_rl`, align: "center" }],
        ["value", x + 4, 4, 4, 4, { field: `${field}_rr`, align: "center" }],
      ];
      const pages = [
        pg(
          "course",
          CORDE_NAMES[lang].course,
          [
            ["rpm", 0, 0, 24, 1],
            ["value", 0, 1, 4, 3, { field: "speed_kmh" }],
            ["value", 4, 1, 4, 3, { field: "position" }],
            ["value", 0, 4, 8, 2, { field: "lap_current" }],
            ["value", 0, 6, 4, 2, { field: "lap_last" }],
            ["value", 4, 6, 4, 2, { field: "lap_best" }],
            ["corde.gear", 8, 1, 8, 5, { speed: false }],
            ["value", 8, 6, 8, 2, { field: "delta_best", align: "center" }],
            ["value", 16, 1, 4, 3, { field: "gap_ahead" }],
            ["value", 20, 1, 4, 3, { field: "gap_behind" }],
            ["value", 16, 4, 4, 2, { field: "fuel_l" }],
            ["value", 20, 4, 4, 2, { field: "fuel_laps" }],
            ["value", 16, 6, 2, 2, { field: "tc", align: "center" }],
            ["value", 18, 6, 2, 2, { field: "abs", align: "center" }],
            ["value", 20, 6, 4, 2, { field: "brake_bias" }],
            ["alert", 8, 1, 8, 2, { text: fr ? "LIMITEUR" : "PIT LIMITER" }, { when: "limiter" }],
          ],
          STRIP_ROWS,
        ),
        pg(
          "tyres",
          CORDE_NAMES[lang].tyres,
          [
            ...corners("tyre_temp", 0),
            ...corners("tyre_pressure", 8),
            ...corners("brake_temp", 16),
          ],
          STRIP_ROWS,
        ),
        pg(
          "energy",
          CORDE_NAMES[lang].energy,
          [
            ["bar", 0, 0, 12, 2, { field: "fuel_fraction" }],
            ["value", 0, 2, 4, 3, { field: "fuel_l" }],
            ["value", 4, 2, 4, 3, { field: "fuel_per_lap" }],
            ["value", 8, 2, 4, 3, { field: "fuel_laps" }],
            ["bar", 12, 0, 12, 2, { field: "virtual_energy" }],
            ["value", 12, 2, 4, 3, { field: "battery" }],
            ["value", 16, 2, 4, 3, { field: "motor_map" }],
            ["value", 20, 2, 4, 3, { field: "lap_number" }],
            ["clock", 0, 5, 6, 3, { mode: "left" }],
            ["value", 6, 5, 3, 3, { field: "track_c" }],
            ["value", 9, 5, 3, 3, { field: "air_c" }],
            ["value", 12, 5, 4, 3, { field: "water_c" }],
            ["value", 16, 5, 4, 3, { field: "position" }],
            ["value", 20, 5, 4, 3, { field: "cars" }],
          ],
          STRIP_ROWS,
        ),
      ];
      return {
        ...base(fr ? "Volant MOZA" : "MOZA wheel", pages),
        format: presetFormat("wheel-moza"),
      };
    }
  }
}
