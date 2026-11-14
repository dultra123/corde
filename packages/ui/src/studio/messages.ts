import type { ElementSpec } from "../dashdef/catalog";
import { isKeyLabel } from "../dashdef/elements";
import type { Label } from "../dashdef/fields";
import type { DashProblem } from "../dashdef/model";
import type { TranslationKey } from "../i18n";
import type { Lang } from "../store/settings";

/** The message of a refused definition (an import, a save). */
export function problemKey(p: DashProblem): TranslationKey {
  return `studio.problem.${p.code}` as TranslationKey;
}

/** A name of the catalogue: inline FR / EN, or a translation key for the widgets of the PC. */
export function nameOf(
  spec: ElementSpec | undefined,
  lang: Lang,
  t: (k: TranslationKey) => string,
): string {
  if (!spec) return lang === "en" ? "Unknown element" : "Élément inconnu";
  return textOf(spec.name, lang, t, isKeyLabel(spec));
}

export function textOf(
  l: Label,
  lang: Lang,
  t: (k: TranslationKey) => string,
  isKey = false,
): string {
  const s = lang === "en" ? l.en : l.fr;
  return isKey ? t(s as TranslationKey) : s;
}
