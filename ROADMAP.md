# Sentinel AI Roadmap

## Vision

Open-source Behavioral Analysis Platform for Counter-Strike 2.

## Current Status

**M2 Complete** - Real CS2 Integration
- ✅ Source 2 demo parsing
- ✅ Entity tracking (names, teams, positions)
- ✅ Visibility engine
- ✅ Feature extraction (17+ features)
- ✅ Anomaly scoring with evidence
- ✅ Validation harness

## Roadmap

### M3 - Visibility Engine (Complete)

- ✅ Line-of-sight with smoke detection
- ✅ Audio propagation
- ✅ Player visibility state
- ✅ Hidden tracking detection
- ✅ Information availability index

### M4 - Dataset Collection (Infrastructure Complete; Corpus Pending)

**Goal:** Build labeled dataset for validation

| Target | Status |
|--------|--------|
| 200 legit demos | ⏳ Requires corpus |
| 50 cheater demos | ⏳ Requires corpus |
| 50 unknown demos | ⏳ Requires corpus |
| Label verification | ⏳ Requires human-verified labels |

> Repository check: the demo corpus and labels are not included, so the M4 collection targets remain unverified.

- [x] Versioned dataset manifest and local audit command (`sentinel dataset init|audit`)
- [x] Verified feature-sidecar loader and supervised training command (`sentinel dataset train`)
- [x] Regression-case contract and loader for real, human-verified demos

**Sources:**
- HLTV pro matches (legit)
- Faceit L10 matches (legit)
- Community reports (cheater)

### M5 - Validation & Calibration

**Goal:** Measure and improve detection quality

- [ ] Validation harness with 200+ demos
- [x] Precision/Recall curves
- [x] AUC-ROC computation
- [ ] Per-feature importance analysis
- [x] Calibration of score thresholds
- [x] Cross-validation (5-fold)
- [x] Per-map analysis
- [x] Per-map threshold override contract, gated by a declared minimum of verified matches
- [ ] Corpus-backed per-map calibration and regression execution

### M6 - Advanced Features

**Goal:** Improve detection accuracy

- [x] Temporal analysis (patterns over time)
- [x] Cross-round behavior tracking
- [x] Team coordination analysis
- [x] Economy-based decision analysis
- [x] Versioned utility-lineup library and geometry matching contract
- [x] Mode-aware calibration gate that refuses cross-mode threshold reuse
- [x] Reviewed-lineup JSON import and audit; unreviewed coordinates cannot match
- [ ] Populate reviewed lineups and connect real grenade trajectory extraction
- [x] Clutch situation analysis

### M7 - ML Integration

**Goal:** Add machine learning models

- [x] Self-learning baselines (online Welford accumulators)
- [x] Persistent memory (`sentinel_memory.json`) + per-player profiles
- [x] Recidivism-based scoring for marginal-cheater detection
- [x] Native XGBoost-compatible binary classifier with saved model artifacts
- [x] Isolation Forest anomaly detection
- [x] Trainable temporal Transformer encoder with saved model artifacts
- [x] Supervised model training pipeline (`sentinel dataset train`)
- [x] Model versioning (memory schema version)
- [x] A/B comparison contract for baseline, XGBoost and Transformer on one verified corpus
- [ ] Corpus-backed A/B evaluation run

### M8 - Web Interface

**Goal:** User-friendly analysis interface

- [x] Web dashboard
- [x] Interactive timeline
- [x] Evidence viewer
- [x] Player comparison
- [x] Match history
- [x] Report and replay JSON export (`sentinel replay`)
- [x] Interactive Replay Viewer (frames, playback controls, Visibility Engine layer)
- [x] Automatic replay sidecar publication during `sentinel analyze`
- [x] Conditional verified model-identity metadata in replay JSON, with safe compiled-resource path checks and explicit unavailable/partial/complete coverage; this is not exact geometry
- [x] Round Story and roster-resolved kill/death explanations in report, replay and Radar Room
- [x] Local player dossier with recurrence, confidence, supporting reports and reanalysis/provenance state
- [x] Optional protected structured AI summary, restricted to supplied evidence facts and fact references

### M9 - API & Integration

**Goal:** Enable external integrations

- [x] REST API
- [x] Replay frames endpoint (`GET /v1/replays/{id}`)
- [x] Replay endpoint forwards verified model-identity metadata only when the exporter validates it; it does not synthesize mappings or geometry
- [x] Automatic report/replay publication to `SENTINEL_REPORTS_DIR`
- [x] Local dossier endpoint (`GET /v1/players/{steam_id}/dossier`)
- [ ] Discord bot
- [x] OBS overlay data endpoint
- [x] Stream integration data contract
- [x] Tournament integration data contract

### Cross-cutting Reliability (Complete in Code; Corpus-dependent Evaluation Pending)

