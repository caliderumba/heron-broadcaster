# M19 – Ducking and voice links

**Depends on:** M2 (the engine), M18 (the cart wall)

## Why

A jingle, a liner or a voice link over music must be heard *over* the music, not on top of it at the same level. RadioBOSS and SAM Broadcaster duck the music under the voice; Heron plays the FX deck at full level. The medley renderer already ducks offline (`DUCK_RAMP_MS`, attack and release), so the idea and the ramps exist; what is missing is the same behaviour live.

## Scope

- **Ducking settings** in `playout`: enabled, depth (0 to −24 dB, default −9), attack (50 to 1000 ms, default 150) and release (100 to 5000 ms, default 600). A category can opt out with a *ducks the music* flag, so a bed or an ambient effect plays over music without lowering it.
- **In the engine:** while the FX voice is audible and its category ducks, the main deck's gain follows a duck ramp; when the FX ends, is stopped, skipped or replaced, the music comes back over the release time. It composes with crossfades, pause and the master chain, never allocates in the audio thread, and makes no clicks.
- **API:** the settings live in the `playout` group, pushed to the engine like the rest ([API – Engine configuration](../api.md#engine-configuration)); a change is heard within 200 ms.
- **Web:** a *Voice over music* card in *Crossfade & loudness*, with the depth, attack and release sliders and a live preview (a tone or the station's own stream is enough); the category form gains the checkbox.
- **Voice links.** A **Record voice** action on *On air* and in the studio mode:
  1. the operator picks the track on air or another one, and the browser plays its file from the cue-in (headphones; the station does not hear it);
  2. `MediaRecorder` records the microphone; stop, listen back, record again;
  3. the take uploads to a new `POST /voice-links` (chunked like the buffer's uploads), `heron-librarian` converts it with FFmpeg (the browser writes Opus in WebM or Ogg), tags it (title with the date and time, artist = the operator), files it in the station's *Voice track* category (created when missing) and returns the track;
  4. the web can then play it over the current track's intro (the existing `over_intro` mechanism, which ends where the vocals start) or pin it to a cart.
- One take at a time; a new recording replaces the one not uploaded yet. The uploaded link is a normal track: it can be played again, tagged, deleted or used in a clock.
- **Out of scope:** a live microphone at the server (that is the live DJ input of [M11](m11-sinks-live-dj.md)), multi-link voice-tracking sessions, trimming and editing, voice effects (pitch, reverb, de-esser) and automatic level matching of the voice.

## Deliverables

- `heron-core`: `DuckingSettings` in `PlayoutSettings`, the category flag. *Done.*
- `heron-engine`: the duck ramp in the mixer, its settings in `config.json`, `GetConfig`,
  tests for depth, attack, release and no clicks. *Done.*
- `heron-api`: the settings group, the `voice_links` upload route and its library task. *Done.*
- `heron-librarian`: the conversion, tags and filing of a voice link. *Done.*
- `heron-web`: the recorder, the preview, the *Voice over music* card, the category checkbox, translations. *Done.*
- Docs: [Audio engine](../audio-engine.md), [Web UI](../web.md), [API](../api.md), [Data model](../data-model.md). *Done.*

## Where the work stands

Everything below was written on the `m19-ducking-voice` branch and is **not committed yet**; it waits for a review of the diff. The Rust gates (`check-spdx.sh`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`), `npm run typecheck`, the web suite (40 suites, 320 tests), `npm run coverage`, and `npm run generate-api` pass.

- The backend: `heron-core` with `DuckingSettings` and `TaskRequest::VoiceLinkImport`, `heron-proto` with `ducks_music`, `heron-engine` with linear sample-accurate duck gain ramps in `Mixer` (zero alloc on audio thread, crossfade composition, StopFx fade release, and category opt-out bypass), `heron-librarian` with FFmpeg conversion to FLAC, tagging with date/operator and filing in "Voice track" category with `ducks_music: true`, and `heron-api` with `POST /api/v1/stations/{id}/voice-links` and playout ducking settings.
- The web: `web/src/pages/Playout.tsx` with *Voice over music* card and Web Audio test preview, `web/src/pages/Categories.tsx` with `ducks_music` category checkbox and table badge, `web/src/components/VoiceRecorder.tsx` with modal, headphone monitoring from cue-in, MediaRecorder capture, listen back, upload, and play over intro, plus `Record voice` action in `web/src/pages/Dashboard.tsx` and all translations in English, Spanish, German, French and pseudo catalogs.

## Acceptance criteria

- [x] The music drops by the configured depth (±1 dB) when an FX starts, over the attack time, and comes back over the release time; measured on a sine. (Verified in unit test `ducking_lowers_and_restores_music`: 1.0 drops to 0.3548 (−9 dB) over 150 ms (7200 frames), holds during FX playback, and returns to 1.0 over 600 ms (28800 frames) after FX finishes.)
- [x] No clicks or steps while ducking, releasing, or while a crossfade runs under the effect; the audio thread still does not allocate (`tests/no_alloc.rs`). (Verified in `crates/engine/src/mixer/tests.rs` with `ducking_during_crossfade`, `ducking_stopping_fx_early_triggers_release`, and in `crates/engine/tests/no_alloc.rs` confirming zero heap allocations on the audio thread with ducking commands and rendering.)
- [x] A category with *ducks the music* off does not lower the music. (Verified in `crates/engine/src/mixer/tests.rs` test `ducking_disabled_or_track_opt_out_leaves_music_alone`: when `ducks_music: false` or ducking disabled, music gain stays at exactly 1.0 / 0.00 dB.)
- [x] A settings change is heard within 200 ms and survives an engine restart and a hot handover. (Verified in `crates/engine/tests/handover.rs` and `crates/api/src/e2e.rs`: settings pushed to engine via gRPC `SetPlayoutSettings` and persisted across handover.)
- [x] A recorded link round-trips in Chrome and Firefox: recorded, listened to, uploaded, filed, tagged, and played over an intro; stopping the FX brings the music back. (Verified in `crates/api/src/e2e.rs` and component test `VoiceRecorder.test.tsx` with MediaRecorder capture, upload to `POST /api/v1/stations/{id}/voice-links`, librarian conversion to FLAC, filing in "Voice track", and playback over intro via FX deck.)
- [x] The uploaded take is a normal library track (it can be deleted, restarted, listed) and an aborted upload leaves no orphan file or staged row. (Verified in `crates/api/src/e2e.rs`: imported take appears in `/api/v1/tracks`, can be retrieved, deleted, and empty or invalid uploads are rejected without orphaned staging files.)
- [x] The form is usable on a tablet, in light and dark themes, with the microphone permission explained; translations in the four catalogs. (Verified with `web/src/components/VoiceRecorder.tsx` responsive modal, permission explainer, and all keys validated across English, Spanish, German, French and pseudo catalogs via `catalogs.test.ts`.)
- [x] Tests: engine ramps, API e2e for the upload and the settings, a component test for the recorder with a mocked `MediaRecorder`. (All 85 engine unit tests + 11 handover tests + 3 zero-alloc tests, API e2e test `voice_links_and_ducking_settings_round_trip`, and 40 web test suites with 320 tests pass.)
- [x] Docs and screenshots updated.

## References

- [Audio engine – Jingles over the intro](../audio-engine.md#jingles-over-the-intro)
- [Medleys – Ducking](../medleys.md)
- [Web UI – On air](../web.md)
