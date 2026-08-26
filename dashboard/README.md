# Sentinel Radar Room Dashboard

This is the consolidated web dashboard for the primary Sentinel repository. It is no longer maintained as the canonical location for current-track approximate viewer work outside this repository.

## Run

```bash
cd dashboard
pnpm install --frozen-lockfile
pnpm dev
```

Open the address printed by the development server, select **Повтор**, then select **Current 5037 · functional**. The consumer provides a player index, tick scrubber, observed crouch navigation, functional 19-capsule overlay, provenance display, and read-only capsule hover details.

## Approximate Boundary

The current-track sidecar is **functional only**. The client accepts it only when its nested record provenance states `evidence_allowed=false`, `usage_scope=exploratory_functional`, `derivation=definition_5037_to_ctm_sas_profile`, `m_hModel_binding=not_used`, all exact-claim flags are false, the definition is `5037`, and every record has exactly 19 generic capsules.

It does not resolve `m_hModel`, inspect model resources, produce skeletons/hitboxes/AG2 transforms, use `SpatialShotEvidence`, expose line of sight, calculate collision/damage/penetration, or support verdicts.
