import { describe, expect, it } from "vitest";
import { functionalRecordsAtTick, type ApproximateSpatialRecord } from "./approximateSpatial";

const record: ApproximateSpatialRecord = {
  record_type: "player_spatial_approximate",
  tick: 64,
  round: 1,
  player_id: 7,
  status: "available",
  usage_scope: "exploratory_functional",
  evidence_allowed: false,
  source: "generic_fallback",
  confidence: "approximate",
  hitboxes: { observed_duck_amount: 0.5, capsules: [{ name: "head", group_id: 1, radius: 4, start: { x: 1, y: 2, z: 3 }, end: { x: 2, y: 2, z: 3 } }] },
};

describe("functionalRecordsAtTick", () => {
  it("accepts only explicit non-evidentiary generic fallback records at the active tick", () => {
    const evidenceLike = { ...record, player_id: 8, evidence_allowed: true };
    expect(functionalRecordsAtTick([record, evidenceLike], 64)).toEqual([record]);
    expect(functionalRecordsAtTick([record], 65)).toEqual([]);
  });
});
