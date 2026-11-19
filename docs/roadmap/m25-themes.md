<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
# M25 – Themes: studio mode and high contrast

**Depends on:** M5 (web application), M18–M24

## Why

Broadcast automation consoles are operated in diverse physical environments:
- **Radio control rooms and on-air studios** often operate under dimmed or low-lighting conditions during live shows, late-night shifts, and overnight automated operations. Standard dark mode reduces brightness, but dedicated *Studio Mode* (Booth mode) provides deep black background contrast, soft non-glare text, and vivid high-visibility indicators for live broadcast elements (`ON AIR`, active playing decks, cart wall triggers, cue monitors, and VU meter peaks) so presenters and operators can instantly assess status at a glance without eye fatigue.
- **Accessibility and visually impaired operators** require *High Contrast* mode adhering to WCAG 2.1 AAA contrast guidelines (ratio >= 7:1 for normal text and >= 4.5:1 for graphical interface components). Operators with low vision, color perception deficiencies, or working on monitors under direct daylight need ultra-sharp separation between controls, thick visual focus boundaries, and high-contrast color coding.

Commercial broadcast suites (such as RadioBOSS, Dalet, and RCS Master Control) feature specialized studio and high-contrast display profiles. Heron currently supports `system`, `light`, and `dark` themes. M25 expands Heron's theme architecture with:
1. **Studio mode (`studio`)**: optimized for broadcast booths with deep-black surfaces, glowing neon/broadcast accents, and high-visibility status indicators.
2. **High contrast (`high_contrast`)**: maximum contrast adhering to WCAG AAA standards with distinct component outlines, high-contrast text, and saturated feedback states.

## Scope

- **Theme options:**
  - `system`: follows the operating system preference (`prefers-color-scheme`).
  - `light`: clean, daylight-readable theme.
  - `dark`: standard modern dark interface.
  - `studio`: deep black booth palette (`#08090c`), lowered surface glare, vivid broadcast accents (`--on-air: #ff2d20`, `--deck-a: #3b82f6`, `--deck-b: #ec4899`, `--accent: #00d2b4`), enhanced meter saturation, and subtle ambient glows.
  - `high_contrast`: WCAG 2.1 AAA compliant palette (`#000000` background, `#ffffff` foreground, `#00ffff` / `#ffff00` accents, crisp solid borders, high-visibility focus rings).

- **Data model & API:**
  - Database migration extending `users.theme` enum with `'studio'` and `'high_contrast'`.
  - Backend `Theme` enum in `crates/api/src/users.rs` supporting `Studio` and `HighContrast`.
  - OpenAPI specification and generated TypeScript client updated.

- **Web UI & CSS tokens:**
  - Token definitions for `:root[data-theme="studio"]` and `:root[data-theme="high_contrast"]` (also supporting `high-contrast`).
  - Automatic detection support: `@media (prefers-contrast: more)` falling back gracefully to high contrast when user theme is `system`.
  - Updated `Theme` selector in user Account settings (`/account`) with icons (`monitor`, `sun`, `moon`, `headphones`, `contrast`).
  - Waveform canvas and audio meter rendering adapt automatically to studio and high contrast palettes.
  - Full translations for `account.studio` and `account.highContrast` across English, Spanish, German, and French catalogs.

## Out of scope

- Custom user-defined HEX color palette creators (pre-tuned professional broadcast themes provide optimal accessibility and consistency).
- Per-component theme overrides.

## Deliverables

- Database migration: `migrations/20261205000000_themes.sql`.
- Backend: `crates/api/src/users.rs` with updated `Theme` enum and unit tests.
- OpenAPI specification update and regenerated TypeScript client.
- Frontend: `web/src/lib/theme.ts`, `web/src/index.css`, `web/src/pages/Account.tsx`, `web/src/components/ui.tsx`, `web/src/i18n/{en,es,de,fr}.ts`.
- Documentation: `docs/roadmap/m25-themes.md`, `docs/roadmap/README.md`, `docs/web.md`.

## Acceptance criteria

- [x] Backend `Theme` enum supports `system`, `light`, `dark`, `studio`, and `high_contrast`.
- [x] Database migration updates `users.theme` enum without data loss.
- [x] User preferences endpoint accepts and persists new theme options.
- [x] Frontend applies `data-theme="studio"` and `data-theme="high_contrast"` to `<html>`.
- [x] Studio theme renders booth-optimized deep black surfaces and vivid broadcast accents.
- [x] High contrast theme meets WCAG AAA contrast ratios with crisp borders and focus indicators.
- [x] Account settings allows switching between all 5 theme choices with descriptive labels and icons.
- [x] Full translation across English, Spanish, German, and French.
- [x] All quality gates pass (`check-spdx.sh`, `cargo fmt`, `cargo clippy`, `cargo test`, `npm run typecheck`, `npm run test`, `npm run coverage`).
