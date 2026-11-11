/** CSV export of the channels loaded in the browser (comma separated, dot decimals, UTF-8). */

export interface CsvColumn {
  name: string;
  unit: string;
  values: ArrayLike<number | null | undefined>;
}

function cell(v: number | null | undefined): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "";
  // keep integers short, trim float noise
  return Number.isInteger(v) ? String(v) : String(Number(v.toPrecision(7)));
}

function header(c: CsvColumn): string {
  const h = c.unit ? `${c.name} [${c.unit}]` : c.name;
  return /[",\n]/.test(h) ? `"${h.replaceAll('"', '""')}"` : h;
}

/** Builds the file text: one header row (`name [unit]`), one row per sample. */
export function buildCsv(columns: readonly CsvColumn[]): string {
  if (columns.length === 0) return "";
  const rows = Math.max(...columns.map((c) => c.values.length));
  const lines: string[] = [columns.map(header).join(",")];
  for (let i = 0; i < rows; i += 1) {
    lines.push(columns.map((c) => cell(c.values[i])).join(","));
  }
  return `${lines.join("\n")}\n`;
}

/** `corde-<session>-tour-<n>.csv`, safe for every file system. */
export function csvFileName(sessionId: string, lap: number, lang: "fr" | "en"): string {
  const safe = sessionId.replace(/[^A-Za-z0-9._-]+/g, "-");
  return `corde-${safe}-${lang === "fr" ? "tour" : "lap"}-${lap}.csv`;
}

/** Triggers a browser download of `text` as `fileName`. */
export function downloadText(fileName: string, text: string, mime = "text/csv"): void {
  const blob = new Blob([text], { type: `${mime};charset=utf-8` });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = fileName;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
