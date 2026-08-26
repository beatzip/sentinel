export type ApproximateCapsule = {
  name: string;
  group_id: number;
  radius: number;
  start: { x: number; y: number; z: number };
  end: { x: number; y: number; z: number };
};

export type ApproximateSpatialRecord = {
  record_type: string;
  tick: number;
  round: number;
  player_id: number;
  status: "available" | "unavailable";
  usage_scope: string;
  evidence_allowed: boolean;
  source: "generic_fallback" | "exact_demo" | "unresolved";
  confidence: "exact" | "approximate" | "unavailable";
  hitboxes: {
    observed_duck_amount?: number | null;
    capsules: ApproximateCapsule[];
  };
};

export function functionalRecordsAtTick(records: ApproximateSpatialRecord[], tick: number) {
  return records.filter(
    (record) =>
      record.record_type === "player_spatial_approximate" &&
      record.tick === tick &&
      record.status === "available" &&
      record.usage_scope === "exploratory_functional" &&
      record.evidence_allowed === false &&
      record.source === "generic_fallback" &&
      record.confidence === "approximate" &&
      record.hitboxes.capsules.length > 0,
  );
}
