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

- `heron-core`: `DuckingSettings` in `PlayoutSettings`, the category flag.
- `heron-engine`: the duck ramp in the mixer, its settings in `config.json`, `GetConfig`,
  tests for depth, attack, release and no clicks.
- `heron-api`: the settings group, the `voice_links` upload route and its library task.
- `heron-librarian`: the conversion, tags and filing of a voice link.
- `heron-web`: the recorder, the preview, the *Voice over music* card, the category checkbox, translations.
- Docs: [Audio engine](../audio-engine.md), [Web UI](../web.md), [API](../api.md), [Data model](../data-model.md).

## Acceptance criteria

- [ ] The music drops by the configured depth (±1 dB) when an FX starts, over the attack time, and comes back over the release time; measured on a sine.
- [ ] No clicks or steps while ducking, releasing, or while a crossfade runs under the effect; the audio thread still does not allocate (`tests/no_alloc.rs`).
- [ ] A category with *ducks the music* off does not lower the music.
- [ ] A settings change is heard within 200 ms and survives an engine restart and a hot handover.
- [ ] A recorded link round-trips in Chrome and Firefox: recorded, listened to, uploaded, filed, tagged, and played over an intro; stopping the FX brings the music back.
- [ ] The uploaded take is a normal library track (it can be deleted, restarted, listed) and an aborted upload leaves no orphan file or staged row.
- [ ] The form is usable on a tablet, in light and dark themes, with the microphone permission explained; translations in the four catalogs.
- [ ] Tests: engine ramps, API e2e for the upload and the settings, a component test for the recorder with a mocked `MediaRecorder`.
- [ ] Docs and screenshots updated.

## References

- [Audio engine – Jingles over the intro](../audio-engine.md#jingles-over-the-intro)
- [Medleys – Ducking](../medleys.md)
- [Web UI – On air](../web.md)
