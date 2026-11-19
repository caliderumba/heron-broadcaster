<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# M23 – Listener requests

**Depends on:** M4 (the API), M5 (the web UI), M6 (rotation weights), M18–M25

## Why

RadioDJ v1, SAM Broadcaster and RadioBOSS take song requests from the station's website or audience portal. Heron already has the internal support — `PlaySource::Request` in the history and `weights.request_factor` in the rotation engine — but lacked a public interface for listeners to submit song requests and an operator inbox to moderate and enqueue them. For community, student and commercial radio stations, listener requests drive audience engagement and direct interaction with on-air presenters.

## Scope

- **Public request portal:**
  - Route `/request` (and `/stations/{code}/request` for multi-station setups).
  - Searchable catalog over playable tracks (title, artist, album, duration), filtered to only enabled, playable, non-missing audio tracks.
  - Listener name and optional dedication / message field (sanitized and length-capped).
  - Clean, mobile-friendly design supporting all themes (light, dark, studio, high contrast).
  - No listener login required.

- **Security, rate limiting & privacy:**
  - One pending request per listener browser enforced via signed anonymous cookie (`heron_listener_token`).
  - Strict privacy: Heron never stores listener IP addresses.
  - Per-station limits: cooldown period between requests for the same track (default 2 h), maximum pending requests, and daily listener submission caps.
  - Rate limiting on public search and submission endpoints.

- **Moderation inbox (Operator console):**
  - Dedicated `/requests` page in the web app navigation under the station menu.
  - Tabbed or status-badged view: *Pending*, *Approved*, *Rejected*, *Played*, *Expired*.
  - Details card showing requested track, listener name, dedication, submission timestamp, and action buttons (*Approve*, *Reject*).
  - Approving a request automatically enqueues the track into the station's playout queue at the requested position (top/next) with `source = request`.
  - Rejecting marks the request without interrupting playlist flow.
  - Requests expire automatically after a configurable time window (e.g. 4 hours).

- **Real-time broadcast integration:**
  - WebSocket broadcast of new pending requests (`request_received`) for instant UI updates on the operator console without manual refreshes.
  - Played requests carry `source = request`, triggering the rotation engine's `weights.request_factor` multiplier to favor popular audience tracks.

- **Station settings:**
  - Requests enabled / disabled toggle.
  - Cooldown minutes per track.
  - Daily submission cap per listener.
  - Maximum pending request limit.
  - Dedication allowed toggle and maximum character length.
  - Custom title and subtitle / banner message for the public portal.

- **API endpoints:**
  - Public:
    - `GET /public/stations/{code}/requests/settings`: public request settings and station branding.
    - `GET /public/stations/{code}/requests/search`: search playable tracks.
    - `POST /public/stations/{code}/requests`: submit a song request.
  - Operator (authenticated, Operator+):
    - `GET /stations/{station_id}/requests`: list requests with status filtering.
    - `POST /stations/{station_id}/requests/{id}/approve`: approve and enqueue request.
    - `POST /stations/{station_id}/requests/{id}/reject`: reject request.
    - `DELETE /stations/{station_id}/requests/{id}`: delete request.
    - `GET /stations/{station_id}/settings/requests`: get station request configuration.
    - `PUT /stations/{station_id}/settings/requests`: update station request configuration.

## Out of scope

- Direct integration with WhatsApp, Telegram bots or phone switchboards (the REST API can be integrated with external webhooks or bots).
- Paid song requests / microtransactions.
- Direct automated playout without operator approval (stations require editorial and FCC/regulatory curation).

## Deliverables

- Database migration: `migrations/20261210000000_listener_requests.sql` (`station_requests_settings`, `listener_requests`).
- Backend API: `crates/api/src/requests.rs`, routes in `crates/api/src/stations.rs`, `crates/api/src/public.rs`, `crates/api/src/app.rs`, and WebSocket live events.
- OpenAPI specification update and regenerated TypeScript client.
- Frontend Web: `web/src/pages/PublicRequest.tsx` (public portal), `web/src/pages/Requests.tsx` (moderation console), route in `App.tsx`, navigation entry in `lib/pages.ts`, i18n catalogs (`en.ts`, `es.ts`, `de.ts`, `fr.ts`).
- Documentation updates: `docs/roadmap/m23-requests.md`, `docs/roadmap/README.md`, `docs/data-model.md`, `docs/api.md`, `docs/web.md`.

## Acceptance criteria

- [x] Listeners can search playable tracks on the public request page and submit a request with name and dedication.
- [x] Submitted requests appear in real time on the operator's Requests moderation page via WebSocket live messages.
- [x] Operators can approve a request, immediately enqueuing the track with `source = request`.
- [x] Playout of approved request reflects `source = request` in history and boosts track rotation weight.
- [x] Rate limits, duplicate cooldown, and daily caps are enforced without storing listener IP addresses.
- [x] Operators can reject or dismiss requests, and expired requests are pruned cleanly.
- [x] Public request portal works seamlessly on mobile and desktop across all themes.
- [x] Station settings allow configuring cooldown, caps, dedication allowance, and enabling/disabling the portal.
- [x] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`).
