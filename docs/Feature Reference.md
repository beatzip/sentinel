# Sentinel AI Feature Reference

## Overview

Sentinel AI extracts 17+ features across 7 categories to detect anomalous player behavior.

## Feature Categories

### 1. Aim Features

Features measuring aiming behavior.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `reaction_time` | Time to shoot after enemy appears | 0.20-0.30s | < 0.10s |
| `crosshair_placement_error` | Distance from head level | 10-20 units | < 5 units |
| `aim_velocity` | Angular speed of crosshair | 80-160 deg/s | > 250 deg/s |
| `tracking_smoothness` | Variance in aim velocity | 0.75-0.95 | > 0.98 |
| `target_switch_speed` | Time between target switches | 0.2-0.4s | < 0.1s |

### 2. Wall/Visibility Features

Features based on visibility information.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `hidden_tracking_duration` | Time tracking unseen enemy | 0.0-0.5s | > 1.0s |
| `prefire_rate` | Shots fired before visibility | 0.0-0.2 | > 0.5 |
| `rotation_justification` | Response to teammate deaths | 0.3-0.7 | < 0.2 |

### 3. Movement Features

Features measuring movement patterns.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `movement_smoothness` | Variance in movement direction | 0.7-0.9 | > 0.98 |
| `counter_strafe_accuracy` | Precision of counter-strafing | 0.5-0.8 | > 0.95 |
| `path_efficiency` | Directness of path taken | 0.6-0.85 | > 0.95 |

### 4. Decision Features

Features based on game decisions.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `trade_kill_timing` | Time to trade teammate death | 2-4s | < 0.5s |
| `rotation_speed` | Speed of rotation to help | 3-7s | < 1s |
| `solo_playstyle_index` | Isolation from teammates | 0.2-0.5 | Context only |

### 5. Utility Features

Features based on grenade usage.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `flash_assist_rate` | Flashes that assist teammates | 0.1-0.3 | > 0.6 |
| `nade_usage_rate` | Frequency of grenade use | 0.2-0.4 | > 0.7 |

### 6. Information Features

Features based on visibility engine.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `information_availability` | Knowledge index | 0.2-0.4 | > 0.7 |

### 7. General Features

Basic statistical features.

| Feature | Description | Normal Range | Anomaly Threshold |
|---------|-------------|--------------|-------------------|
| `kd_ratio` | Kill/death ratio | 0.8-1.5 | > 3.0 |
| `headshot_percentage` | Headshot ratio | 0.3-0.5 | > 0.8 |
| `survival_time` | Average time alive per round | 40-80s | > 100s |

## Feature Computation

### Interface

```rust
pub trait Feature: Send + Sync {
    fn name(&self) -> &str;
    fn category(&self) -> FeatureCategory;
    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult;
}
```

### Result

```rust
pub struct FeatureResult {
    pub value: f64,           // Computed value
    pub confidence: f64,      // Confidence in the value
    pub metadata: BTreeMap<String, String>,  // Additional info
}
```

## Baseline Configuration

Baselines define normal ranges for each feature:

```rust
FeatureBaseline::new("reaction_time", 0.25, 0.08)
//                                mean   stddev
```

### Z-Score Calculation

```
z_score = (value - mean) / stddev
```

### Anomaly Score

```
anomaly_score = sigmoid(z_score)
             = 1 / (1 + exp(-z_score + 2))
```

## Adding New Features

1. Create a struct implementing `Feature`:

```rust
pub struct MyFeature;

impl Feature for MyFeature {
    fn name(&self) -> &str { "my_feature" }
    fn category(&self) -> FeatureCategory { FeatureCategory::Aim }
    
    fn compute(&self, ctx: &MatchContext, tick: Tick, player: PlayerId) -> FeatureResult {
        // Compute feature value
        let value = 0.5;
        FeatureResult::new(value)
    }
}
```

2. Register in `FeatureEngine::new()`:

```rust
engine.register(crate::aim::MyFeature);
```

3. Add baseline:

```rust
set.add(FeatureBaseline::new("my_feature", 0.5, 0.1));
```

## Feature Categories

| Category | Count | Key Features |
|----------|-------|--------------|
| Aim | 5 | reaction_time, aim_velocity, tracking_smoothness |
| Wall | 3 | hidden_tracking, prefire_rate, rotation_justification |
| Movement | 3 | smoothness, counter_strafe, path_efficiency |
| Decision | 3 | trade_timing, rotation_speed, solo_playstyle |
| Utility | 2 | flash_assist, nade_usage |
| Information | 1 | information_availability |
| General | 3 | kd_ratio, headshot_pct, survival_time |
| **Total** | **17** | |
