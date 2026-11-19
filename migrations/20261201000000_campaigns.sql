-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Advertising campaigns and commercial spots (docs/roadmap/m24-advertising.md, M24).

CREATE TABLE campaigns (
    id                   BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    station_id           BIGINT UNSIGNED NOT NULL,
    name                 VARCHAR(128) NOT NULL,
    client_name          VARCHAR(128) NOT NULL,
    status               VARCHAR(16) NOT NULL DEFAULT 'active',
    start_date           VARCHAR(10) NOT NULL,
    end_date             VARCHAR(10) NOT NULL,
    daily_target         INT UNSIGNED NOT NULL DEFAULT 0,
    total_target         INT UNSIGNED NOT NULL DEFAULT 0,
    priority             TINYINT UNSIGNED NOT NULL DEFAULT 5,
    days_mask            TINYINT UNSIGNED NOT NULL DEFAULT 127,
    start_hour           TINYINT UNSIGNED NOT NULL DEFAULT 0,
    end_hour             TINYINT UNSIGNED NOT NULL DEFAULT 24,
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    KEY campaigns_station_status (station_id, status),
    CONSTRAINT campaigns_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

CREATE TABLE campaign_spots (
    id                   BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    campaign_id          BIGINT UNSIGNED NOT NULL,
    track_id             BIGINT UNSIGNED NOT NULL,
    weight               TINYINT UNSIGNED NOT NULL DEFAULT 100,
    enabled              BOOLEAN NOT NULL DEFAULT TRUE,
    created_at           TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE KEY campaign_spots_unique (campaign_id, track_id),
    CONSTRAINT campaign_spots_campaign FOREIGN KEY (campaign_id) REFERENCES campaigns (id) ON DELETE CASCADE,
    CONSTRAINT campaign_spots_track FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

CREATE TABLE campaign_plays (
    id                   BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    station_id           BIGINT UNSIGNED NOT NULL,
    campaign_id          BIGINT UNSIGNED NOT NULL,
    spot_id              BIGINT UNSIGNED NULL,
    track_id             BIGINT UNSIGNED NOT NULL,
    played_at_unix_ms    BIGINT UNSIGNED NOT NULL,
    duration_ms          INT UNSIGNED NOT NULL,
    PRIMARY KEY (id),
    KEY campaign_plays_campaign (campaign_id, played_at_unix_ms),
    KEY campaign_plays_station (station_id, played_at_unix_ms),
    CONSTRAINT campaign_plays_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE,
    CONSTRAINT campaign_plays_campaign FOREIGN KEY (campaign_id) REFERENCES campaigns (id) ON DELETE CASCADE,
    CONSTRAINT campaign_plays_track FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;
