import type { Label } from "./fields";

/**
 * The catalogue of elements a dashboard can hold (ADR 0015): name FR / EN, default and minimum
 * size on the grid, the settings it takes. The drawing is in `elements.tsx`; the widgets of the
 * PC are added from their own registry (`widgetSpecs`).
 */

export type PropKind =
  | { kind: "field"; fraction?: boolean }
  | { kind: "text"; max: number }
  | { kind: "number"; min: number; max: number; step: number }
  | { kind: "bool" }
  | { kind: "color" }
  | { kind: "choice"; options: { value: string; label: Label }[] };

export interface PropSpec {
  key: string;
  label: Label;
  type: PropKind;
  default: string | number | boolean;
}

export type ElementGroup = "generic" | "corde" | "pages" | "widgets";

export interface ElementSpec {
  type: string;
  group: ElementGroup;
  name: Label;
  size: { w: number; h: number };
  min: { w: number; h: number };
  props: PropSpec[];
}

const L = (fr: string, en: string): Label => ({ fr, en });

/** Every element has it: a factor on its text sizes. */
export const SCALE_PROP: PropSpec = {
  key: "scale",
  label: L("Taille du texte", "Text size"),
  type: { kind: "number", min: 0.5, max: 3, step: 0.1 },
  default: 1,
};

const FRAMED: PropSpec = {
  key: "framed",
  label: L("Cadre", "Frame"),
  type: { kind: "bool" },
  default: true,
};

const ALIGN: PropSpec = {
  key: "align",
  label: L("Alignement", "Alignment"),
  type: {
    kind: "choice",
    options: [
      { value: "left", label: L("À gauche", "Left") },
      { value: "center", label: L("Centré", "Centred") },
      { value: "right", label: L("À droite", "Right") },
    ],
  },
  default: "left",
};

const COLOR: PropSpec = {
  key: "color",
  label: L("Couleur", "Colour"),
  type: { kind: "color" },
  default: "auto",
};

const LABEL: PropSpec = {
  key: "label",
  label: L("Étiquette (vide : celle de la donnée)", "Label (empty: the data's own)"),
  type: { kind: "text", max: 40 },
  default: "",
};

const CORNER: PropSpec = {
  key: "corner",
  label: L("Coin", "Corner"),
  type: {
    kind: "choice",
    options: [
      { value: "fl", label: L("Avant gauche", "Front left") },
      { value: "fr", label: L("Avant droit", "Front right") },
      { value: "rl", label: L("Arrière gauche", "Rear left") },
      { value: "rr", label: L("Arrière droit", "Rear right") },
    ],
  },
  default: "fl",
};

function block(
  type: string,
  fr: string,
  en: string,
  size: [number, number],
  min: [number, number],
  props: PropSpec[] = [],
): ElementSpec {
  return {
    type: `corde.${type}`,
    group: "corde",
    name: L(fr, en),
    size: { w: size[0], h: size[1] },
    min: { w: min[0], h: min[1] },
    props,
  };
}

function page(type: string, fr: string, en: string): ElementSpec {
  return {
    type: `corde.page.${type}`,
    group: "pages",
    name: L(fr, en),
    size: { w: 24, h: 12 },
    min: { w: 8, h: 4 },
    props: [],
  };
}

