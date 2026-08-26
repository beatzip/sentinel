# Smoke/Inferno Lifecycle Implementation Contract

**Status:** Approved implementation scope. This contract supersedes no provenance or exact-path gate and does not reopen Exact Gate 1D.x.

## Events listened to

| Event | Required observed fields | State action |
|---|---|---|
| `SmokeGrenadeDetonate` | `entityid`, `x`, `y`, `z`; optional `userid` | Create or update an observed smoke effect |
| `SmokeGrenadeExpired` | `entityid`; optional `x`, `y`, `z`, `userid` | Close the matching observed smoke effect |
| `InfernoStart` | `entityid`, `x`, `y`, `z` | Create or update a generic observed inferno effect |
| `InfernoExpire` | `entityid`; optional `x`, `y`, `z` | Close the matching observed inferno effect |

`SmokeGrenadeExpired` and `InfernoExpire` never create a missing state. A missing prior start/detonation is recorded only as absent state, not reconstructed from timing or nearby players.

## Pairing and fields

Pairing is exact within the current round: `entityid` is copied to both `GrenadeState.id` and `GrenadeState.entity_id`; no proximity, timing, owner, model, weapon-fire, or filename heuristic is allowed.

Every created state writes observed `position`, effect `start_tick`/`detonated_tick`, effect `end_tick` when an observed expiry arrives, `active`, and optional observed owner. `velocity` remains zero because no observed projectile velocity is available. `thrown_tick` becomes optional and remains `None`; the first effect tick is not relabelled as a throw tick. A distinct `Inferno` effect type is used because `inferno_startburn` does not establish Molotov versus Incendiary.

## Explicit non-goals and safety guard

The new states are marked `observed_effect_only=true`. Existing visibility predicates and grenade-counting feature extractors must ignore this marker, so this slice does **not** add LOS, smoke-blocking, flash, penetration, damage, collision, score, detector-feature, verdict, or evidence behavior. No `SpatialShotEvidence` write is introduced.

The slice does not listen to `WeaponFire`, `MolotovDetonate`, decoy events, projectile entity updates, or player flash effects. It creates no trajectory, throw-time, owner, velocity, flash duration, or grenade-type inference.

## Required checks

Focused unit tests must cover smoke/inferno create-and-expire pairing, missing-start expiry non-creation, absent owner, and the visibility guard. A read-only current-demo check must show observed smoke/inferno states without enabling visibility effects.
