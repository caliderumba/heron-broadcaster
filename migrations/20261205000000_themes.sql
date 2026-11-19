-- SPDX-License-Identifier: AGPL-3.0-or-later

-- M25: studio mode and high contrast themes.

ALTER TABLE users
    MODIFY COLUMN theme ENUM('system', 'light', 'dark', 'studio', 'high_contrast') NOT NULL DEFAULT 'system';