- [x] Analysis provenance records engine, parser, demo, map geometry, feature schema and model-artifact fingerprints
- [x] Reanalysis lifecycle marks reports stale when a provenance dependency changes
- [x] Confidence policy with Clean/Low/Moderate/High/Extreme levels and InsufficientHistory/Tentative/Standard/Strong verdict statuses
- [x] Account-level local-history scan with recurrence and report-linked supporting matches; it does not use external profile statistics
- [x] Round context in reports and replay sidecars, including score progression, survivors, outcome fields and roster-resolved kill/assist feed
- [x] Kill-context fields for weapon, headshot, wallbang and through-smoke events
- [x] Terminal Encounter/Duel Ledger exported through round context and replay JSON; shot history/TTD stay absent until verified event streams are exported
- [x] Replay-wide normalized `weapon_fire` and `player_hurt` facts; Encounter stores only direct attacker-to-victim damage and an observed damage-to-death interval
- [x] Audited review manifest and explicit `sentinel dataset promote-reviews` workflow; unverified, ambiguous or evidence-less labels cannot promote training data
- [x] Empty versioned utility lineup manifest, preventing fabricated production lineups
- [x] Verified model-identity manifest gate: demo SHA-256, observed handle tuple, canonical compiled-resource path and resource SHA-256 are required; current build value is explicitly external-only until adapter build identity exists
- [x] Gate 1B VMDL resource discovery: deterministic compiled-resource header/block directory, RERL dependencies, SHA-256 and explicit unresolved dependency statuses; no geometry or fallback promotion
- [x] Gate 1C.1 Binary KV3 v5 generic tree decoder: bounded uncompressed/LZ4 buffers, explicit rejection of Zstd/binary blobs, and local-oracle regression for MDAT/CTRL/RED2/DATA; no VMDL geometry semantics
- [x] Gate 1C.2 Read-only semantic-key inspection for decoded VMDL-shaped KV3: class/key/collection observations only, with `exact_geometry_available=false`
- [x] Gate 1D.0 Local fixture qualification: full KV3 review classifies supplied `ctm_diver_varianta.vmdl_c` as `not_geometry_fixture`; no verified handle mapping and no player hitbox-set source
- [ ] Gate 1D.1 requires a telemetry-derived qualified verified player-model fixture bundle with exact block-qualified schema paths; do not search VPK assets by name or parse global KV3 key matches
- [ ] Gate 1D.2 Model Schema Registry: after the first qualified fixture, register a deterministic schema fingerprint together with literal block-qualified skeleton/hitbox paths, parser version and fixture/resource hashes; unknown schema fingerprints are explicitly unsupported
- [ ] Gate 1D.3 Automatic model resolution: resolve only `verified asset mapping → schema fingerprint → registered exact parser`; reject unregistered schemas rather than searching generic KV3 keys or promoting fallback geometry
- [x] Gate 0 local pose-capture trace: emits one non-empty AG2 byte payload with observed pawn identity, model handle, hitbox set and SHA-256; raw bytes remain local-only and never enter replay/evidence paths
- [x] Gate 1 raw telemetry prerequisite: local Gate 0 trace emits controller `m_nPawnCharacterDefIndex` alongside the same pawn/tick AG2 record; pawn `m_iItemDefinitionIndex` is recorded separately as equipment-only and never used for agent identity
- [x] Gate 1A.5 model-handle semantics: the selected build-10847 demo exposes `m_hModel` as `CStrongHandle<InfoForResourceTypeCModel>` but contains no authoritative resource binding; result is `handle_not_resolvable_from_demo`
- [ ] Gate 1A.6 Model Resource Index acquisition: require separate byte-verified build-scoped resource inventory and runtime handle-binding capture; only their path/SHA/build agreement may form a verified model binding, while unresolved/conflict states block all model/geometry work
- [ ] Gate 1A.7 Runtime ResourceSystem capture: design-only until an official Valve-supported export or explicitly authorized non-VAC test environment exists; require same-session linkage to the recorded demo and prohibit hooks, injection and process-memory reads against CS2
- [x] Gate 1 observed asset extraction: same-tick controller character definition `5308` resolves through extracted `items_game.txt` to `ctm_fbi_variantb.vmdl_c`, which is deterministically extracted and hashed; build-match provenance remains unproven
- [x] Variant B 19-Aug content check: reconstructed `game/csgo/pak01_dir.vpk` from depot `2347770` manifest `4814468113142569832` does not match the supplied local directory index; individual chunks remain untested
- [ ] Gate 1 documented non-handle identity chain: requires demo-derived agent/econ/loadout identity plus build-matched schema/VPK provenance before mapping to a VMDL; no handle, CRC or filename inference
- [ ] Verified CS2 build identity in demo metadata plus VMDL/mesh/skeleton/hitbox-set parser before any exact geometry gate can be available
- [ ] AG2 byte-level golden fixture, offline pose decoder and exact bone-transform fixture; do not enable hitbox intersection or approximate LOS beforehand

## Long-term Vision

- **Multi-game support** (CS:GO, Valorant, Apex Legends)
- **Real-time analysis** (stream processing)
- **Cloud deployment** (optional)
- **Mobile app** (view reports)
- **Educational platform** (learn from analysis)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to this roadmap.
