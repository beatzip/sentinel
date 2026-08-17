# Sentinel Dataset Manifest

Demo files are deliberately not committed. Register each acquired `.dem` file in `manifest.json` with a relative `demo_path`, a `legit`, `cheater`, or `unknown` label, its source, and whether that label was verified.

Create a local layout with `sentinel dataset init datasets`, then validate the corpus with `sentinel dataset audit datasets/manifest.json`. The audit reports missing files, duplicate paths, and labels that still need verification.
