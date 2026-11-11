import type { TranslationKey } from "../../i18n";
import { useT } from "../../i18n";

/** Placeholder shown while a widget has nothing to display. */
export function Empty({ textKey = "widget.noPlayer" }: { textKey?: TranslationKey }) {
  const { t } = useT();
  return (
    <div className="w-empty">
      <span>{t(textKey)}</span>
    </div>
  );
}
