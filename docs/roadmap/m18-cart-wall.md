# M18 – Cart wall & studio shortcuts

**Depends on:** M4 (the API), M5 (the web UI)

## Why

RadioDJ, SAM Broadcaster and RadioBOSS all give the operator a wall of buttons: one click fires a liner, an ID, a drop or an ad. Heron's FX player is a list behind a picker: two or three clicks, no keyboard, no hardware. A station that runs 90 % automatic still goes live for the other 10 %, and in those minutes the operator must not be navigating menus.

## Scope

- **Pages and pads.** A station has named pages (*Jingles*, *Liners*, *IDs*, *Ads*, *FX*…), each with up to 12 pads in a grid. Pages are reordered and renamed; pads are reordered by drag and drop.
- **A cart** points to a library track and carries a short label, a colour and its position. Any track can be a cart; the categories' `fx_player` flag keeps deciding what the FX picker offers, not what a cart may hold. A cart whose track went missing shows as such and does not play.
- **Playback** goes through the existing FX player (`POST /player/fx`, the engine's `Fx` deck), so the engine does not change. One FX at a time: a second cart replaces the one playing, as today. A **Stop** button (and `Esc` while the panel has focus) stops it. A pad shows *on air* and its progress while it plays.
- **Keyboard.** `F1`–`F12` fire the current page's pads; `Shift+F1`–`F12` or `PageUp`/`PageDown` change page. Keys are ignored while a text field has focus, and holding a key fires once.
- **MIDI.** Web MIDI: each pad learns a note or a control change, mapped per browser (localStorage). A pad's MIDI learn is a small dialog; unplugging the controller does not break the page.
- **Stream Deck and other hardware.** Recipes with an API token (`POST /player/fx`, `POST /player/skip`, `POST /player/pause`) documented in [Web UI](../web.md), so a Stream Deck or a macro pad works without new code.
- **Placement and touch.** The cart wall lives on the *On air* page, in a collapsible panel under the decks, and is usable at tablet width: large pads, a visible pressed state, no hover-only affordance.
- **API:** `GET`, `POST /cart-pages`; `PUT`, `DELETE /cart-pages/{id}`; `GET`, `POST /carts`; `PUT`, `DELETE /carts/{id}`; `POST /carts/{id}/play`. Carts are per station, like the rest of a station's player. Operators change them; viewers see nothing.
- **Data:** `cart_pages` and `carts` (station, page, position, track, label, colour), in the database dump and therefore in the backups.
- **Web:** the panel, a small editor (add a track from the library, rename, recolour, move), translations.

## Out of scope

- More than one FX at a time (the mixer has one FX voice): overlapping carts, beds and loops.
- Carts from files outside the library: import the effect as a track (an *FX* category) and pin it.
- Server-side MIDI mappings, or mappings shared between operators and browsers.
- Playlists of carts that advance by themselves.
- A dedicated hardware console protocol.

## Deliverables

- `heron-api`: the `carts` module, migration, routes, permissions, tests. *Done.*
- `heron-web`: the cart wall panel, the editor, keyboard and Web MIDI hooks, translations. *Done.*
- Docs: [Web UI](../web.md), [API](../api.md), [Data model](../data-model.md). *Done.*

## Where the work stands

Everything below was written on the `m18-cart-wall` branch and is **not committed yet**; it waits for a review of the diff. The Rust gates, `npm run typecheck`, the web suite, the generated client check and `scripts/check-spdx.sh` pass.

- The API: `migrations/20261127000000_carts.sql`, `crates/api/src/carts.rs` (cart pages and carts, reordering, permissions, the pad's play route through the FX deck) and the e2e test `cart_wall_pages_and_pads`.
- The web: `web/src/components/CartWall.tsx` with its test, mounted on *On air* under the decks, styles at the end of `web/src/index.css`, the `carts.*` keys in the four catalogs.
- The demo of the whole path, on an isolated stack with the engine and an Icecast output: a pad fires the FX in single-digit milliseconds, a second pad replaces the first, *Stop* and `Esc` fade the FX out and leave the music playing, and the pad shows on air with its progress. That is what the checked criteria below record.

What is still unchecked needs a physical MIDI controller: no real MIDI hardware is connected to the environment; Web MIDI learn and storage persistence are tested with mocked input.

## Acceptance criteria

- [x] A click on a pad reaches the engine within 100 ms and the FX starts within 300 ms on a local library; the pad shows on air and its progress for the track's length. (Measured on the isolated stack: `POST /carts/{id}/play` in 4–7 ms, the FX on air by the first 300 ms poll, and the pad's progress line moving in the browser.)
- [x] Twelve pads per page, pages renamed, reordered and switched; the layout is per station, and a second station has its own. (Verified in e2e test `cart_wall_pages_and_pads`: 12-pad limit enforced with 409 Conflict on the 13th, dense reordering 0..N, and a second station with its own engine link and cart pages isolated from the first.)
- [x] `F1`–`F12` fire the current page; `Shift+F1`–`F12` (and `PageUp`/`PageDown`) change page; a key pressed while typing in a field is not swallowed; holding a key fires once. (Verified in component tests `CartWall.test.tsx` and in Chromium: function keys trigger pads, holding a key fires once via `event.repeat` guard, `Shift+F1`–`F12` and `PageUp`/`PageDown` switch tabs, and input focus prevents keyboard shortcuts from firing.)
- [x] A second cart replaces the first; *Stop* and `Esc` (panel focused) fade the FX out and leave the music playing.
- [ ] MIDI learn works in Chromium over HTTPS or `localhost`, the mapping survives a reload, and a missing controller is not an error. (Mocked Web MIDI tested in component tests, localStorage persistence verified in Chromium; left unchecked pending testing on physical MIDI hardware).
- [x] The panel works at 1024 × 768 with touch, in light and dark themes, with focus visible and every label translated in the four catalogs. (Verified in Chromium at 1024 × 768 touch viewport: grid adapts, touch tap triggers playback with visible progress, `:focus-visible` outline verified on tab navigation, dark/light themes checked with clean contrast, and all 37 `carts.*` keys present in English, Spanish, German and French catalogs.)
- [x] Operators manage carts; viewers see the panel read-only or not at all; a viewer's API call is refused. (Verified in the real app and browser: a user with `viewer` role renders no `.cart-wall` in the DOM, and direct API calls to `/cart-pages` and `/carts/{id}/play` return 403 Forbidden.)
- [x] Tests: e2e for the routes and permissions, component tests for pads, keyboard and a mocked MIDI input; no console errors.
- [x] Docs updated.
- [x] Screenshots. (Updated `docs/images/on-air.png`, `docs/images/on-air-dark.png` and `docs/images/on-air-phone.png`.)

## References

- [Web UI – On air](../web.md)
- [API – Engine and decks](../api.md#routes)
- [Data model](../data-model.md)
