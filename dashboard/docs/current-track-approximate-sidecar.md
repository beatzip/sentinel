# Current-Track Approximate Sidecar

The Radar Room now includes the existing `approximate_spatial` functional consumer for the current-track C# adapter output. It reads only two dashboard-local public assets:

| Asset | SHA-256 | Purpose |
|---|---|---|
| `client/public/approximate/current-track-5037-downsampled.json.gz` | `f99ead536f216dd706fa644b6347d4e8dee12c961c4b3c34263b19487ddbc8ec` | 60,189 downsampled generic records |
| `client/public/approximate/standard-player-19-capsule-generic.json` | `222627bf08f4ffa419e49a23f6484d95009f063a52048ba7a05e4455d7d1a50a` | Generic 19-capsule profile |

The client rejects the sidecar unless all records are definition `5037`, contain exactly 19 capsules, and carry the emitted nested non-evidentiary provenance: `evidence_allowed=false`, `usage_scope=exploratory_functional`, `derivation=definition_5037_to_ctm_sas_profile`, `m_hModel_binding=not_used`, and every exact-claim flag set to `false`.

The conversion into the existing functional map layer uses observed `origin`, yaw, duck amount, materialized generic capsule fields, and the generic profile only. It does not resolve `m_hModel`, load a VMDL, construct exact geometry, show a skeleton, run AG2, add `SpatialShotEvidence`, enable LOS, or create collision/damage/penetration/verdict output.

> **Approximate transform loaded — not an exact model pose.**

## Local viewer check

The existing dashboard Replay Viewer was opened with the `CURRENT TRACK / 5037 / FUNCTIONAL` source. It loaded `de_ancient` at tick `6`, listed nine observed pawns, and showed the functional-only provenance boundary with visibility explicitly unavailable.

With `PWN-106` selected, the dashboard’s existing observed-crouch control moved to tick `6510`. The viewer displayed `duck_amount / 0.86`, one functional player record, and nineteen materialized approximate capsule SVG segments. Its visible provenance remained `generic_fallback · evidence_allowed=false · functional_only / true`.
