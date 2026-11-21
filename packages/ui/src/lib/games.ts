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
  ac_evo: { name: "Assetto Corsa EVO", short: "AC EVO" },
  ac_rally: { name: "Assetto Corsa Rally", short: "AC Rally" },
  iracing: { name: "iRacing", short: "iRacing" },
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
  return id === "acc" || id === "ac" || id === "ac_evo" || id === "ac_rally" || id === "iracing";
}

/**
 * The race engineer proposes setup changes for this game. Its knowledge (parameters, setup
 * files) is Le Mans Ultimate's; the simulator plays a Le Mans Ultimate session.
 */
export function hasSetupEngineer(id: GameId | null | undefined): boolean {
  return id === "lmu" || id === "sim" || id === "unknown" || id == null;
}

/**
 * What a game publishes while driving, as far as the interface of departure is concerned: a
 * widget or a page that would stay empty is left out of what Corde proposes for that game
 * (the pilot can always add it). Everything is taken as published for a game not listed.
 */
export interface GameTraits {
  /** Tyre temperatures, pressures and wear that move while driving. */
  tyres: boolean;
  /** Brake temperatures. */
  brakes: boolean;
  /** The other cars: places, laps, times. */
  standings: boolean;
  /** Gaps to the cars around. */
  gaps: boolean;
  /** Driver aids and their levels (traction control, ABS, engine map). */
  electronics: boolean;
}

const EVERYTHING: GameTraits = {
  tyres: true,
  brakes: true,
  standings: true,
  gaps: true,
  electronics: true,
};

const TRAITS: Partial<Record<GameId, Partial<GameTraits>>> = {
  // The pages of the game name no other driver; the gaps ahead and behind are there.
  acc: { standings: false },
  // The game publishes its own car only, and no level of the driver aids.
  ac: { standings: false, gaps: false, electronics: false },
  // The other cars are only placed on the map; the gaps ahead and behind are there.
  ac_evo: { standings: false },
  // A stage is driven alone, and the cars have no levels of driver aids to show.
  ac_rally: { standings: false, gaps: false, electronics: false },
  // Tyres are only measured at a stop in the pits, brakes never.
  iracing: { tyres: false, brakes: false },
};

export function gameTraits(id: GameId | null | undefined): GameTraits {
  return { ...EVERYTHING, ...(id ? TRAITS[id] : undefined) };
}

/** The games that have an interface of their own (those Corde reads, in the order shown). */
export const PROFILE_GAMES: readonly GameId[] = [
  "lmu",
  "acc",
  "ac",
  "ac_evo",
  "ac_rally",
  "iracing",
];

/**
 * A game whose « circuit » is a stage driven from a start line to a finish line: its map is
 * a line, not a loop.
 */
export function isPointToPoint(id: GameId | null | undefined): boolean {
  return id === "ac_rally";
}

/**
 * The game a profile is kept for: the game itself; the simulator plays Le Mans Ultimate;
 * `null` for what is not a game Corde reads.
 */
export function profileGame(id: GameId | null | undefined): GameId | null {
  if (id === "sim") return "lmu";
  return id && PROFILE_GAMES.includes(id) ? id : null;
}
