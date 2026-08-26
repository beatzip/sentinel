# Grenade Pipeline Audit — Current-State, No-Code

**Decision:** `blocked_for_effective_detection` with a low-cost telemetry extension path.

**Scope:** This is a repository and current-demo audit only. It introduces no grenade parser, state mutation, detection rule, spatial inference, corpus, or ML claim. Exact C4–C8 remains external-blocked and outside this work.

## Executive finding

The active Source 2 path does recognize six grenade/effect event families and preserves a small, usable set of event-level fields. However, the normalized events do not create or update `WorldState.grenades`, so every saved `TickState` contains an empty grenade list despite observed grenade activity. The downstream visibility code therefore has the data model it needs but receives no runtime smoke state; player flash state is also never populated.

> The immediate gap is **state construction and lifecycle management**, not a missing `GrenadeState` type.

## 1. Observed current-demo coverage

The audit ran the existing Source2 adapter read-only over the current-track demo (`SHA-256 7c5bad6f12be4cb7be81a996afa8adbda4a8d3182a0e77c26c7f8a47601bd917`). It observed the following normalized grenade/effect events.

| Normalized event | Observed count | Fields observed in every event family | Ownership observed in this demo |
|---|---:|---|---|
| `SmokeDetonate` | 84 | `entityid`, `x`, `y`, `z` | `userid` present |
| `SmokeExpired` | 80 | `entityid`, `x`, `y`, `z` | `userid` present |
| `FlashDetonate` | 90 | `entityid`, `x`, `y`, `z` | `userid` present |
| `HEDetonate` | 79 | `entityid`, `x`, `y`, `z` | `userid` present |
| `InfernoStart` | 68 | `entityid`, `x`, `y`, `z` | no `userid` observed |
| `InfernoExpire` | 68 | `entityid`, `x`, `y`, `z` | no `userid` observed |
| `MolotovDetonate` | 0 | Not observed in this demo | Not established |
| `Decoy*` | 0 | No active Source2/core event path | Not established |

The current Source2 collector maps smoke, flash, HE, molotov, and inferno game-event names, then preserves common fields plus `entityid` and event position components when provided.[1] The parser-agnostic source taxonomy contains the same seven families but has no decoy variant; the shared normalized event schema is broader and includes decoy variants that the active Source2 path cannot currently emit.[2] [3]

The audit also checked all 3,297 observed `WeaponFire` events in the same demo. None carried one of the known grenade weapon strings (`hegrenade`, `flashbang`, `smokegrenade`, `molotov`, `incgrenade`, or `decoy`). Therefore existing `WeaponFire` must **not** be promoted to a grenade-throw event without additional telemetry evidence.

## 2. Event flow and where data is lost

| Stage | Existing behavior | Audit conclusion |
|---|---|---|
| Source2 game-event ingestion | Emits six observed grenade/effect kinds with `userid` when present, `entityid`, and `x/y/z` | Event-level detonation/effect-position telemetry exists |
| CLI conversion | Maps Source2 `SmokeDetonate`, `SmokeExpired`, `FlashDetonate`, `HEDetonate`, `MolotovDetonate`, `InfernoStart`, and `InfernoExpire` into shared `GameEvent` variants | Fields are preserved across this boundary [4] |
| World rebuilder dispatch | Routes only smoke/flash/HE/molotov detonation to a shared grenade handler | Lifecycle events fall through the wildcard branch [5] |
| Grenade handler | Accepts `_event` and mutates no state | Silent stub; observed grenade telemetry is discarded [6] |
| Saved tick state | Clones `WorldState.grenades` into every `TickState` | Storage path exists, but has no live records to persist [5] |
| Visibility consumer | Reads active smoke state and `PlayerState.flash_duration` | Consumer behavior cannot reflect real smokes or flashes until upstream state is populated [7] [8] |

## 3. Existing data models

The core model already supports `Flash`, `Smoke`, `HE`, `Molotov`, `Incendiary`, and `Decoy`; it holds optional owner and entity ID, position, velocity, thrown/detonated/start/end ticks, and active status.[9] `WorldState` and `TickState` already store `Vec<GrenadeState>` and have active/type queries.[10]

The model is richer than observed live telemetry. Current Source2 events provide effect/detonation positions and sometimes owner, but no observed throw event, no projectile trajectory, no velocity, and no generally available ownership for inferno events. Any later implementation must preserve those fields as **unknown**, not infer them from timing or nearby players.

