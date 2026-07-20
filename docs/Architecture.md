# Sentinel AI Architecture

## Overview

Sentinel AI is an open-source behavioral analysis platform for Counter-Strike 2. It analyzes recorded matches and identifies statistically unusual player behavior without interacting with the game process.

## Design Principles

### Modular
Every subsystem is an independent Rust crate with clear boundaries.

### Deterministic
Same demo file → Same output, 100% reproducible.

### Explainable
Every score has evidence with tick references and reasons.

### Testable
Every algorithm has unit tests, golden datasets, and regression tests.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        sentinel-cli                         │
│                    (Command Line Interface)                 │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     sentinel-source2                        │
│              (CS2 Demo Adapter - DemoSource trait)          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     sentinel-world                          │
│              (World State Reconstruction)                   │
└─────────────────────────────────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│ sentinel-       │ │ sentinel-       │ │ sentinel-       │
│ features        │ │ visibility      │ │ map             │
│ (Feature Engine)│ │ (LOS, Audio)    │ │ (Geometry)      │
└─────────────────┘ └─────────────────┘ └─────────────────┘
          │                   │                   │
          └───────────────────┼───────────────────┘
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     sentinel-analysis                       │
│              (Bayesian Scoring + Evidence)                   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                     sentinel-report                         │
│              (JSON, Markdown, HTML Reports)                 │
└─────────────────────────────────────────────────────────────┘
```

## Crate Responsibilities

### sentinel-core
Core types and data model:
- `Tick` - Time unit in demo
- `PlayerState` - Player snapshot at tick
- `FeatureVector` - Computed features
- `Evidence` - Anomaly evidence
- `BehaviorScore` - Analysis scores
- `DemoSource` trait - Adapter pattern

### sentinel-source2
CS2 demo adapter implementing `DemoSource`:
- Parses Source 2 .dem files via source2-demo
- Extracts game events (25+ types)
- Tracks entity state (players, weapons, grenades)
- Provides player names, teams, positions

### sentinel-world
World state reconstruction:
- Maintains `TickState` for each tick
- Tracks player positions, health, weapons
- Handles grenade states
- Reconstructs from game events

### sentinel-features
Feature extraction engine:
- 17+ features across 7 categories
- Plugin architecture via `Feature` trait
- Computes from `MatchContext` and `TickState`
- Produces `FeatureVector` per player per tick

### sentinel-visibility
Visibility calculations:
- Line-of-sight with wall/smoke checks
- Audio propagation with distance
- Radar information
- Flash duration tracking

### sentinel-analysis
Behavior analysis:
- Z-score baseline comparison
- Bayesian aggregation
- Evidence generation
- Category scoring (aim, wall, movement, decision)

### sentinel-evidence
Evidence management:
- Collection and indexing
- Context linking
- Human-readable explanations

### sentinel-report
Report generation:
- JSON (structured)
- Markdown (readable)
- HTML (visual)

### sentinel-validation
Validation harness:
- Process multiple demos
- Compute metrics (Precision, Recall, F1, FPR, TPR)
- Score distribution analysis

## Data Flow

```
Demo File (.dem)
       │
       ▼
┌──────────────┐
│  Source2     │
│  Adapter     │
└──────────────┘
       │
       ▼ GameEvents
┌──────────────┐
│  World       │
│  Rebuilder   │
└──────────────┘
       │
       ▼ TickStates
┌──────────────┐
│  Feature     │
│  Engine      │
└──────────────┘
       │
       ▼ FeatureVectors
┌──────────────┐
│  Scorer      │
└──────────────┘
       │
       ▼ BehaviorScores + Evidence
┌──────────────┐
│  Report      │
│  Generator   │
└──────────────┘
       │
       ▼ JSON / Markdown / HTML
```

## Key Design Decisions

### 1. DemoSource Trait
The adapter pattern allows swapping demo parsers without changing core logic:
```rust
pub trait DemoSource {
    type Event: DemoEvent;
    type PlayerSnapshot: PlayerSnapshot;
    fn events(&self) -> impl Iterator<Item = Self::Event>;
    fn player_ids(&self) -> Vec<PlayerId>;
    // ...
}
```

### 2. TickState vs WorldState
Features operate on `TickState` (single tick snapshot) rather than `WorldState` (full match context). This enables parallel computation and simpler testing.

### 3. Evidence Hierarchy
- Tier 1: Visibility (can_see, can_hear) - highest confidence
- Tier 2: Behavior (aim, movement) - medium confidence
- Tier 3: Context (solo, economy) - reduces anomaly score, doesn't eliminate evidence

### 4. Bayesian Scoring
Anomaly scores are combined using Bayesian aggregation:
```
prior → evidence₁ → evidence₂ → ... → posterior
```

## Performance

Target for 35-round match (150K ticks, 10 players):
- Load: < 2 seconds
- Analysis: < 10 seconds
- Memory: < 1 GB
