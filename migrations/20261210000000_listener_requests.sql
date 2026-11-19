-- SPDX-License-Identifier: AGPL-3.0-or-later

-- M23: listener requests settings and request logs.

CREATE TABLE IF NOT EXISTS station_requests_settings (
    station_id BIGINT UNSIGNED NOT NULL PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    cooldown_minutes INT UNSIGNED NOT NULL DEFAULT 120,
    max_pending INT UNSIGNED NOT NULL DEFAULT 50,
    daily_cap INT UNSIGNED NOT NULL DEFAULT 5,
    allow_dedication BOOLEAN NOT NULL DEFAULT TRUE,
    max_dedication_len INT UNSIGNED NOT NULL DEFAULT 200,
    title VARCHAR(128) NOT NULL DEFAULT 'Song Requests',
    subtitle VARCHAR(255) NOT NULL DEFAULT 'Search our library and send your favorite track to our on-air host',
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    CONSTRAINT fk_requests_settings_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS listener_requests (
    id BIGINT UNSIGNED NOT NULL AUTO_INCREMENT PRIMARY KEY,
    station_id BIGINT UNSIGNED NOT NULL,
    track_id BIGINT UNSIGNED NOT NULL,
    listener_name VARCHAR(128) NOT NULL,
    dedication VARCHAR(512) NOT NULL DEFAULT '',
    status ENUM('pending', 'approved', 'rejected', 'played', 'expired') NOT NULL DEFAULT 'pending',
    client_token VARCHAR(64) NOT NULL,
    requested_at_unix_ms BIGINT NOT NULL,
    reviewed_at_unix_ms BIGINT NULL,
    reviewed_by BIGINT UNSIGNED NULL,
    CONSTRAINT fk_listener_requests_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE,
    CONSTRAINT fk_listener_requests_track FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE,
    CONSTRAINT fk_listener_requests_user FOREIGN KEY (reviewed_by) REFERENCES users (id) ON DELETE SET NULL
);

CREATE INDEX idx_listener_requests_station_status ON listener_requests (station_id, status);
CREATE INDEX idx_listener_requests_track ON listener_requests (station_id, track_id, requested_at_unix_ms);
CREATE INDEX idx_listener_requests_client ON listener_requests (station_id, client_token, requested_at_unix_ms);
