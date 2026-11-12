import { beforeEach, describe, expect, it, vi } from "vitest";
import type { RaceSources } from "../net/races-api";
import { useRaces } from "../store/races";

const answer: RaceSources = { enabled: true, sources: [] };
const fetchRaceSources = vi.fn(
  (_force?: boolean) => new Promise<RaceSources>((r) => setTimeout(() => r(answer), 5)),
);
vi.mock("../net/races-api", () => ({
  fetchRaceSources: (force?: boolean) => fetchRaceSources(force),
  fetchPicks: vi.fn(),
  fetchRaceSettings: vi.fn(),
  savePick: vi.fn(),
  deletePick: vi.fn(),
}));

const { loadSources, SOURCES_EVERY_MS, sourcesStale } = await import("./hooks");

describe("one copy of the schedule for the Courses and home pages", () => {
  beforeEach(() => {
    fetchRaceSources.mockClear();
    useRaces.getState().update({ sources: null, sourcesAt: 0, sourcesError: null });
  });

  it("shares a request on its way instead of sending another", async () => {
    await Promise.all([loadSources(), loadSources(), loadSources()]);
    expect(fetchRaceSources).toHaveBeenCalledTimes(1);
    expect(useRaces.getState().sources).toBe(answer);
    expect(useRaces.getState().sourcesLoading).toBe(false);
    // « Actualiser » asks for a new reading even then
    await Promise.all([loadSources(), loadSources(true)]);
    expect(fetchRaceSources).toHaveBeenCalledTimes(3);
    expect(fetchRaceSources).toHaveBeenLastCalledWith(true);
  });

  it("asks again only once the copy is five minutes old", () => {
    const now = 1_000_000_000;
    expect(sourcesStale(0, now)).toBe(true);
    expect(sourcesStale(now - SOURCES_EVERY_MS + 1, now)).toBe(false);
    expect(sourcesStale(now - SOURCES_EVERY_MS, now)).toBe(true);
  });
});
