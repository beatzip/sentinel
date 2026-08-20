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
