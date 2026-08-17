# Sentinel REST API

Run the local API with `cargo run -p sentinel-api -- reports`. It binds to `127.0.0.1:8787` by default and accepts a report directory containing Sentinel JSON reports. Set `SENTINEL_API_BIND` only when an explicit network binding is required.

| Method | Path | Purpose |
|---|---|---|
| `GET` | `/health` | Service status and version. |
| `GET` | `/v1/reports` | Report identifiers and high-level match summaries. |
| `GET` | `/v1/reports/{id}` | Full JSON report for a safe report identifier. |
| `GET` | `/v1/players/{steam_id}` | Matching player reports across all local reports. |

The API never accepts report paths from clients and only resolves JSON files below its configured report directory.
