-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Spoken time, weather and station announcements (docs/roadmap/m21-spoken-time.md, M21).

CREATE TABLE station_announcements (
    station_id             BIGINT UNSIGNED NOT NULL,
    time_enabled           BOOLEAN NOT NULL DEFAULT FALSE,
    time_format            VARCHAR(16) NOT NULL DEFAULT '24h',
    time_voice_pack        VARCHAR(64) NOT NULL DEFAULT 'default',
    weather_enabled        BOOLEAN NOT NULL DEFAULT FALSE,
    weather_city           VARCHAR(128) NOT NULL DEFAULT '',
    weather_latitude       DOUBLE NULL,
    weather_longitude      DOUBLE NULL,
    weather_units          VARCHAR(16) NOT NULL DEFAULT 'metric',
    weather_cached_temp    DOUBLE NULL,
    weather_cached_summary VARCHAR(128) NULL,
    weather_fetched_at_ms  BIGINT UNSIGNED NULL,
    created_at             TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at             TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (station_id),
    CONSTRAINT station_announcements_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

INSERT INTO station_announcements (station_id)
SELECT id FROM stations;
