# Sentinel Integrations

Sentinel exposes report data locally through `sentinel-api`. The API binds to `127.0.0.1` by default, so it is not publicly reachable without an explicit deployment decision.

## OBS overlay

Use `GET /v1/overlay/{report_id}` as the data source for a browser overlay. The response contains the map, overall anomaly score and players ordered by score. A browser overlay can poll this endpoint at its own cadence; Sentinel does not run a background poller.

## Discord

The report schema is suitable for a Discord webhook or bot summary, but no webhook URL or token is stored in this repository. Configure a webhook secret in the chosen deployment environment, then post only after human review of the report. This preserves Sentinel's evidence-first workflow and prevents automatic accusations.

## Stream and tournament systems

Consume `GET /v1/reports`, `GET /v1/reports/{id}` and `GET /v1/players/{steam_id}` for match cards, spectator tooling or post-match review. The API reads immutable JSON reports, so an integration cannot modify the analysis result or datasets.
