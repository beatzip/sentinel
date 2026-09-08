# Reviewed Ground-Truth Process

This is the minimum process required before Sentinel may publish Phase 3
supervised calibration, regression, A/B, feature-importance, precision,
recall, or ROC metrics.

It does not authorize automatic cheat verdicts. It does not promote the current
all-unknown corpus, and it does not change detector logic.

## 1. Immutable intake

Every demo starts as `unknown`. Before analysis, record:

- demo SHA-256 and byte size;
- source URL or delivery reference and permitted-use basis;
- match, map, mode, and acquisition timestamp when known;
- parser, engine, feature schema, report schema, and map-asset provenance;
- hashes of the generated report, replay, and feature-vector sidecars.

Replacing any byte or provenance dependency creates a new corpus unit and
requires a new review.

## 2. Technical admissibility

A demo is reviewable only when:

- parser completion and roster identity are recorded;
- map and round reconstruction are not silently substituted;
- the report states unsupported capabilities and telemetry-quality failures;
- evidence references resolve to immutable report/replay locations;
- player IDs used by labels match the roster IDs used by exported evidence.

An incomplete demo may remain in the unknown corpus for coverage analysis, but
it cannot enter supervised metrics.

## 3. Human review record

The reviewer examines evidence without seeing a model arm's expected label or
using Sentinel's score as ground truth. Each review record must include:

- stable `review_id`;
- reviewer identity and timestamp;
- exact demo path/hash reference;
- `legit`, `cheater`, or `unknown`;
- evidence references and explicit limitations;
- exact Steam-ID `player_labels` for every `cheater` review;
- `verified: true` only after the reviewer confirms the record.

`unknown` is the required result when the evidence cannot support the proposed
label. A match-level accusation must never be inherited by every rostered
player.

For evaluation-quality `cheater` labels, require either two independent
agreeing reviews or one review backed by an authoritative enforcement or
tournament decision. Disagreements remain `unknown` until a separate
adjudicator records the decision and both prior review IDs.

## 4. Promotion gate

Keep reviews separate from `datasets/manifest.json`. Promote only through:

```text
sentinel dataset promote-reviews datasets/manifest.json datasets/reviews.json
sentinel dataset audit datasets/manifest.json
```

Promotion is admissible only when:

- the review is verified, unambiguous, and has evidence references;
- the demo entry already exists and its immutable identity matches the review;
- cheater reviews contain explicit player labels;
- the feature sidecar was generated from that same demo and provenance;
- no reviewer conflict or unresolved adjudication remains.

The current CLI enforces the structural subset of these rules. Hash identity,
reviewer independence, conflicts, and adjudication must be checked in the
review package until they receive dedicated machine-readable validation.

## 5. Leakage-free evaluation plan

Freeze a versioned corpus snapshot before computing metrics.

- Split by match/demo, never by individual feature vector.
- Keep all players and temporal windows from one match in one partition.
- Keep duplicated demos and re-encodes in the same partition.
- Fit thresholds, calibration, feature selection, and model artifacts only on
  training folds.
- Use validation folds for model selection and one locked holdout for the final
  report.
- Stratify by label, map, and mode where the corpus supports it.
- Do not publish a per-map override before its declared
  `minimum_verified_matches` gate passes.
- Run baseline, XGBoost, Transformer, and ensemble arms on the exact same
  frozen corpus and partitions.

## 6. Publication gate

A Phase 3 metric report must include:

- corpus manifest and review-manifest hashes;
- inclusion, exclusion, duplicate, and unresolved-review counts;
- label counts by player, match, map, mode, and source;
- partition manifest and random seed or deterministic split rule;
- model/configuration artifact hashes;
- confidence intervals and denominators;
- precision, recall, false-positive rate, ROC-AUC, and the declared primary A/B
  metric;
- per-feature importance method and its limitations;
- failed/unsupported demo counts without silently dropping them.

No metric may be described as production performance when the locked holdout,
review requirements, minimum sample gates, or provenance checks are absent.

## Current state

`datasets/manifest.json` has no entries. Therefore Sentinel currently has
training and validation contracts, but no repository-verifiable supervised
ground truth and no admissible Phase 3 metric result.
