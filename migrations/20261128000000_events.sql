-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Scheduled events (docs/roadmap/m20-events.md, M20): timed actions that fire
-- on schedule (recurring days + time, fixed intervals, or one-off timestamps).

CREATE TABLE scheduled_events (
    id                   BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    station_id           BIGINT UNSIGNED NOT NULL,
    name                 VARCHAR(64) NOT NULL,
    enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    -- Trigger type: 'recurring', 'interval', or 'once'.
    trigger_type         VARCHAR(16) NOT NULL DEFAULT 'recurring',
    -- Days of the week bitmask for recurring triggers (1=Mon, 2=Tue, 4=Wed, 8=Thu, 16=Fri, 32=Sat, 64=Sun; 127=all days).
    days_mask            TINYINT UNSIGNED NOT NULL DEFAULT 127,
    -- Seconds from midnight in station's local timezone (0 to 86399) for recurring triggers.
    time_of_day_s        INT UNSIGNED NOT NULL DEFAULT 0,
    -- Interval in seconds for interval triggers (e.g. 1800 for 30m, 3600 for 1h).
    interval_s           INT UNSIGNED NULL,
    -- Epoch millisecond timestamp for one-off triggers.
    scheduled_at_unix_ms BIGINT UNSIGNED NULL,
    -- Timing mode: 'exact' (interrupts/ducks immediately) or 'next' (queued smoothly).
    timing_mode          VARCHAR(16) NOT NULL DEFAULT 'exact',
    -- Action type: 'play_track', 'play_category', 'relay', 'load_clock'.
    action_type          VARCHAR(32) NOT NULL DEFAULT 'play_track',
    -- Typed action configuration stored as JSON:
    -- play_track: {"track_id": 123}
    -- play_category: {"category_id": 45}
    -- relay: {"url": "http://example.com/stream", "duration_seconds": 1800}
    -- load_clock: {"clock_id": 10}
    action_data          JSON NOT NULL,
    last_run_at_unix_ms  BIGINT UNSIGNED NULL,
    next_run_at_unix_ms  BIGINT UNSIGNED NULL,
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY scheduled_events_station_next (station_id, enabled, next_run_at_unix_ms),
    CONSTRAINT scheduled_events_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;
