import type { LapRecord, Standing, TelemetryFrame } from "@corde/protocol";
import type { ReactNode } from "react";
import { type DashFlags, NO_FLAGS } from "./model";

/** What every page receives, refreshed a few times a second (`live.ts`). */
export interface PageProps {
  frame: TelemetryFrame | null;
  laps: LapRecord[];
  standings: Standing[];
  flags: DashFlags;
  /** The page is the one on screen (the others keep their fast values still). */
  active: boolean;
  /** What to say in place of the data while there is none (connecting, waiting for the game…). */
  idle: string | null;
}

export const EMPTY_PAGE: PageProps = {
  frame: null,
  laps: [],
  standings: [],
  flags: NO_FLAGS,
  active: true,
  idle: null,
};

/** A small spaced capital label. */
export function Lab({ children, className }: { children: ReactNode; className?: string }) {
  return <span className={`dlab${className ? ` ${className}` : ""}`}>{children}</span>;
}

/** A level 0..1 (fuel, energy, wear), the colour by class. */
export function Level({ fraction, tone }: { fraction: number | null; tone?: string }) {
  const f =
    fraction === null || !Number.isFinite(fraction) ? 0 : Math.max(0, Math.min(1, fraction));
  return (
    <span className={`dlevel${tone ? ` is-${tone}` : ""}`} aria-hidden="true">
      <i style={{ transform: `scaleX(${f})` }} />
    </span>
  );
}

/** The title of a page and, on its right, one line of context. */
export function PageHead({ title, info }: { title: string; info?: ReactNode }) {
  return (
    <header className="dhead">
      <h2 className="dhead-title">{title}</h2>
      {info && <span className="dhead-info dlab">{info}</span>}
    </header>
  );
}

/** A label and its value on one line. */
export function Row({
  label,
  children,
  className,
}: {
  label: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`drow${className ? ` ${className}` : ""}`}>
      <Lab>{label}</Lab>
      <span className="num dval">{children}</span>
    </div>
  );
}
