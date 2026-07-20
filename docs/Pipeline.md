# Sentinel AI Analysis Pipeline

## Overview

The Sentinel AI pipeline transforms a CS2 demo file into an actionable behavioral analysis report.

## Pipeline Steps

```
[1/7] Parse Demo
         │
         ▼
[2/7] Extract Events
         │
         ▼
[3/7] Transform Events
         │
         ▼
[4/7] Reconstruct World State
         │
         ▼
[5/7] Compute Features
         │
         ▼
[6/7] Run Analysis
         │
         ▼
[7/7] Generate Report
```

## Step-by-Step

### Step 1: Parse Demo File

**Input:** `.dem` file (zstd compressed)

**Process:**
1. Decompress zstd if needed
2. Parse Source 2 protobuf format
3. Read header (map, server, tick rate)
4. Extract frames (packets, string tables, data tables)

**Output:** `ParsedDemo` with header and raw frames

```rust
let adapter = Source2Adapter::from_file(path)?;
```

### Step 2: Extract Events

**Input:** Raw frames from demo

**Process:**
1. Decode game events from packets
2. Extract entity state updates
3. Parse player spawn/death/hurt events
4. Parse weapon fire, grenade detonations
5. Parse round start/end, bomb plant/defuse

**Output:** `Vec<GameEvent>` with 25+ event types

```rust
let events: Vec<_> = adapter.events().collect();
```

### Step 3: Transform Events

**Input:** Raw game events

**Process:**
1. Map raw events to typed `GameEvent` enum
2. Extract player IDs, weapons, teams
3. Normalize event data format

**Output:** Typed `GameEvent` list ready for world reconstruction

```rust
let game_events: Vec<GameEvent> = convert_events(&events);
```

### Step 4: Reconstruct World State

**Input:** Typed game events

**Process:**
1. Initialize empty `WorldState`
2. Process events chronologically
3. Update player positions, health, weapons
4. Track grenade states
5. Update round information

**Output:** `Vec<TickState>` - world snapshot at each tick

```rust
let mut rebuilder = WorldRebuilder::new();
let tick_states = rebuilder.process_events(&game_events);
let ctx = MatchContext::new(tick_states);
```

### Step 5: Compute Features

**Input:** `MatchContext` with all tick states

**Process:**
1. For each player, for each tick:
   - Compute aim features (reaction time, crosshair error, etc.)
   - Compute movement features (smoothness, counter-strafe, etc.)
   - Compute wall features (hidden tracking, prefire, etc.)
   - Compute decision features (trade timing, rotation, etc.)
   - Compute utility features (flash assist, nade usage, etc.)

**Output:** `Vec<FeatureVector>` - feature vectors for all players

```rust
let feature_engine = FeatureEngine::new();
let players = adapter.player_ids();

for &player in &players {
    let vectors = feature_engine.compute_match(&ctx, player);
    all_feature_vectors.extend(vectors);
}
```

### Step 6: Run Analysis

**Input:** Feature vectors + baselines

**Process:**
1. For each player, aggregate feature vectors
2. Compute z-scores against baselines
3. Generate evidence for anomalous features
4. Apply Bayesian aggregation
5. Compute category scores (aim, wall, movement, decision)
6. Compute overall anomaly score

**Output:** `PlayerScoreResult` with scores and evidence

```rust
let scorer = Scorer::default_cs2();
for &player in &players {
    let fvs = feature_vectors.iter().filter(|fv| fv.player == player);
    let result = scorer.score_player(player, &fvs.collect());
}
```

### Step 7: Generate Report

**Input:** Analysis results

**Process:**
1. Create `MatchReport` with metadata
2. Add player reports with scores
3. Include evidence entries
4. Generate output formats (JSON, HTML, Markdown)

**Output:** Report files

```rust
let report = MatchReport::new(metadata);
let json = JsonReport::generate(&report);
let html = HtmlReport::generate(&report);
```

## Data Transformations

```
.dem file
    ↓ parse
Header + Frames
    ↓ decode
RawGameEvent[]
    ↓ convert
GameEvent[]
    ↓ reconstruct
TickState[]
    ↓ feature extraction
FeatureVector[]
    ↓ scoring
BehaviorScore + Evidence[]
    ↓ report
JSON / HTML / Markdown
```

## Performance Characteristics

| Step | Time (150K ticks) | Memory |
|------|-------------------|--------|
| Parse | ~500ms | ~100MB |
| Extract Events | ~100ms | ~10MB |
| Transform | ~50ms | ~5MB |
| World State | ~200ms | ~200MB |
| Features | ~3s | ~50MB |
| Analysis | ~500ms | ~20MB |
| Report | ~100ms | ~10MB |
| **Total** | **~4.5s** | **~380MB** |
