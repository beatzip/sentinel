# Demo 3 Functional Viewer Validation

The development preview loaded the read-only `demo3_functional_downsampled` sidecar from project storage. Its first frame showed Anubis, tick 1, ten player records, ten available approximate player records, nineteen capsules per record, and `functional_only=true`.

The viewer presents `generic_fallback`, `approximate`, and `evidence_allowed=false` in the functional panel. The separate LOS panel reported zero visible pairs for that first sampled frame. No `SpatialShotEvidence` section was present in the sidecar payload.

With the Functional control enabled, the map showed the separate approximate layer and legend. Selecting one real player reduced the functional panel to one player record with nineteen capsules and updated the read-only tick inspector. It did not change the LOS counter or expose any evidence action.

The player index now excludes unresolved ID `0` and retains a non-`Unknown` observed player name when later sampled frames contain an unresolved name for the same Steam ID.

Live check after the correction: the index displayed ten resolved player IDs. Selecting `Player_76561198824626265` reduced the functional geometry readout from ten records to one record while retaining the fixed contract of nineteen capsules, `generic_fallback`, `approximate`, and `evidence_allowed=false`.

For the selected player, the next observed-crouch control advanced from tick 1 to tick 5377 (frame 22 of 267), where the inspector reported `duck_amount=1.00`, one observed crouched player, round 01, and the same functional-only provenance contract. The LOS readout remained zero confirmed pairs.

Capsule hover was also validated on that crouched frame. Player dots are intentionally pointer-transparent so they no longer intercept the SVG capsule hit target. Hovering `leg_lower_l` showed its group, radius, observed endpoints, and the mandatory provenance string `generic_fallback / approximate / evidence_allowed=false`; the inspector supplied no evidence action or verdict path.
