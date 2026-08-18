# Sentinel REST API

Run the local API with `cargo run -p sentinel-api -- reports`. It binds to `127.0.0.1:8787` by default and accepts a report directory containing Sentinel JSON reports. Set `SENTINEL_API_BIND` only when an explicit network binding is required.

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | Service status and version. |
| `GET` | `/v1/reports` | Report identifiers and high-level match summaries. |
| `GET` | `/v1/reports/{id}` | Full JSON report for a safe report identifier. |
| `GET` | `/v1/replays/{id}` | Sampled replay frames and Visibility Engine line-of-sight pairs from `{id}.replay.json`. |
| `GET` | `/v1/players/{steam_id}` | Matching player reports across all local reports. |
| `GET` | `/v1/players/{steam_id}/dossier` | Local-only dossier with report-linked matches, recurrence, confidence, provenance and reanalysis state. |

The API never accepts report paths from clients and only resolves JSON files below its configured report directory.

The dossier endpoint aggregates only reports already published by Sentinel. It does not fetch Steam/FACEIT data, K/D history, account reputation, bans, or other external credibility signals.

Analyze a real demo with `SENTINEL_REPORTS_DIR=reports sentinel analyze match.dem`. The analyzer writes `reports/<match>.json`, `reports/<match>.html`, and `reports/<match>.replay.json` together, so the report and replay endpoints become available without a second export command.
