# Changelog

All notable changes to Sentinel AI will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased](https://github.com/user/sentinel-ai/compare/v0.1.0...HEAD)

### Added
- M5 Validation: Precision/Recall curves, AUC-ROC, threshold calibration,
  k-fold cross-validation, per-map analysis (`sentinel-validation::calibration`,
  `::curves`); new `sentinel evaluate` and `sentinel cross-validate` CLI commands
- M6 Advanced features: temporal consistency, cross-round consistency, team
  coordination, economy-decision score, utility lineup accuracy, clutch
  performance (`sentinel-features::advanced`)
- M7 ML: real recursive Isolation Forest with deterministic seeded training,
  XGBoost-style BoostedStumps (gradient boosting on decision stumps),
  TemporalTransformer self-attention scaffold, and an A/B testing framework
  comparing models by AUC-ROC (`sentinel-analysis::ml`)
- Validation harness for evaluation metrics
- Solo playstyle detection to reduce false positives
- Team proximity and trade kill features
- Information availability index
- Rotation justification with context awareness

### Changed
- Improved rotation_justification to account for solo playstyle
- Enhanced visibility engine with smoke and flash detection
- Updated baseline configurations for new features

### Fixed
- Entity tracking for real player names and teams
- Feature computation using actual world state
- Evidence generation with proper thresholds

## [0.1.0](https://github.com/user/sentinel-ai/releases/tag/v0.1.0) - 2026-07-20

### Added

#### Core Platform
- Sentinel workspace with 15 crates
- Core types: Tick, PlayerState, FeatureVector, Evidence, BehaviorScore
- DemoSource trait for adapter pattern
- MockSource for testing

#### Demo Parsing
- Source 2 demo adapter (sentinel-source2)
- Entity tracking for player names, teams, positions
- Game event extraction (25+ event types)
- Snappy decompression support

#### Feature Engine
- 17+ features across 7 categories
- Aim features: reaction time, crosshair placement, aim velocity, tracking
- Movement features: smoothness, counter-strafe, path efficiency
- Wall features: hidden tracking, prefire rate, rotation justification
- Decision features: trade timing, rotation speed
- Utility features: flash assist, nade usage
- General features: K/D, headshot %, survival time

#### Visibility Engine
- Line-of-sight checks with smoke detection
- Audio propagation with distance attenuation
- Radar information
- Player visibility state tracking

#### Analysis
- Bayesian aggregation for anomaly scoring
- Z-score based baseline comparison
- Evidence generation with confidence scores
- Category-based scoring (aim, wall, movement, decision)

#### Evidence
- Evidence collection and indexing
- Evidence linking with context
- Human-readable explanations

#### Reports
- JSON report generation
- Markdown report generation
- HTML report with visualizations

#### CLI
- `sentinel analyze` - Full analysis pipeline
- `sentinel validate` - Validation harness
- `sentinel calibrate` - Generate calibration data
- `sentinel stats` - Dataset statistics

#### Validation
- ValidationHarness for evaluation metrics
- Confusion matrix computation
- Precision, Recall, F1, FPR, TPR metrics
- Score distribution analysis

### Changed
- Migrated from WorldState to TickState for feature computation
- Improved entity tracking with safe property access
- Enhanced visibility engine with proper smoke detection

### Fixed
- Entity tracking panics in source2-demo serializer
- Feature computation bounds checking
- Evidence generation thresholds

## [0.0.1](https://github.com/user/sentinel-ai/releases/tag/v0.0.1) - 2026-07-15

### Added
- Initial project structure
- Core type definitions
- Basic feature engine
- Simplified visibility checks