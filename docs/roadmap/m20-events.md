# M20 – Scheduled events

**Depends on:** M2 (the engine), M4 (the API), M6 (rotation clockwheels), M18 (the cart wall)

## Why

RadioDJ v1, RadioBOSS, and SAM Broadcaster all provide scheduled timed events: at exact hours, days, or recurring intervals, the station interrupts or blends into top-of-hour news, legal station IDs, commercials, network relay streams, or changes the active rotation clock. Heron has continuous clockwheels (M6) and weekly schedule grids (M14), but lacks the precision mechanism to fire specific actions at scheduled times.

## Scope

- **Event configuration (`scheduled_events`):**
  - A station has named scheduled events (*Top of the Hour News*, *Legal Station ID*, *Evening Relay*, *Format Switch*).
  - **Triggers:**
    - Recurrence by day of week mask (Monday through Sunday) and time of day (`HH:MM:SS` or minutes from midnight in the station's timezone).
    - Periodic interval (e.g. every 30 or 60 minutes).
    - One-off single date-time trigger.
  - **Timing modes:**
    - `exact`: fires immediately at the scheduled second. If music is playing on Deck A/B, it performs an immediate smooth duck / quick crossfade to prioritize the scheduled event on time.
    - `next`: smooth insertion; queues the action to start immediately after the currently playing track finishes, without abrupt interruption.
  - **Action types:**
    - `play_track`: plays a specific library track (by `track_id`).
    - `play_category`: picks and plays a track from a specified category (using least-recently-played or category rotation).
    - `relay`: connects to an external Icecast/HTTP stream URL for a set duration (seconds) or until stopped, then seamlessly returns to automation.
    - `load_clock`: switches the station's active rotation clockwheel or format immediately.
  - Action parameters are stored in a typed JSON `payload`.
- **Scheduler service in backend (`heron-api`):**
  - A lightweight asynchronous scheduler loop evaluating upcoming events against the station's timezone using `jiff`.
  - Calculates and stores `next_run_at` and `last_run_at`.
  - Dispatches actions via the engine's playback commands without dead air.
  - Provides a manual `POST /stations/{station_id}/events/{id}/run-now` trigger for instant studio testing.
- **API endpoints:**
  - `GET /stations/{station_id}/events`: lists all scheduled events with their status, trigger, timing mode, and next run time.
  - `POST /stations/{station_id}/events`: creates a new scheduled event.
  - `PUT /stations/{station_id}/events/{id}`: updates an existing scheduled event.
  - `DELETE /stations/{station_id}/events/{id}`: deletes a scheduled event.
  - `POST /stations/{station_id}/events/{id}/run-now`: manually triggers the event immediately.
- **Web UI:**
  - Dedicated **Events** page accessible from navigation.
  - List of events with active toggles, countdown to next execution, execution history, and test trigger button.
  - Intuitive event modal supporting trigger builder, timing mode, and action configuration.
  - Full translation across English, Spanish, German, and French catalogs.

## Out of scope

- Complex cron syntax parsing beyond days-of-week and time-of-day / interval recurrence.
- Hard cutoffs that abort mid-word without fade (all exact events duck or crossfade smoothly).
- Multi-station synchronized events (each station owns and configures its own scheduled events).

## Deliverables

- `heron-api`: `events` module, database migration `20261128000000_events.sql`, background scheduler, endpoints, and integration tests.
- `heron-core`: Event models, timing modes, and action types.
- `heron-web`: `Events.tsx` page, dialogs, state hooks, and translation keys.
- Docs: [Web UI](../web.md), [API](../api.md), [Data model](../data-model.md), [Roadmap](README.md).

## Acceptance criteria

- [x] A scheduled event configured for an exact time triggers within 500 ms of the scheduled second.
- [x] In `exact` mode, automation music ducks/fades smoothly without abrupt digital clipping or dead air.
- [x] In `next` mode, the event track is inserted at the front of the queue and plays immediately once the current track completes.
- [x] Recurring events automatically calculate and update `next_run_at` upon execution.
- [x] `run-now` executes the event immediately from the API or UI.
- [x] Relay stream action connects to the remote audio URL, plays for the specified duration, and returns to automation cleanly.
- [x] Web UI displays scheduled events, allows adding/editing/deleting, toggling enabled state, and displays countdowns accurately.
- [x] Permissions: Operators and admins can manage and fire events; viewers have read-only access.
- [x] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`, `npm run generate-api`).
