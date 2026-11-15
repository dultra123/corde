import { type ReactNode, useState } from "react";

/**
 * The fields of the Studio's panels (the properties on the right, the format): a label tied to
 * its control by an id, the same look everywhere (`styles/studio.css`).
 */

let uid = 0;
export function fieldId(key: string): string {
  uid += 1;
  return `ins-${key}-${uid}`;
}

export function Field({ label, children, id }: { label: string; children: ReactNode; id: string }) {
  return (
    <div className="ins-field">
      <label htmlFor={id}>{label}</label>
      {children}
    </div>
  );
}

export function NumberInput({
  id,
  value,
  min,
  max,
  step = 1,
  onChange,
}: {
  id: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (v: number) => void;
}) {
  return (
    <input
      id={id}
      className="input num"
      type="number"
      value={value}
      min={min}
      max={max}
      step={step}
      onChange={(e) => {
        const v = Number(e.target.value);
        if (e.target.value !== "" && Number.isFinite(v)) onChange(Math.max(min, Math.min(max, v)));
      }}
    />
  );
}

/**
 * A whole number typed freely: the field keeps what is typed, and gives its value as soon as it
 * is one the caller accepts (`onCommit` returns `false` to refuse it). An entry that is not
 * (too small while it is being typed, out of bounds) changes nothing and is marked; when the
 * field is left it shows the value in use again.
 */
export function DraftNumber({
  id,
  value,
  min,
  max,
  disabled = false,
  describedBy,
  onCommit,
  onFocus,
  onBlur,
}: {
  id: string;
  value: number;
  min: number;
  max: number;
  disabled?: boolean;
  describedBy?: string;
  /** `false`: refused (the field stays marked until a value is accepted). */
  onCommit: (v: number) => boolean | undefined;
  onFocus?: () => void;
  onBlur?: () => void;
}) {
  const [text, setText] = useState<string | null>(null);
  const [bad, setBad] = useState(false);
  const shown = text ?? String(value);
  return (
    <input
      id={id}
      className="input num"
      type="number"
      inputMode="numeric"
      value={shown}
      min={min}
      max={max}
      step={1}
      disabled={disabled}
      aria-invalid={bad || undefined}
      aria-describedby={describedBy}
      onFocus={onFocus}
      onChange={(e) => {
        const raw = e.target.value;
        setText(raw);
        const v = Number(raw);
        const ok = raw !== "" && Number.isInteger(v) && v >= min && v <= max;
        setBad(!(ok && onCommit(v) !== false));
      }}
      onBlur={() => {
        setText(null);
        setBad(false);
        onBlur?.();
      }}
      onKeyDown={(e) => {
        // Entrée: the value in use is shown again (what was typed is applied already, or
        // refused) — and nothing else: in a form, the entry of a number does not submit it.
        if (e.key === "Enter") {
          e.preventDefault();
          setText(null);
          setBad(false);
        }
      }}
    />
  );
}
