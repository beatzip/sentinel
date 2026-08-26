# Current-Track Approximate Sidecar

The Radar Room now includes the existing `approximate_spatial` functional consumer for the current-track C# adapter output. It reads only two dashboard-local public assets:

| Asset | SHA-256 | Purpose |
|---|---|---|
| `client/public/approximate/current-track-5037-downsampled.json.gz` | `f99ead536f216dd706fa644b6347d4e8dee12c961c4b3c34263b19487ddbc8ec` | 60,189 downsampled generic records |
| `client/public/approximate/standard-player-19-capsule-generic.json` | `222627bf08f4ffa419e49a23f6484d95009f063a52048ba7a05e4455d7d1a50a` | Generic 19-capsule profile |

The client rejects the sidecar unless all records are definition `5037`, contain exactly 19 capsules, and carry the emitted nested non-evidentiary provenance: `evidence_allowed=false`, `usage_scope=exploratory_functional`, `derivation=definition_5037_to_ctm_sas_profile`, `m_hModel_binding=not_used`, and every exact-claim flag set to `false`.

The conversion into the existing functional map layer uses observed `origin`, yaw, duck amount, materialized generic capsule fields, and the generic profile only. It does not resolve `m_hModel`, load a VMDL, construct exact geometry, show a skeleton, run AG2, add `SpatialShotEvidence`, enable LOS, or create collision/damage/penetration/verdict output.

The functional map is scoped to the selected player and current tick. If the selected pawn has no record at a later tick, the dashboard intentionally renders no functional capsule record until the analyst chooses a pawn observed in that frame.

> **Approximate transform loaded — not an exact model pose.**

## Local viewer check

The existing dashboard Replay Viewer was opened with the `CURRENT TRACK / 5037 / FUNCTIONAL` source. It loaded `de_ancient` at tick `6`, listed nine observed pawns, and showed the functional-only provenance boundary with visibility explicitly unavailable.

With `PWN-106` selected, the dashboard’s existing observed-crouch control moved to tick `6510`. The viewer displayed `duck_amount / 0.86`, one functional player record, and nineteen materialized approximate capsule SVG segments. Its visible provenance remained `generic_fallback · evidence_allowed=false · functional_only / true`.

## High-Contrast Visual Verification

The primary dashboard now places a vermilion **NOT EVIDENCE** plate directly on the functional map. Its three visible labels are `generic_fallback`, `evidence_allowed=false`, and `functional_only=true`; vermilion is used only for this non-evidentiary quarantine boundary.

At the standing current-track frame (tick `6`, selected `PWN-193`), the map showed `DUCKAMOUNT 0.00` and `UPRIGHT GENERIC PROFILE`. The separate observed-crouch indicator therefore distinguishes the observed state without upgrading the generic profile to exact geometry.

Selecting the second standing pawn, `PWN-66`, at the same observed tick preserved the prominent `NOT EVIDENCE` plate, showed `DUCKAMOUNT 0.00`, and restricted the functional map to one player record with nineteen generic capsule segments.

At the selected movement frame `PWN-316 / tick 82524`, the inspector showed observed yaw `-76.9°`, `DUCKAMOUNT 0.00`, one functional record, and nineteen generic capsules. The map retained the high-contrast provenance plate and listed five observed pawns in the frame.

At the second movement frame `PWN-260 / tick 82518`, the viewer showed observed yaw `-87.1°`, `DUCKAMOUNT 0.00`, five observed pawns in the frame, and one selected functional record with nineteen capsules under the same non-evidentiary provenance plate.

At `PWN-106 / tick 6510`, the enhanced observed-crouch plate showed `DUCKAMOUNT 0.86` with `GENERIC PROFILE COMPRESSED`. The frame had one observed crouched pawn, one selected functional record, nineteen generic capsules, and the same `NOT EVIDENCE` provenance labels.

At the full observed crouch frame `PWN-386 / tick 1914`, the plate showed `DUCKAMOUNT 1.00` with `GENERIC PROFILE COMPRESSED`. The selected functional record retained nineteen generic capsules and all visible provenance labels remained non-evidentiary.

Standing, movement, and crouch verification captures are taken with the map canvas showing both the complete `NOT EVIDENCE` plate and the observed-crouch plate; these visual plates do not alter the stored record or the functional selection filter.

The complete canvas was also captured for `PWN-260 / tick 82518`, with `DUCKAMOUNT 0.00`, `UPRIGHT GENERIC PROFILE`, the three provenance labels, and the selected one-record / nineteen-capsule functional layer visible together.

The complete-canvas capture for `PWN-106 / tick 6510` reaffirmed `DUCKAMOUNT 0.86`, `GENERIC PROFILE COMPRESSED`, and the three non-evidentiary provenance labels with one selected nineteen-capsule record.