## 4. Silent stubs and uncovered surfaces

| Surface | Current behavior | Consequence |
|---|---|---|
| `handle_grenade_detonate` | Empty body; `_event` unused | Detonation events never create or mutate `GrenadeState` |
| `SmokeGrenadeExpired`, `InfernoStart`, `InfernoExpire` | Not handled by `WorldRebuilder::process_event` | Effects cannot open/close active windows |
| Decoy | Shared schema has variants, core Source2 taxonomy and active name mapping do not | No end-to-end decoy ingestion path |
| Projectile entities | Source2 entity handler materializes controllers, player pawns, and game rules only | No projectile identity, velocity, or trajectory state [11] |
| Flash duration | `PlayerState.flash_duration` is read by visibility but no event handler sets or decays it | Real flash effects cannot influence visibility state [7] |
| Legacy packet transformer | Supports only smoke/flash/HE/molotov detonation and explicitly skips entity updates | It cannot supply expiry, inferno, decoy, or projectile state [12] |

## 5. Lowest-cost extension options — deferred decisions

### Option A — Effect-window state only (recommended first implementation)

Create state only from already observed detonation/start/expiry events. Smoke can use `SmokeDetonate` and `SmokeExpired` paired by observed `entityid`; inferno can use `InfernoStart` and `InfernoExpire` paired by observed `entityid`. Position comes from observed `x/y/z`; owner remains absent where `userid` is absent. Flash and HE remain one-shot event records rather than active world effects. This adds no projectile trajectory, throw time, velocity, model, skeleton, hitbox, LOS upgrade, damage, collision, or verdict logic.

### Option B — Add observed throw telemetry

Do this only after a multi-demo audit demonstrates a stable Source2 event or entity field that explicitly identifies a grenade throw. The present demo does not support `WeaponFire` as that signal. This work requires a new evidence fixture and cannot be inferred from detonation timing.

### Option C — Player flash-effect state

This requires a separately observed player-effect field or event payload that carries flash duration/strength. The current `FlashDetonate` payload does not establish which players were flashed or for how long, so setting `flash_duration` from detonation alone would be an inadmissible inference.

### Option D — Decoy and projectile trajectories

These are distinct scopes. Decoy first requires source taxonomy and active ingestion coverage; trajectories require observed projectile entity properties. Neither is a safe addition to an initial effect-window implementation.

## 6. Required decisions before any code

1. Approve or reject **Option A** as a state-only smoke/inferno lifecycle feature.
2. Confirm whether HE/flash should remain event-only in the first slice; the audit recommends **yes**.
3. Confirm whether to add a small, build-qualified grenade telemetry fixture set before implementation. A single current demo proves observed field shape but does not prove cross-demo stability.
4. Keep corpus/ML work separate: a 20–30 demo labelled corpus is needed for honest detector evaluation, but it is not a prerequisite for the narrow no-inference state-lifecycle implementation.

## References

[1]: ../crates/sentinel-source2/src/lib.rs#L269-L404 "Source2 game-event mapping and field extraction"
[2]: ../crates/sentinel-core/src/source.rs#L83-L115 "Parser-agnostic source event taxonomy"
[3]: ../crates/sentinel-events/src/kinds.rs#L29-L38 "Shared normalized grenade event variants"
[4]: ../crates/sentinel-cli/src/main.rs#L484-L525 "Source2-to-shared event conversion"
[5]: ../crates/sentinel-world/src/rebuilder.rs#L187-L223 "World event dispatch and tick-state persistence"
[6]: ../crates/sentinel-world/src/rebuilder.rs#L400-L404 "Silent grenade detonation handler"
[7]: ../crates/sentinel-visibility/src/los.rs#L104-L113 "Visibility flash-state dependency"
[8]: ../crates/sentinel-visibility/src/los.rs#L468-L507 "Visibility smoke-state dependency"
[9]: ../crates/sentinel-core/src/grenade.rs#L6-L81 "Core grenade state model"
[10]: ../crates/sentinel-world/src/state.rs#L7-L90 "World grenade storage and queries"
[11]: ../crates/sentinel-source2/src/lib.rs#L407-L500 "Source2 entity-handler scope"
[12]: ../crates/sentinel-events/src/transform.rs#L192-L233 "Legacy transformer coverage and skipped entity updates"
