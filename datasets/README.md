# Sentinel Dataset Manifest

Demo files are deliberately not committed. Register each acquired `.dem` file in `manifest.json` with a relative `demo_path`, a `legit`, `cheater`, or `unknown` label, its source, and whether that label was verified. For supervised training, add a `features_path` pointing to the `.vectors.json` output of `sentinel analyze`. Cheater demos must include exact `player_labels` keyed by Steam ID so no other player inherits a cheater label.

Create a local layout with `sentinel dataset init datasets`, then validate the corpus with `sentinel dataset audit datasets/manifest.json`. The audit reports missing files, duplicate paths, and labels that still need verification.

Train production artifacts with `sentinel dataset train datasets/manifest.json models`. The command refuses unknown, unverified, missing-feature, or ambiguous cheater labels; it writes `sentinel-xgboost.sqb`, `sentinel-transformer.json`, and `training-metadata.json` only from verified feature vectors.

Human review labels live in a separate versioned JSON manifest. Each promoted review must have a stable `review_id`, a reviewer, timestamp, evidence references, `verified: true`, and exact `player_labels` for a cheater demo. Apply them explicitly with `sentinel dataset promote-reviews datasets/manifest.json datasets/reviews.json`; reviews without evidence, unresolved labels, or missing manifest entries are reported and do not enter training.

`utility-lineups.json` intentionally starts empty. Add only map-specific coordinates that have `source`, `reviewed: true`, and a non-empty `review_ref`; inspect incoming coordinates with `sentinel dataset audit-lineups datasets/utility-lineups.json`. Unreviewed coordinates are preserved for audit but cannot match in analysis.
