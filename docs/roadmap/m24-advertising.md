<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# M24 – Advertising campaigns

**Depends on:** M6 (rotation and clockwheels), M20 (scheduled events)

## Why

Broadcast radio stations sustain their operations through sponsorships, commercial advertisements, and station promos. Commercial software such as RadioDJ v1, RadioBOSS (Advertising / Commercial breaks scheduler), and SAM Broadcaster provide specialized tools to schedule advertising campaigns, insert commercial breaks into playlists, ensure fair spot rotation among sponsors, and generate audit-proof proof-of-play reports for clients and agencies.

Heron now has:
- Rotation and clockwheels (M6) for automated track selection.
- Scheduled events (M20) for triggering actions on exact seconds or recurring intervals.
- Spoken announcements and cart triggers (M18, M21).

M24 adds first-class advertising campaign management: configuring client campaigns, flight dates, play quotas, day/hour restrictions, assigning audio spot creatives with relative weights, assembling commercial breaks, and logging verified playouts for advertiser compliance reporting.

## Scope

- **Campaign management:**
  - Client / advertiser name and campaign title.
  - Flight dates: start date and end date (inclusive).
  - Status lifecycle: `active`, `paused`, `completed`, `expired`.
  - Quotas / Targets:
    - Daily target plays (e.g. 5 plays per day; 0 = unlimited).
    - Total campaign target plays (e.g. 100 plays total; 0 = unlimited).
  - Priority level (1 to 10; higher priority campaigns receive precedence in breaks).
  - Time restrictions:
    - Day-of-week bitmask (Monday through Sunday).
    - Hourly window (start hour to end hour, e.g. 06:00 to 22:00).

- **Spot assignment:**
  - Campaigns associate one or more audio tracks from the library as commercial spots.
  - Relative rotation weight per spot (e.g. 100, 50, 25).
  - Individual spot enable/disable toggle.

- **Ad Break / Commercial block generation:**
  - Next-break assembler selects eligible active campaigns whose flight dates, day of week, hour, and daily/total targets have not been exceeded.
  - Weighted random or round-robin selection among active spots in chosen campaigns.
  - Separation between identical campaigns within the same block.

- **Scheduled events integration:**
  - Extends M20 Scheduled Events with new action types:
    - `play_campaign`: plays an eligible spot from a specific campaign immediately or next.
    - `play_ad_block`: assembles and plays a multi-spot commercial break from active campaigns.

- **Proof of Play (Certificados de emisión):**
  - Itemized playout logging: records station, campaign, spot, track, timestamp, and duration for every commercial played.
  - Compliance report: returns target plays, delivered plays, fulfillment percentage, total broadcast time, and chronological play log within a specified date window (`from` to `to`).

- **API endpoints:**
  - `GET /stations/{station_id}/campaigns`: list campaigns with optional status filtering.
  - `POST /stations/{station_id}/campaigns`: create campaign.
  - `GET /stations/{station_id}/campaigns/{id}`: get campaign details, spot list, and quick progress summary.
  - `PUT /stations/{station_id}/campaigns/{id}`: update campaign details.
  - `DELETE /stations/{station_id}/campaigns/{id}`: delete campaign and associated spots.
  - `POST /stations/{station_id}/campaigns/{id}/spots`: attach audio track as a spot with weight.
  - `DELETE /stations/{station_id}/campaigns/{id}/spots/{spot_id}`: detach spot from campaign.
  - `GET /stations/{station_id}/campaigns/{id}/proof-of-play`: generate proof-of-play report with date range filters.
  - `POST /stations/{station_id}/campaigns/next-break`: preview or assemble the next ad block.
  - `POST /stations/{station_id}/campaigns/log-play`: record a completed spot playout.

- **Web UI:**
  - *Advertising & Campaigns* page (`/campaigns`) with status filter tabs (All, Active, Paused, Completed, Expired).
  - Campaign summary cards with client name, dates, priority badge, and visual progress bar (delivered vs target plays).
  - Campaign creation and edit dialogs.
  - Spot management view: audio preview, title, artist, duration, weight slider, and add/remove controls.
  - Proof of Play dialog: date range filter, summary metrics (fulfillment %, total airplay time), and itemized play logs.
  - Integration in Scheduled Events dialog for `play_campaign` and `play_ad_block` action types.
  - Full translation across English, Spanish, German, and French catalogs.

## Out of scope

- Real-time programmatic programmatic ad bidding (DSP/SSP protocols like VAST / OpenRTB; local and direct sponsorship campaigns are primary).
- Automated customer invoicing and accounting ledger (can be exported via Proof of Play CSV/API).

## Deliverables

- Database migration: `migrations/20261201000000_campaigns.sql` (`campaigns`, `campaign_spots`, `campaign_plays`).
- Backend: `crates/api/src/campaigns.rs`, integration with `crates/api/src/events.rs`, `crates/api/src/app.rs`, `crates/api/src/main.rs`, and `crates/api/src/stations.rs` (`PER_STATION` updated to 20).
- OpenAPI specification update and regenerated TypeScript client.
- Frontend: `web/src/pages/Campaigns.tsx`, `web/src/pages/Campaigns.test.tsx`, route in `App.tsx`, navigation entry in `lib/pages.ts`, i18n catalogs (`en.ts`, `es.ts`, `de.ts`, `fr.ts`).
- Documentation: `docs/roadmap/m24-advertising.md`, updates to `docs/roadmap/README.md`, `docs/data-model.md`, `docs/api.md`, `docs/web.md`.

## Acceptance criteria

- [ ] Campaigns can be created with client, start/end dates, daily and total targets, priority, and day/hour restrictions.
- [ ] Audio spots can be assigned to campaigns with weights and enabled/disabled status.
- [ ] Next-break assembler selects eligible campaign spots respecting flight dates, day/hour filters, target caps, and weights.
- [ ] Scheduled events support `play_campaign` and `play_ad_block` action types.
- [ ] Playouts are logged with timestamp, duration, station, campaign, and track ID for auditability.
- [ ] Proof of play endpoint and Web UI report total plays, target fulfillment percentage, and itemized play logs.
- [ ] Web UI allows managing campaigns, viewing progress bars, assigning spots, and inspecting proof of play.
- [ ] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`).
