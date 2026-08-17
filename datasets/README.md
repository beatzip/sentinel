# Sentinel Dataset Manifest

Demo files are deliberately not committed. Register each acquired `.dem` file in `manifest.json` with a relative `demo_path`, a `legit`, `cheater`, or `unknown` label, its source, and whether that label was verified. For supervised training, add a `features_path` pointing to the `.vectors.json` output of `sentinel analyze`. Cheater demos must include exact `player_labels` keyed by Steam ID so no other player inherits a cheater label.

Create a local layout with `sentinel dataset init datasets`, then validate the corpus with `sentinel dataset audit datasets/manifest.json`. The audit reports missing files, duplicate paths, and labels that still need verification.

Train production artifacts with `sentinel dataset train datasets/manifest.json models`. The command refuses unknown, unverified, missing-feature, or ambiguous cheater labels; it writes `sentinel-xgboost.sqb`, `sentinel-transformer.json`, and `training-metadata.json` only from verified feature vectors.
