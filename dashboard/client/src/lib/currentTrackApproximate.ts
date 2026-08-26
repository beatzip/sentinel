/** Current-track adapter: converts only quarantined generic records into the dashboard's existing functional replay shape. */
import type { ApproximateCapsule, ApproximateSpatialRecord } from "@/lib/approximateSpatial";

const CURRENT_TRACK_URL = "/approximate/current-track-5037-downsampled.json.gz";
const GENERIC_PROFILE_URL = "/approximate/standard-player-19-capsule-generic.json";

type Vec3 = { x: number; y: number; z: number };
type RawCapsule = { id: string; center_world: Vec3; radius: number; height: number; orientation_yaw_deg: number };
type RawRecord = {
  tick: number;
  entity_index: number;
  pawn_character_def_index: number;
  origin: Vec3;
  yaw_deg: number;
  duck_amount: number;
  capsules: RawCapsule[];
  provenance: {
    evidence_allowed: boolean;
    usage_scope: string;
    derivation: string;
    m_hModel_binding: string;
    exact_geometry_claimed: boolean;
    exact_skeleton_claimed: boolean;
    exact_hitboxes_claimed: boolean;
    ag2_bone_matrices_claimed: boolean;
  };
};
type RawDataset = { schema_version: number; demo_sha256: string; map: string; resolution: string; profile_id: string; records: RawRecord[] };
type ProfileCapsule = { id: string; group: string; a: [number, number, number]; b: [number, number, number] };
type GenericProfile = {
  artifact_kind: string;
  profile_id: string;
  confidence: string;
  evidence_allowed: boolean;
  usage_scope: string;
  exact_model_calibration: boolean;
  capsules: ProfileCapsule[];
};

export type CurrentTrackReplay = {
  version: string;
  map: string;
  tick_rate: number;
  replay_mode: string;
  functional_only: true;
  frames: Array<{ tick: number; round: number; players: Array<{ steam_id: number; name: string; team: string; x: number; y: number; z: number; yaw: number; duck_amount: number }>; visible_pairs: [] }>;
  approximate_spatial: ApproximateSpatialRecord[];
};

function validRecord(record: RawRecord) {
  const provenance = record.provenance;
  return record.pawn_character_def_index === 5037
    && record.capsules?.length === 19
    && provenance?.evidence_allowed === false
    && provenance.usage_scope === "exploratory_functional"
    && provenance.derivation === "definition_5037_to_ctm_sas_profile"
    && provenance.m_hModel_binding === "not_used"
    && provenance.exact_geometry_claimed === false
    && provenance.exact_skeleton_claimed === false
    && provenance.exact_hitboxes_claimed === false
    && provenance.ag2_bone_matrices_claimed === false;
}

function groupId(group: string) {
  return { head: 1, neck: 2, torso: 3, pelvis: 4, arm: 5, leg: 6 }[group] ?? 0;
}

function genericEndpoints(capsule: RawCapsule, profileCapsule: ProfileCapsule): Pick<ApproximateCapsule, "start" | "end"> {
  const dx = profileCapsule.b[0] - profileCapsule.a[0];
  const dy = profileCapsule.b[1] - profileCapsule.a[1];
  const dz = profileCapsule.b[2] - profileCapsule.a[2];
  const sourceLength = Math.hypot(dx, dy, dz);
  const lateralLength = Math.hypot(dx, dy);
  if (!sourceLength || !lateralLength) return { start: capsule.center_world, end: capsule.center_world };
  const planarLength = capsule.height * (lateralLength / sourceLength);
  const angle = capsule.orientation_yaw_deg * Math.PI / 180;
  const directionX = ((dx / lateralLength) * Math.cos(angle) - (dy / lateralLength) * Math.sin(angle)) * planarLength;
  const directionY = ((dx / lateralLength) * Math.sin(angle) + (dy / lateralLength) * Math.cos(angle)) * planarLength;
  return {
    start: { x: capsule.center_world.x - directionX / 2, y: capsule.center_world.y - directionY / 2, z: capsule.center_world.z },
    end: { x: capsule.center_world.x + directionX / 2, y: capsule.center_world.y + directionY / 2, z: capsule.center_world.z },
  };
}

async function readGzipJson(response: Response) {
  if ((response.headers.get("content-encoding") ?? "").includes("gzip")) return response.json();
  if (!response.body || !("DecompressionStream" in globalThis)) throw new Error("gzip decompression unavailable");
  return new Response(response.body.pipeThrough(new DecompressionStream("gzip"))).json();
}

export async function loadCurrentTrackApproximateReplay(): Promise<CurrentTrackReplay> {
  const [recordsResponse, profileResponse] = await Promise.all([fetch(CURRENT_TRACK_URL), fetch(GENERIC_PROFILE_URL)]);
  if (!recordsResponse.ok || !profileResponse.ok) throw new Error("current approximate sidecar unavailable");
  const [dataset, profile] = [await readGzipJson(recordsResponse) as RawDataset, await profileResponse.json() as GenericProfile];
  if (dataset.schema_version !== 1 || dataset.profile_id !== "standard_player_19_capsule_generic_v1" || dataset.resolution !== "downsampled_10hz" || !Array.isArray(dataset.records)) throw new Error("current approximate dataset contract rejected");
  if (profile.artifact_kind !== "sentinel_approximate_generic_player_capsule_profile" || profile.profile_id !== dataset.profile_id || profile.confidence !== "generic_fallback" || profile.evidence_allowed !== false || profile.usage_scope !== "exploratory_functional" || profile.exact_model_calibration !== false || profile.capsules.length !== 19) throw new Error("generic profile contract rejected");
  if (!dataset.records.every(validRecord)) throw new Error("current record provenance contract rejected");

  const profileById = new Map(profile.capsules.map((capsule) => [capsule.id, capsule]));
  const approximateSpatial = dataset.records.map<ApproximateSpatialRecord>((record) => ({
    record_type: "player_spatial_approximate",
    tick: record.tick,
    round: 0,
    player_id: record.entity_index,
    status: "available",
    usage_scope: "exploratory_functional",
    evidence_allowed: false,
    source: "generic_fallback",
    confidence: "approximate",
    hitboxes: {
      observed_duck_amount: record.duck_amount,
      capsules: record.capsules.map((capsule) => {
        const sourceCapsule = profileById.get(capsule.id);
        if (!sourceCapsule) throw new Error("capsule profile mismatch");
        return { name: capsule.id, group_id: groupId(sourceCapsule.group), radius: capsule.radius, ...genericEndpoints(capsule, sourceCapsule) };
      }),
    },
  }));
  const frames = new Map<number, CurrentTrackReplay["frames"][number]>();
  for (const record of dataset.records) {
    const frame = frames.get(record.tick) ?? { tick: record.tick, round: 0, players: [], visible_pairs: [] };
    frame.players.push({ steam_id: record.entity_index, name: `PWN-${record.entity_index}`, team: "Functional", x: record.origin.x, y: record.origin.y, z: record.origin.z, yaw: record.yaw_deg, duck_amount: record.duck_amount });
    frames.set(record.tick, frame);
  }
  return { version: `current-track/${dataset.schema_version}`, map: dataset.map, tick_rate: 64, replay_mode: "current_track_5037_generic_fallback", functional_only: true, frames: Array.from(frames.values()).sort((left, right) => left.tick - right.tick), approximate_spatial: approximateSpatial };
}
