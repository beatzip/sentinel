# Performance Optimization Ledger

This ledger preserves the accepted optimization boundary recorded in the
project task log. The implementation is committed at `878c162`. The numerical
results below are migrated from the prior task log; raw profiler output,
fixture hashes, and generated artifact comparisons were not committed, so the
measurements are not independently reproducible from a clean clone yet.

## Semantic boundary

Only two reuse changes were retained:

1. `trade_kill_timing` reuses the already selected history window rather than
   repeating `state_at` lookup work.
2. `map_segment_blocked_3d` caches segment results for the lifetime of one
   `MapData` / `MatchContext`, keyed by exact endpoint bits.

No detector threshold, feature identifier, evidence contract, report schema,
or approximate/exact geometry gate was intentionally changed.

The associated segment benchmark constructs its synthetic fixture through a
public `MapData` constructor so the private context-scoped cache remains
initialized and all-target compilation continues to cover the benchmark.

## Reported A/B results

### Trade kill timing

- 321 history ticks per calculation before reuse.
- 99.689% reduction in trade-local `state_at` calls.
- End-to-end reduction: Inferno 25.12%, Nuke 9.63%, Mirage 20.49%.
- Reported unchanged outputs: FeatureVectors, JSON, replay, and HTML.
- Reported unchanged schema: 25 feature identifiers and per-feature
  cardinalities.

### Context-scoped segment cache

- Nuke end-to-end: 508.041 s to 418.520 s, a 17.62% reduction.
- Nuke feature extraction: 490.208 s to 399.668 s, an 18.47% reduction.
- Sampled segment time reduction: 27.103%.
- Cache hits: 2,273,942 of 6,247,231 calls, or 36.399%.
- Cache entries: 3,973,289.
- Peak RSS increase: 233.69 MiB.
- Inferno and Mirage had no active 3D cache calls and are not cache-speedup
  claims.
- Reported unchanged outputs: FeatureVectors, JSON, replay, and HTML.

## Reproduction requirement

A future benchmark publication must record:

- demo SHA-256, parser and feature schema versions;
- map asset provenance;
- exact command, build profile, host CPU, memory, and OS;
- wall-clock, CPU time, and peak RSS;
- generated artifact SHA-256 before and after;
- feature identifiers and cardinalities;
- profiler output or counters supporting the attributed call reduction.

Until that bundle exists, these figures are historical project measurements,
not a portable benchmark baseline or a scaling threshold.
