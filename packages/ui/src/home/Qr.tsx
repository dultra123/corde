import qrcode from "qrcode-generator";
import { useMemo } from "react";

/** Quiet zone around the code, in modules (the QR specification asks for 4). */
const QUIET = 4;

/** The modules of the QR code of `text` (`true` = dark), error correction level M. */
export function qrMatrix(text: string): boolean[][] {
  const qr = qrcode(0, "M");
  qr.addData(text);
  qr.make();
  const n = qr.getModuleCount();
  return Array.from({ length: n }, (_, row) =>
    Array.from({ length: n }, (_, col) => qr.isDark(row, col)),
  );
}

/**
 * The dark modules as one SVG path (horizontal runs merged), in a `size × size` user space that
 * includes the quiet zone.
 */
export function qrPath(text: string): { size: number; d: string } {
  const m = qrMatrix(text);
  const parts: string[] = [];
  m.forEach((cells, row) => {
    let col = 0;
    while (col < cells.length) {
      if (!cells[col]) {
        col += 1;
        continue;
      }
      let end = col;
      while (end < cells.length && cells[end]) end += 1;
      parts.push(`M${col + QUIET} ${row + QUIET}h${end - col}v1h-${end - col}z`);
      col = end;
    }
  });
  return { size: m.length + 2 * QUIET, d: parts.join("") };
}

/**
 * A QR code as inline SVG: dark modules on a light tile whatever the theme — inverted codes are
 * not read reliably by phone cameras.
 */
export function Qr({ value, label }: { value: string; label: string }) {
  const { size, d } = useMemo(() => qrPath(value), [value]);
  return (
    <svg
      className="qr"
      viewBox={`0 0 ${size} ${size}`}
      role="img"
      aria-label={label}
      shapeRendering="crispEdges"
    >
      <rect width={size} height={size} rx="1.5" fill="#e9edf2" />
      <path d={d} fill="#0b0e12" />
    </svg>
  );
}