export const ELEMENTS: readonly ElementSpec[] = [
  // ---- generic
  {
    type: "value",
    group: "generic",
    name: L("Valeur", "Value"),
    size: { w: 4, h: 3 },
    min: { w: 2, h: 1 },
    props: [
      {
        key: "field",
        label: L("Donnée", "Data"),
        type: { kind: "field" },
        default: "speed_kmh",
      },
      LABEL,
      {
        key: "decimals",
        label: L("Décimales (−1 : celles de la donnée)", "Decimals (−1: the data's own)"),
        type: { kind: "number", min: -1, max: 3, step: 1 },
        default: -1,
      },
      { key: "unit", label: L("Unité", "Unit"), type: { kind: "bool" }, default: true },
      COLOR,
      ALIGN,
      FRAMED,
    ],
  },
  {
    type: "bar",
    group: "generic",
    name: L("Barre / jauge", "Bar / gauge"),
    size: { w: 6, h: 2 },
    min: { w: 1, h: 1 },
    props: [
      {
        key: "field",
        label: L("Donnée (de 0 à 100 %)", "Data (0 to 100 %)"),
        type: { kind: "field", fraction: true },
        default: "throttle",
      },
      LABEL,
      {
        key: "vertical",
        label: L("Verticale", "Vertical"),
        type: { kind: "bool" },
        default: false,
      },
      {
        key: "showValue",
        label: L("Afficher la valeur", "Show the value"),
        type: { kind: "bool" },
        default: true,
      },
      COLOR,
      FRAMED,
    ],
  },
  {
    type: "text",
    group: "generic",
    name: L("Texte", "Text"),
    size: { w: 6, h: 1 },
    min: { w: 1, h: 1 },
    props: [
      { key: "text", label: L("Texte", "Text"), type: { kind: "text", max: 120 }, default: "" },
      { ...COLOR, label: L("Couleur", "Colour") },
      ALIGN,
      { key: "bold", label: L("Gras", "Bold"), type: { kind: "bool" }, default: true },
      { key: "upper", label: L("Majuscules", "Capitals"), type: { kind: "bool" }, default: true },
      { ...FRAMED, default: false },
    ],
  },
  {
    type: "panel",
    group: "generic",
    name: L("Panneau / séparateur", "Panel / divider"),
    size: { w: 6, h: 4 },
    min: { w: 1, h: 1 },
    props: [
      {
        key: "style",
        label: L("Forme", "Shape"),
        type: {
          kind: "choice",
          options: [
            { value: "panel", label: L("Panneau", "Panel") },
            { value: "line-h", label: L("Ligne horizontale", "Horizontal line") },
            { value: "line-v", label: L("Ligne verticale", "Vertical line") },
          ],
        },
        default: "panel",
      },
      { key: "title", label: L("Titre", "Title"), type: { kind: "text", max: 40 }, default: "" },
      { ...COLOR, label: L("Couleur du bord", "Border colour") },
      { key: "fillColor", label: L("Fond", "Fill"), type: { kind: "color" }, default: "auto" },
    ],
  },
  {
    type: "clock",
    group: "generic",
    name: L("Heure / temps de session", "Clock / session time"),
    size: { w: 4, h: 2 },
    min: { w: 2, h: 1 },
    props: [
      {
        key: "mode",
        label: L("Affiche", "Shows"),
        type: {
          kind: "choice",
          options: [
            { value: "local", label: L("Heure de l'appareil", "Time of this device") },
            { value: "left", label: L("Temps restant de la session", "Session time left") },
            { value: "elapsed", label: L("Temps écoulé de la session", "Session time elapsed") },
            { value: "game", label: L("Heure en jeu", "Time of day in the game") },
          ],
        },
        default: "local",
      },
      LABEL,
      COLOR,
      ALIGN,
      FRAMED,
    ],
  },
  // ---- the blocks of the pages of Corde
  block(
    "rev",
    "Rampe de régime et drapeaux",
    "Rev lights and flags",
    [24, 1],
    [6, 1],
    [
      {
        key: "flags",
        label: L("Feux de drapeau", "Flag lights"),
        type: { kind: "bool" },
        default: true,
      },
    ],
  ),
  block(
    "gear",
    "Rapport et vitesse",
    "Gear and speed",
    [6, 7],
    [3, 3],
    [{ key: "speed", label: L("Vitesse", "Speed"), type: { kind: "bool" }, default: true }],
  ),
  block("position", "Position", "Position", [7, 3], [4, 2], [FRAMED]),
  block(
    "laps",
    "Tours (en cours, dernier, meilleur)",
    "Laps (current, last, best)",
    [7, 5],
    [4, 3],
    [FRAMED],
  ),
  block("delta", "Écart au meilleur tour", "Delta to best lap", [8, 3], [4, 2], [FRAMED]),
  block("lap-current", "Tour en cours", "Current lap", [8, 1], [4, 1], [FRAMED]),
  block("sectors", "Secteurs", "Sectors", [8, 1], [3, 1]),
  block("gaps", "Écarts devant / derrière", "Gaps ahead / behind", [8, 1], [4, 1], [FRAMED]),
  block("car-strip", "Bandeau voiture", "Car strip", [24, 2], [8, 2]),
  block("mini-tyres", "Mini-pneus", "Mini tyres", [3, 2], [2, 2], [FRAMED]),
  block("tyre", "Pneu d'un coin", "Tyre of one corner", [9, 5], [5, 3], [CORNER]),
  block(
    "tyres-centre",
    "Fenêtre, répartition, eau et huile",
    "Window, bias, water and oil",
    [4, 10],
    [3, 4],
  ),
  block("brakes", "Freins", "Brakes", [6, 4], [4, 3], [FRAMED]),
  block("fuel", "Carburant", "Fuel", [8, 10], [5, 5]),
  block("energy", "Énergie virtuelle", "Virtual energy", [7, 5], [4, 4]),
  block("hybrid", "Hybride", "Hybrid", [7, 5], [4, 4]),
  block("session", "Session", "Session", [7, 5], [4, 4]),
  block("weather", "Météo", "Weather", [7, 5], [4, 4]),
  block("sector-table", "Tableau des secteurs", "Sector table", [11, 10], [6, 5]),
  block(
    "last-laps",
    "Derniers tours",
    "Last laps",
    [6, 10],
    [4, 4],
    [
      {
        key: "count",
        label: L("Nombre de tours", "Number of laps"),
        type: { kind: "number", min: 1, max: 10, step: 1 },
        default: 5,
      },
    ],
  ),
  block(
    "around",
    "Autour de moi (classement)",
    "Around me (standings)",
    [7, 10],
    [5, 4],
    [
      {
        key: "span",
        label: L("Voitures de chaque côté", "Cars on each side"),
        type: { kind: "number", min: 1, max: 4, step: 1 },
        default: 2,
      },
    ],
  ),
  // ---- whole pages of Corde
  page("course", "Page Course (Corde)", "Race page (Corde)"),
  page("tyres", "Page Pneus & freins (Corde)", "Tyres & brakes page (Corde)"),
  page("energy", "Page Carburant & énergie (Corde)", "Fuel & energy page (Corde)"),
  page("timing", "Page Chronos & écarts (Corde)", "Timing & gaps page (Corde)"),
];

