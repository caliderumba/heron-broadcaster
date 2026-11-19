<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# M22 – Reports and airplay

**Depends on:** M4 (the API), M5 (the web UI), M18–M25

## Why

Radio automation systems such as RadioDJ v1, SAM Broadcaster and RadioBOSS provide reporting tools to satisfy legal, royalty and programming obligations:
1. **Royalty and copyright compliance:** Broadcasters must submit periodic airplay reports to performance rights organizations (PROs) and collection societies (e.g. SoundExchange, ASCAP, BMI, SESAC, GEMA, SACM, SAYCO). These reports require exact playout timestamps, track titles, artists, albums, ISRCs and durations.
2. **Station programming intelligence:** Program directors and station managers need clear visibility into which songs and artists get the most airplay, rotation density across hours of the day, and playout balance between automated rotation, live manual plays, and listener requests.
3. **Auditing and proof of play:** A fast search and CSV/JSON export tool allows operators to verify when any track or spot was broadcast and resolve sponsor or listener inquiries.

## Scope

- **Airplay reporting & compliance exports:**
  - Playout logs queryable by time window (`from`, `to` timestamps).
  - Filtering by play source (`rotation`, `request`, `manual`, `fallback`), minimum played duration, and title/artist search.
  - Export endpoints: `GET /stations/{id}/reports/airplay/export?format=csv` and `?format=json` generating structured compliance reports with columns: `started_at`, `ended_at`, `title`, `artist`, `album`, `isrc`, `duration_ms`, `played_ms`, `source`, `deck`.
  - Content-Disposition headers for direct browser downloads.

- **Airplay & rotation analytics:**
  - **Top tracks:** ranking of most broadcasted tracks in a chosen time window with play counts and cumulative airtime.
  - **Top artists:** ranking of most played artists with play counts, total airtime, and distinct tracks.
  - **Summary metrics:** total airplay events, total airtime, unique songs, unique artists, and source breakdown (rotation vs requests vs manual vs fallback).
  - **Hourly distribution:** 24-hour play volume breakdown to inspect programming dayparts.

- **Operator Web UI (`/reports`):**
  - Added to navigation under station pages with `chart` icon.
  - Quick date range filters (*Today*, *Yesterday*, *Last 7 days*, *Last 30 days*, *Custom range*).
  - Three tabbed perspectives:
    - **Airplay Log:** chronological list of plays with search, source badges, duration, and one-click *Export CSV* / *Export JSON* action buttons.
    - **Top Charts:** leaderboard cards of top tracks and top artists with play counts and visual airtime bars.
    - **Performance Summary:** high-level KPI cards and source distribution percentages.
  - Mobile-responsive, theme-compliant across light, dark, studio, and high contrast modes.

- **API endpoints:**
  - `GET /stations/{id}/reports/airplay`: list paginated airplay records in range.
  - `GET /stations/{id}/reports/airplay/export`: stream/download CSV or JSON report file.
  - `GET /stations/{id}/reports/top-tracks`: get top played tracks.
  - `GET /stations/{id}/reports/top-artists`: get top played artists.
  - `GET /stations/{id}/reports/summary`: get summary aggregates for the date window.

## Out of scope

- Direct automated FTP/SFTP upload to PRO servers (handled via downstream tools or webhooks).
- Financial accounting, billing or invoice generation (commercial sponsor invoicing belongs to billing software).

## Deliverables

- Backend module: `crates/api/src/reports.rs` with airplay log queries, analytics aggregations, CSV generation, and route handlers.
- Route registration in `crates/api/src/app.rs` and `crates/api/src/stations.rs`.
- OpenAPI schema update and regenerated TypeScript client (`npm run generate-api`).
- Frontend Web: `web/src/pages/Reports.tsx`, unit tests in `web/src/pages/Reports.test.tsx`, navigation entry in `web/src/lib/pages.ts`, routing in `web/src/App.tsx`.
- i18n keys across all 4 catalogs (`en.ts`, `es.ts`, `de.ts`, `fr.ts`).
- Documentation: `docs/roadmap/m22-reports.md`, `docs/roadmap/README.md`, `docs/api.md`, `docs/web.md`.

## Acceptance criteria

- [x] Playout history can be filtered by date range and source, returning accurate play records.
- [x] Exporting airplay records as CSV generates valid RFC 4180 CSV with standard royalty headers (ISRC, title, artist, timestamps).
- [x] Top tracks and top artists endpoints return correct play counts and cumulative broadcast durations.
- [x] Summary endpoint reports accurate totals, unique artist/track counts, and source percentages.
- [x] Web page `/reports` allows selecting preset or custom date ranges and switching between Log, Top Charts, and Summary views.
- [x] CSV and JSON export buttons trigger downloads directly in the browser.
- [x] All 4 i18n catalogs maintain full parity with no missing keys.
- [x] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`).
