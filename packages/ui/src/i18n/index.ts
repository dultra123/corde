import { type Lang, useSettings } from "../store/settings";
import { en } from "./en";
import { fr, type TranslationKey } from "./fr";

const DICTS: Record<Lang, Record<TranslationKey, string>> = { fr, en };

export type { TranslationKey };

/** Translate outside React (uses the current language from the settings store). */
export function t(key: TranslationKey, params?: Record<string, string | number>): string {
  return translate(useSettings.getState().lang, key, params);
}

export function translate(
  lang: Lang,
  key: TranslationKey,
  params?: Record<string, string | number>,
): string {
  let s: string = DICTS[lang][key] ?? fr[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) s = s.replaceAll(`{${k}}`, String(v));
  }
  return s;
}

/**
 * `n` followed by the right word: `1 course` / `2 courses`. French keeps the singular for 0
 * (`0 course`), English does not (`0 races`).
 */
export function countOf(lang: Lang, n: number, one: TranslationKey, many: TranslationKey): string {
  const plural = lang === "fr" ? n >= 2 : n !== 1;
  return `${n} ${translate(lang, plural ? many : one)}`;
}

/** Hook: returns a `t` bound to the current language (re-renders on language change). */
export function useT(): {
  t: (key: TranslationKey, params?: Record<string, string | number>) => string;
  nOf: (n: number, one: TranslationKey, many: TranslationKey) => string;
  lang: Lang;
} {
  const lang = useSettings((s) => s.lang);
  return {
    lang,
    t: (key, params) => translate(lang, key, params),
    nOf: (n, one, many) => countOf(lang, n, one, many),
  };
}