export const ELEMENT_GROUPS: Record<ElementGroup, Label> = {
  generic: L("Éléments", "Elements"),
  corde: L("Blocs du tableau de bord Corde", "Blocks of the Corde dashboard"),
  pages: L("Pages entières", "Whole pages"),
  widgets: L("Widgets du PC", "PC widgets"),
};

const extra = new Map<string, ElementSpec>();

/** Adds element types from elsewhere (the widgets of the PC, `elements.tsx`). */
export function registerElementSpec(spec: ElementSpec): void {
  extra.set(spec.type, spec);
}

const BY_TYPE = new Map(ELEMENTS.map((e) => [e.type, e]));

export function getElementSpec(type: string): ElementSpec | undefined {
  return BY_TYPE.get(type) ?? extra.get(type);
}

export function listElementSpecs(): ElementSpec[] {
  return [...ELEMENTS, ...extra.values()];
}

/** The settings of an element with the defaults of its type filled in. */
export function propsWithDefaults(
  type: string,
  props: Record<string, unknown>,
): Record<string, unknown> {
  const spec = getElementSpec(type);
  const out: Record<string, unknown> = { scale: SCALE_PROP.default };
  for (const p of spec?.props ?? []) out[p.key] = p.default;
  return { ...out, ...props };
}

export function numProp(
  props: Record<string, unknown>,
  key: string,
  fallback: number,
  min = -Infinity,
  max = Infinity,
): number {
  const v = props[key];
  return typeof v === "number" && Number.isFinite(v) ? Math.max(min, Math.min(max, v)) : fallback;
}

export function strProp(props: Record<string, unknown>, key: string, fallback: string): string {
  const v = props[key];
  return typeof v === "string" ? v : fallback;
}

export function boolProp(props: Record<string, unknown>, key: string, fallback: boolean): boolean {
  const v = props[key];
  return typeof v === "boolean" ? v : fallback;
}
