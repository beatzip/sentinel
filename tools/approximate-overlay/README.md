# Approximate Player Overlay Adapter

This isolated DemoFile.Net adapter emits a **non-evidentiary** generic 19-capsule replay overlay. It is not an exact model, skeleton, hitbox, pose, or collision implementation.

## Contract

The adapter accepts observed pawn `EntityIndex`, `PawnCharacterDefIndex`, `Origin`, `EyeAngles.Yaw`, and `MovementServices.DuckAmount`. It processes definition `5037` by default. `--generic-fallback` is an explicit opt-in for other definitions and remains generic.

The accepted profile is `standard_player_19_capsule_generic_v1`, with aliases `ctm_sas_style` and `tm_phoenix_style`. It requires exactly nineteen capsules, `confidence=generic_fallback`, `evidence_allowed=false`, `usage_scope=exploratory_functional`, and `exact_model_calibration=false`.

For visualization only, the adapter uses yaw-only rotation and these generic crouch transforms:

```text
spine_head_scale = 1 + (0.78 - 1) × duck_amount
leg_scale        = 1 + (0.82 - 1) × duck_amount
upper-body z     = 30 + (local_z - 30) × spine_head_scale - 12 × duck_amount
leg z            = local_z × leg_scale
world_point      = observed_origin + yaw_only_rotation(local_point)
```

It writes full-rate and every-six-tick JSON outputs. Every record carries `confidence=generic_fallback` and a provenance object with `evidence_allowed=false`, `usage_scope=exploratory_functional`, `derivation=definition_5037_to_ctm_sas_profile`, `m_hModel_binding=not_used`, and every exact-claim flag set to `false`.

> This adapter must not write to `SpatialShotEvidence` and must not be used for verdicts, LOS, penetration, damage, collision, AG2, skeleton, exact geometry, or exact hitbox claims.

## Run

```bash
dotnet run --project tools/approximate-overlay/ApproximateLayerAdapter.csproj -- \
  /path/to/demo.dem \
  dashboard/client/public/approximate/standard-player-19-capsule-generic.json \
  /safe-output/approximate-spatial-records-per-tick.json \
  /safe-output/approximate-spatial-records-downsampled.json
```

Do not write the large JSON outputs into the repository. Only the compressed downsampled sidecar is served by the dashboard.
