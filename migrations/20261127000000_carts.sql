-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Cart wall (docs/roadmap/m18-cart-wall.md, M18): pages of pads that fire
-- the FX player. A cart points to a library track; pages and carts are per
-- station. Positions are 0-based and the API keeps them dense.

CREATE TABLE cart_pages (
    id          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    station_id  BIGINT UNSIGNED NOT NULL,
    name        VARCHAR(64) NOT NULL,
    position    INT UNSIGNED NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY cart_pages_station (station_id, position),
    CONSTRAINT cart_pages_station FOREIGN KEY (station_id) REFERENCES stations (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;

CREATE TABLE carts (
    id         BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    page_id    BIGINT UNSIGNED NOT NULL,
    track_id   BIGINT UNSIGNED NOT NULL,
    -- Shown on the pad: the operator's name for it, which may differ from
    -- the track's title (a liner cut from a longer file).
    label      VARCHAR(64) NOT NULL,
    -- `#rrggbb`, or none for the pad's default colour.
    color      VARCHAR(7) NULL,
    position   INT UNSIGNED NOT NULL DEFAULT 0,
    PRIMARY KEY (id),
    KEY carts_page (page_id, position),
    KEY carts_track (track_id),
    CONSTRAINT carts_page FOREIGN KEY (page_id) REFERENCES cart_pages (id) ON DELETE CASCADE,
    CONSTRAINT carts_track FOREIGN KEY (track_id) REFERENCES tracks (id) ON DELETE CASCADE
) ENGINE = InnoDB DEFAULT CHARSET = utf8mb4;
