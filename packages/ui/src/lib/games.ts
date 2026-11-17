import type { GameId } from "@corde/protocol";

/**
 * The games Corde reads (ADR 0020). Corde finds the running game by itself; these are the
 * names it shows. A game is listed here once its adapter exists — the others of the
 * protocol are plans.
 */
const NAMES: Partial<Record<GameId, { name: string; short: string }>> = {
  lmu: { name: "Le Mans Ultimate", short: "LMU" },
  acc: { name: "Assetto Corsa Competizione", short: "ACC" },
  ac: { name: "Assetto Corsa", short: "AC" },
};

/** The full name of a game, « » for the simulator and what is not known. */
export function gameName(id: GameId | null | undefined): string {
  return (id && NAMES[id]?.name) || "";
}

/** The short name of a game (a tag in a list), « » when there is none. */
export function gameShort(id: GameId | null | undefined): string {
  return (id && NAMES[id]?.short) || "";
}

/** Read by Corde, but not checked against the running game yet: said wherever it shows. */
export function isExperimentalGame(id: GameId | null | undefined): boolean {
  return id === "acc" || id === "ac";
}

/**
 * The race engineer proposes setup changes for this game. Its knowledge (parameters, setup
 * files) is Le Mans Ultimate's; the simulator plays a Le Mans Ultimate session.
 */
export function hasSetupEngineer(id: GameId | null | undefined): boolean {
  return id === "lmu" || id === "sim" || id === "unknown" || id == null;
}
