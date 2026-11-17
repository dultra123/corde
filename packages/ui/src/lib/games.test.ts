import { describe, expect, it } from "vitest";
import { gameName, gameShort, hasSetupEngineer, isExperimentalGame } from "./games";

describe("the games Corde reads", () => {
  it("names the games that have an adapter, and only them", () => {
    expect(gameName("lmu")).toBe("Le Mans Ultimate");
    expect(gameShort("lmu")).toBe("LMU");
    expect(gameName("acc")).toBe("Assetto Corsa Competizione");
    expect(gameShort("acc")).toBe("ACC");
    expect(gameName("ac")).toBe("Assetto Corsa");
    expect(gameShort("ac")).toBe("AC");
    // Planned games, the simulator and the unknown: no name is shown.
    for (const id of ["iracing", "rfactor2", "ams2", "ac_evo", "sim", "unknown"] as const) {
      expect(gameName(id)).toBe("");
      expect(gameShort(id)).toBe("");
    }
    expect(gameName(null)).toBe("");
  });

  it("says which game is not verified yet, and which one the setup engineer knows", () => {
    expect(isExperimentalGame("acc")).toBe(true);
    expect(isExperimentalGame("ac")).toBe(true);
    expect(isExperimentalGame("lmu")).toBe(false);
    expect(hasSetupEngineer("lmu")).toBe(true);
    expect(hasSetupEngineer("sim")).toBe(true);
    expect(hasSetupEngineer("acc")).toBe(false);
    expect(hasSetupEngineer("ac")).toBe(false);
  });
});
