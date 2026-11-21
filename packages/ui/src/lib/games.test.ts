import { describe, expect, it } from "vitest";
import { gameName, gameShort, hasSetupEngineer, isExperimentalGame, isPointToPoint } from "./games";

describe("the games Corde reads", () => {
  it("names the games that have an adapter, and only them", () => {
    expect(gameName("lmu")).toBe("Le Mans Ultimate");
    expect(gameShort("lmu")).toBe("LMU");
    expect(gameName("acc")).toBe("Assetto Corsa Competizione");
    expect(gameShort("acc")).toBe("ACC");
    expect(gameName("ac")).toBe("Assetto Corsa");
    expect(gameShort("ac")).toBe("AC");
    expect(gameName("ac_evo")).toBe("Assetto Corsa EVO");
    expect(gameShort("ac_evo")).toBe("AC EVO");
    expect(gameName("ac_rally")).toBe("Assetto Corsa Rally");
    expect(gameShort("ac_rally")).toBe("AC Rally");
    expect(isPointToPoint("ac_rally")).toBe(true);
    expect(isPointToPoint("lmu")).toBe(false);
    expect(isPointToPoint(null)).toBe(false);
    expect(gameName("iracing")).toBe("iRacing");
    expect(gameShort("iracing")).toBe("iRacing");
    // Planned games, the simulator and the unknown: no name is shown.
    for (const id of ["rfactor2", "ams2", "sim", "unknown"] as const) {
      expect(gameName(id)).toBe("");
      expect(gameShort(id)).toBe("");
    }
    expect(gameName(null)).toBe("");
  });

  it("says which game is not verified yet, and which one the setup engineer knows", () => {
    expect(isExperimentalGame("acc")).toBe(true);
    expect(isExperimentalGame("ac")).toBe(true);
    expect(isExperimentalGame("ac_evo")).toBe(true);
    expect(isExperimentalGame("ac_rally")).toBe(true);
    expect(isExperimentalGame("iracing")).toBe(true);
    expect(isExperimentalGame("lmu")).toBe(false);
    expect(hasSetupEngineer("lmu")).toBe(true);
    expect(hasSetupEngineer("sim")).toBe(true);
    expect(hasSetupEngineer("acc")).toBe(false);
    expect(hasSetupEngineer("ac")).toBe(false);
    expect(hasSetupEngineer("ac_evo")).toBe(false);
    expect(hasSetupEngineer("iracing")).toBe(false);
  });
});
