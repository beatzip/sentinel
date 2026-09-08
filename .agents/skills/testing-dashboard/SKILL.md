---
name: sentinel-dashboard-local-testing
description: Run and verify the Sentinel Radar Room dashboard locally with its Rust report API and bundled functional replay, without inventing evidence or requiring external credentials.
---

# Sentinel dashboard local testing

## Services
From the repository's `dashboard/` directory, run `pnpm install --frozen-lockfile`, then `pnpm dev`. If pnpm is unavailable, use `npx --yes pnpm@10.4.1` for both commands (check packageManager for future version changes). The Express entry point embeds Vite and defaults to port 3000, choosing another port if busy; use its printed URL.

Run the real report API from the repository root:
`cargo +1.88.0 run -p sentinel-api -- /absolute/path/to/report-directory`

The API defaults to `127.0.0.1:8787`, matching the client. `SENTINEL_API_BIND` changes server binding; `VITE_SENTINEL_API_URL` changes the browser's API address. No database or OAuth is needed for the public report/replay dashboard. Blueprint may cover Rust only, so check dashboard dependencies separately.

For a limited report-rendering check, the checked-in `examples/` directory contains `demo_report.json`. Clearly label it as an example, not a fresh analysis or labeled ground truth. Its absent rounds and replay file cannot establish round-story or exact LOS coverage. Do not generate replacements to disguise missing real artifacts.

## UI paths and assertions
- Обзор loads the first report; Синхронизировать refreshes the archive list.
- Игроки scrolls to risk profiles; clicking a row requests `/v1/players/{steam_id}/dossier`. Verify the exact ID against raw data: SteamIDs exceed JavaScript's safe integer range, so rounding can cause silent 404s.
- Доказательства, Архив and Раунды scroll to sections on the same page. The sidebar itself may scroll offscreen; use Ctrl+Home before the next navigation action.
- Повтор defaults to the checked-in gzip current-track sidecar, independent of the selected report. It is functional/non-evidentiary, not a replay of the active report.
- Current-track names are `PWN-{entity_index}` even if the search placeholder says Player_. Search is case-insensitive. Select a pawn before using observed-crouch buttons.
- Confirm the NOT EVIDENCE banner, `evidence_allowed=false`, `functional_only=true`, 19 capsules per record and unavailable/disabled LOS. Never equate these with real skeletons or hitboxes.
- Check playback advancement and stability after pause, reset, held timeline drag (capture while held), player filtering, crouch navigation and read-only capsule hover.
- Adversarial source-switch check: if API report's replay request fails, require an explicit unavailable state rather than stale data with a new source label. Returning to current-track should restore its source.
- Capsule geometry can be tiny and overlap pawn markers. Do not infer visible toggle effects from DOM alone; mark inconclusive if the pixel difference cannot be verified.

## Devin Secrets Needed
None for local public reports or bundled functional replay.

Optional external storage images require `BUILT_IN_FORGE_API_URL` and `BUILT_IN_FORGE_API_KEY`; absent values cause `/manus-storage/*` 500s, including logo/background images. Do not mistake successful fallback layout for full asset verification.

Authenticated AI summary additionally depends on configured OAuth/app/session/admin access (`OAUTH_SERVER_URL`, `VITE_APP_ID`, `JWT_SECRET`, `OWNER_OPEN_ID` and relevant deployment login configuration), model service access, and potentially `DATABASE_URL`. Inspect current auth setup before testing it; never bypass it. Mark AI/full model/demo-derived workflows untested when these prerequisites are absent.
