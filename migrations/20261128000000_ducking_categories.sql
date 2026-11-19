-- SPDX-License-Identifier: AGPL-3.0-or-later
--
-- Categories can opt out of live ducking: by default playing a track on the FX
-- player ducks the music decks; background beds and ambient sounds can set
-- ducks_music to false to play over the music without lowering it.

ALTER TABLE categories
    ADD COLUMN ducks_music BOOLEAN NOT NULL DEFAULT TRUE AFTER fx_player;
