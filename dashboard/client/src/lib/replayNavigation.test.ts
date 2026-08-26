import { describe, expect, it } from "vitest";
import { matchingPlayerIds, nextObservedCrouchFrame } from "./replayNavigation";

const frames = [
  { players: [{ steam_id: 1, name: "Alpha", duck_amount: 0 }] },
  { players: [{ steam_id: 1, name: "Alpha", duck_amount: 0.6 }, { steam_id: 2, name: "Bravo", duck_amount: 0 }] },
  { players: [{ steam_id: 1, name: "Alpha", duck_amount: 0 }] },
];

describe("replay navigation", () => {
  it("matches players by case-insensitive query without changing source ordering", () => {
    expect(matchingPlayerIds(frames[1].players, "brA")).toEqual([2]);
    expect(matchingPlayerIds(frames[1].players, "")).toEqual([1, 2]);
  });

  it("moves only to recorded crouch frames for the selected player", () => {
    expect(nextObservedCrouchFrame(frames, 1, 0, 1)).toBe(1);
    expect(nextObservedCrouchFrame(frames, 1, 2, -1)).toBe(1);
    expect(nextObservedCrouchFrame(frames, 2, 0, 1)).toBeNull();
  });
});
