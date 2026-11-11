import type { ComponentType } from "react";
import type { TranslationKey } from "../i18n";

export interface WidgetProps {
  /** Instance id (the `WidgetPlacement.id`). */
  id: string;
  config: Record<string, unknown>;
  setConfig(patch: Record<string, unknown>): void;
  editing: boolean;
}

export interface WidgetSize {
  w: number;
  h: number;
}

/** A config option rendered by the widget chrome in edit mode (a `<select>`). */
export interface WidgetConfigField {
  key: string;
  labelKey: TranslationKey;
  options: Array<{ value: string; labelKey: TranslationKey }>;
}

export interface WidgetDef {
  /** Kind, stored in `WidgetPlacement.kind`. */
  id: string;
  titleKey: TranslationKey;
  minW: number;
  minH: number;
  /** Size on the 12-column desktop grid. */
  defaultSize: WidgetSize;
  /** Size on the 4-column mobile grid (defaults to `defaultSize` clamped to 4 columns). */
  mobileSize?: WidgetSize;
  component: ComponentType<WidgetProps>;
  defaultConfig?: Record<string, unknown>;
  configFields?: WidgetConfigField[];
}

const REGISTRY = new Map<string, WidgetDef>();

export function registerWidget(def: WidgetDef): WidgetDef {
  if (REGISTRY.has(def.id)) throw new Error(`widget "${def.id}" registered twice`);
  if (def.minW < 1 || def.minH < 1) throw new Error(`widget "${def.id}": min size must be ≥ 1`);
  if (def.defaultSize.w < def.minW || def.defaultSize.h < def.minH) {
    throw new Error(`widget "${def.id}": default size smaller than its minimum`);
  }
  REGISTRY.set(def.id, def);
  return def;
}

export function getWidget(id: string): WidgetDef | undefined {
  return REGISTRY.get(id);
}

export function listWidgets(): WidgetDef[] {
  return [...REGISTRY.values()];
}

/** Reads a string option from a widget config with a fallback. */
export function configString(
  config: Record<string, unknown>,
  key: string,
  fallback: string,
): string {
  const v = config[key];
  return typeof v === "string" ? v : fallback;
}

/** Reads a number; also accepts numeric strings since `<select>` values are strings. */
export function configNumber(
  config: Record<string, unknown>,
  key: string,
  fallback: number,
): number {
  const v = config[key];
  if (typeof v === "number" && Number.isFinite(v)) return v;
  if (typeof v === "string" && v.trim() !== "" && Number.isFinite(Number(v))) return Number(v);
  return fallback;
}
