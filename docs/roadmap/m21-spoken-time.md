<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# M21 – Spoken time, weather and station IDs

**Depends on:** M18 (the cart wall), M19 (ducking and voice links), M20 (scheduled events)

## Why

RadioDJ v1 (SayTime / Time announcements plugin), RadioBOSS (Voice tracking, automated time/weather announcements), and SAM Broadcaster all provide automated, spoken time and weather announcements along with dynamic station IDs. Broadcasters rely on these features to give their stations a live, localized feel 24/7 without needing a live presenter in the studio around the clock.

Heron now has the core building blocks:
- The cart wall (M18) for instant studio triggers.
- Live ducking (M19) for lowering music under spoken announcements without digital clipping or abrupt cuts.
- Scheduled events (M20) for triggering actions on exact seconds or periodic intervals.

M21 combines these to deliver seamless spoken time announcements, automated weather reports, and sweepers/station IDs over music intros.

## Scope

- **Spoken time announcements (SayTime / Hora Hablada):**
  - Audio clips arranged into modular voice packs (intros, hours, minutes, am/pm, outros) or synthesized templates.
  - Formats: 12-hour (with AM/PM or morning/afternoon/evening indicators) and 24-hour time formats.
  - Multiple voice pack support per station/language with easy preview and selection.
  - Seamless multi-clip concatenation or playback queueing: assembles intro + hour + minute into a continuous spoken phrase without unnatural pauses or dead air.
  - Ducking integration: automatically ducks Decks A/B during playback according to M19 ducking settings.

- **Weather reports:**
  - Automated current weather fetching: temperature, weather condition summary, wind, and humidity.
  - Provider integration: Open-Meteo API (free, open, no API key required, high precision by latitude/longitude or city name), with fallback caching.
  - Audio generation or speech pack assembly for weather reports.
  - Configurable update frequency (e.g. hourly, every 30 minutes) and metric/imperial unit selection (°C / °F, km/h / mph).

- **Dynamic station IDs & sweepers:**
  - Playout over music intro (`over_intro`): calculates the remaining intro duration on the current playing track (`intro_ms` from M5b) and plays a matching station ID or sweeper so it finishes right as the vocals begin.
  - Category and tag-based sweeper rotation.

- **Scheduled events and Cart wall integration:**
  - M20 Scheduled Events adds new action types:
    - `announce_time`: speaks the current local time in the station's configured timezone.
    - `announce_weather`: speaks the current weather conditions for the station's city.
    - `station_id`: fires a station identifier, optionally ducking or playing over intro.
  - M18 Cart Wall pads can be assigned to trigger "Announce Time" or "Announce Weather" with a single click or keyboard/MIDI shortcut.

- **API endpoints:**
  - `GET`, `PUT /stations/{station_id}/announcements/time`: time announcement configuration (enabled, format, voice pack, intro frequency).
  - `POST /stations/{station_id}/announcements/time/preview`: previews the spoken time phrase for the current or specified time.
  - `GET`, `PUT /stations/{station_id}/announcements/weather`: weather configuration (enabled, city, latitude, longitude, units, update interval).
  - `POST /stations/{station_id}/announcements/weather/refresh`: manually refreshes weather data from provider.
  - `GET /stations/{station_id}/announcements/weather/status`: returns latest weather conditions and fetch timestamp.
  - `POST /stations/{station_id}/announcements/weather/preview`: previews weather audio phrase.

- **Web UI:**
  - *Announcements & Spoken Audio* tab under station management (or dedicated page).
  - Voice pack selector and player test button.
  - Weather location picker, unit toggle, live condition preview, and refresh button.
  - Cart wall and Scheduled events selectors for Spoken Time & Weather actions.
  - Full translation across English, Spanish, German, and French catalogs.

## Out of scope

- Advanced localized radar maps and severe weather siren relays.
- Real-time cloud TTS synthesis requiring third-party paid subscriptions (local/cached voice packs and open standard audio clips are primary; standard API hooks allowed).

## Deliverables

- Database migration for announcement settings (`time_announcements` and `weather_announcements` or `station_announcements`).
- Backend: `announcements` module in `heron-api`, Open-Meteo client, time assembler, dispatcher integration with `events` and `carts`.
- OpenAPI schema and generated TypeScript client updates.
- Frontend: Web UI components for configuration and previews, integration into Events and Carts dialogs, i18n in 4 languages.
- Documentation: `docs/roadmap/m21-spoken-time.md`, `docs/data-model.md`, `docs/api.md`, `docs/web.md`.

## Acceptance criteria

- [x] Spoken time concatenates intro, hour, minute (and AM/PM when applicable) accurately reflecting the station's timezone without gaps.
- [x] During spoken time or weather announcements, music on Decks A/B ducks cleanly according to M19 settings and releases when the announcement concludes.
- [x] Weather service retrieves current temperature and condition from Open-Meteo for the configured coordinates and caches results reliably.
- [x] In M20 Scheduled Events, `announce_time` and `announce_weather` actions fire on schedule and play the correct announcements.
- [x] In M18 Cart Wall, pads assigned to Announce Time or Weather trigger the spoken announcement immediately with visual feedback.
- [x] Web UI displays announcement settings, lets operators preview time and weather phrases, and updates weather status.
- [x] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`, `npm run generate-api`).
