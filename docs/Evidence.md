# Sentinel AI Evidence System

## Overview

Evidence is the core of Sentinel AI's explainability. Every anomaly score is backed by specific, verifiable evidence entries.

## Evidence Structure

```rust
pub struct Evidence {
    pub tick: Tick,           // When it happened
    pub round: u32,          // Which round
    pub player: PlayerId,    // Who
    pub feature: String,     // What was detected
    pub score: f64,          // Anomaly score (0-1)
    pub confidence: f64,     // Confidence level (0-1)
    pub reason: String,      // Human-readable explanation
    pub metadata: BTreeMap<String, String>,  // Additional context
}
```

## Evidence Hierarchy

### Tier 1: Visibility (Highest Confidence)

Evidence based on what the player could see/hear.

| Feature | Description | Weight |
|---------|-------------|--------|
| `hidden_tracking_duration` | Tracking unseen enemies | 1.0 |
| `prefire_rate` | Shooting before visibility | 1.0 |
| `information_availability` | Knowledge index | 1.0 |

### Tier 2: Behavior (Medium Confidence)

Evidence based on player actions.

| Feature | Description | Weight |
|---------|-------------|--------|
| `aim_velocity` | Crosshair movement speed | 0.8 |
| `tracking_smoothness` | Aim smoothness | 0.8 |
| `crosshair_placement_error` | Aim accuracy | 0.8 |
| `movement_smoothness` | Movement patterns | 0.8 |

### Tier 3: Context (Lower Confidence)

Evidence based on contextual factors.

| Feature | Description | Weight |
|---------|-------------|--------|
| `rotation_justification` | Response to teammate deaths | 0.5 |
| `solo_playstyle_index` | Isolation from team | 0.5 |
| `trade_kill_participation` | Team coordination | 0.5 |

## Evidence Generation

### During Analysis

```rust
// For each feature
for (name, result) in &feature_vector.features {
    // Check if anomalous
    if result.anomaly_score >= threshold {
        // Generate evidence
        let evidence = Evidence::new(
            tick,
            round,
            player,
            name.clone(),
            result.anomaly_score,
            generate_reason(name, result),
        );
        evidence_collector.add(evidence);
    }
}
```

### Reason Generation

Each evidence entry includes a human-readable reason:

```
reaction_time: value 0.080, -2.1σ from mean (anomaly: 0.82)
```

Format: `{feature}: value {value}, {z_score}σ from mean (anomaly: {score})`

## Evidence Querying

### By Player
```rust
let evidence = collector.for_player(PlayerId::new(123));
```

### By Feature
```rust
let evidence = collector.for_feature("hidden_tracking_duration");
```

### By Tick
```rust
let evidence = collector.at_tick(Tick(1000));
```

### Top Anomalous
```rust
let top = collector.top_anomalous(10); // Top 10 most anomalous
```

## Evidence in Reports

### JSON Format
```json
{
  "tick": 1234,
  "round": 5,
  "player": 76561198012345678,
  "feature": "hidden_tracking_duration",
  "score": 0.82,
  "confidence": 0.95,
  "reason": "Player tracked unseen enemy for 1.34s",
  "metadata": {
    "visibility": "false",
    "distance": "1250"
  }
}
```

### Markdown Format
```
**Round 5, Tick 1234**
- Feature: hidden_tracking_duration
- Score: 0.82 (anomalous)
- Reason: Player tracked unseen enemy for 1.34s
- Visibility: Not visible
- Distance: 1250 units
```

## Context Modifiers

### Solo Playstyle

When a player consistently plays alone, rotation_justification evidence is adjusted:

```
Raw rotation_justification: 0.82
Solo playstyle index: 0.75
Adjusted: 0.82 × (1 - 0.75 × 0.5) = 0.52
```

This reduces false positives for solo players while preserving evidence.

### Evidence Preservation

Even with context adjustments, evidence is always preserved in the report. The system shows:
1. Raw anomaly score
2. Context applied
3. Adjusted score
4. Decision (suspicious/not suspicious)

This maintains explainability while reducing false positives.
