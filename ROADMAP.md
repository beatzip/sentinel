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

### M4 - Dataset Collection (In Progress)

**Goal:** Build labeled dataset for validation

| Target | Status |
|--------|--------|
| 200 legit demos | 🔄 In Progress |
| 50 cheater demos | ⏳ Pending |
| 50 unknown demos | ⏳ Pending |
| Label verification | ⏳ Pending |

**Sources:**
- HLTV pro matches (legit)
- Faceit L10 matches (legit)
- Community reports (cheater)

### M5 - Validation & Calibration

**Goal:** Measure and improve detection quality

- [ ] Validation harness with 200+ demos
- [ ] Precision/Recall curves
- [ ] AUC-ROC computation
- [ ] Per-feature importance analysis
- [ ] Calibration of score thresholds
- [ ] Cross-validation (5-fold)
- [ ] Per-map analysis

### M6 - Advanced Features

**Goal:** Improve detection accuracy

- [ ] Temporal analysis (patterns over time)
- [ ] Cross-round behavior tracking
- [ ] Team coordination analysis
- [ ] Economy-based decision analysis
- [ ] Utility lineup analysis
- [ ] Clutch situation analysis

### M7 - ML Integration

**Goal:** Add machine learning models

- [x] Self-learning baselines (online Welford accumulators)
- [x] Persistent memory (`sentinel_memory.json`) + per-player profiles
- [x] Recidivism-based scoring for marginal-cheater detection
- [ ] XGBoost for feature combination
- [x] Isolation Forest anomaly detection (scaffold)
- [ ] Temporal patterns (Transformer)
- [x] Model training pipeline (`sentinel learn`)
- [x] Model versioning (memory schema version)
- [ ] A/B testing framework

### M8 - Web Interface

**Goal:** User-friendly analysis interface

- [ ] Web dashboard
- [ ] Interactive timeline
- [ ] Evidence viewer
- [ ] Player comparison
- [ ] Match history
- [ ] Export capabilities

### M9 - API & Integration

**Goal:** Enable external integrations

- [ ] REST API
- [ ] Discord bot
- [ ] OBS overlay
- [ ] Stream integration
- [ ] Tournament support

## Long-term Vision

- **Multi-game support** (CS:GO, Valorant, Apex Legends)
- **Real-time analysis** (stream processing)
- **Cloud deployment** (optional)
- **Mobile app** (view reports)
- **Educational platform** (learn from analysis)

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to this roadmap.
