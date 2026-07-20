# Sentinel AI Dataset Guide

## Overview

The dataset is critical for validating and calibrating Sentinel AI's detection capabilities.

## Dataset Structure

```
dataset/
├── metadata.csv          # Master index of all demos
├── legit/                # Known legitimate players
│   ├── hltv/            # Professional matches
│   ├── faceit/          # Faceit Level 10
│   └── premier/         # Premier mode
├── cheater/             # Known cheaters
│   ├── overwatch/       # Overwatch-reviewed
│   └── community/       # Community-reported
├── unknown/             # Unlabeled matches
└── calibration/         # Baseline distributions
```

## Labeling Schema

### Player Labels

| Label | Description | Confidence |
|-------|-------------|------------|
| `legit` | No evidence of cheating | High |
| `cheater` | Confirmed cheater (VAC/OW) | High |
| `unknown` | Unlabeled | None |

### Evidence Types

| Type | Source | Reliability |
|------|--------|-------------|
| VAC Ban | Valve Anti-Cheat | Definitive |
| Overwatch | Player review | High |
| Manual Review | Expert analysis | Medium |
| Community Report | Player reports | Low |

## Data Collection

### Source 1: HLTV Professional Matches

Professional matches are guaranteed clean (LAN, anti-cheat).

```bash
# Example: Download from HLTV
# https://www.hltv.org/results
```

**Label:** `legit`
**Volume:** 100+ matches available daily

### Source 2: Faceit Matches

Faceit has kernel-level anti-cheat.

```bash
# Faceit API requires authentication
# https://open.faceit.com/
```

**Label:** `legit` (95%+)
**Volume:** 50+ matches per day

### Source 3: Community Reports

Players report suspected cheaters.

**Label:** `cheater` (requires verification)
**Volume:** Variable

## Feature Coverage

### Required Features for Validation

| Category | Features | Count |
|----------|----------|-------|
| Aim | reaction_time, crosshair_error, aim_velocity, tracking | 5 |
| Wall | hidden_tracking, prefire_rate, rotation_justification | 3 |
| Movement | smoothness, counter_strafe, path_efficiency | 3 |
| Decision | trade_timing, rotation_speed, solo_playstyle | 3 |
| Utility | flash_assist, nade_usage | 2 |
| General | kd_ratio, headshot_pct, survival_time | 3 |
| **Total** | | **19** |

### Feature Computation

Each feature is computed per-player per-tick:

```rust
// For each tick
for tick in 0..match_ticks {
    // For each player
    for player in players {
        // Compute all features
        let fv = feature_engine.compute_all(&ctx, Tick(tick), player);
        feature_vectors.push(fv);
    }
}
```

## Calibration

### Baseline Distributions

Baselines are computed from legit player data:

```
Feature: reaction_time
  Mean: 0.25s
  StdDev: 0.08s
  P95: 0.38s
  P99: 0.44s
```

### Anomaly Scoring

```
z_score = (value - mean) / stddev
anomaly_score = sigmoid(z_score)
```

### Threshold Calibration

The anomaly threshold determines:
- **Precision** (low threshold → more false positives)
- **Recall** (high threshold → more false negatives)

Optimal threshold is found via ROC analysis on validation data.

## Validation Metrics

### Confusion Matrix

```
                    Predicted
                 Legit  |  Cheater
Actual  Legit  |  TN   |    FP
        Cheater|  FN   |    TP
```

### Key Metrics

| Metric | Formula | Target |
|--------|---------|--------|
| Precision | TP / (TP + FP) | > 0.8 |
| Recall | TP / (TP + FN) | > 0.7 |
| F1 Score | 2 * P * R / (P + R) | > 0.75 |
| FPR | FP / (FP + TN) | < 0.1 |
| AUC-ROC | Area under ROC curve | > 0.9 |

## Data Quality

### Checks

- [ ] No duplicate demos
- [ ] All labels verified
- [ ] No corrupted files
- [ ] Consistent tick rates
- [ ] Complete player data

### Statistics

Track per-demo:
- Player count
- Round count
- Tick count
- Event count
- Feature coverage

## Usage

```bash
# Generate calibration from legit demos
sentinel calibrate dataset/legit/

# Run validation on labeled dataset
sentinel validate dataset/

# View statistics
sentinel stats dataset/vectors.json
```
