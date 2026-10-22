# ADR 0029 — Continuous riding audio from recorded textures

- Status: accepted
- Date: 2026-10-22

## Context

The first ambient synthesizer was removed because it did not sound convincing (#259).
R112 calls for realistic wind and bicycle sound that follows riding rather than a repeating
sound track. R26 and R48 require per-ride controls before and during the ride, offline.

## Decision

The Rust application layer owns a 48 kHz stereo mixer for each ride. Small CC0 wind, chain
and freewheel recordings are embedded as PCM textures. Randomly positioned, overlapping
passages retain their recorded texture without repeating an entire clip. Continuously
generated stereo airflow adds speed-dependent pressure and brightness, with irregular,
smoothed gusts; weather changes the breeze without changing ride physics. The recorded wind
stays at natural pitch. Chain playback follows cadence and effort; freewheel playback
follows simulated road speed while coasting. Missing cadence is estimated from road speed
only while power is reported; an explicit zero cadence always counts as coasting.

Godot only transfers PCM into an `AudioStreamGenerator` with a short buffer. Pause, finish,
abort and hiding the ride screen stop playback and discard its queue. Ambient sound runs
only on a 3D course ride, including workouts on a course and the overlay; a video ride uses
its original sound and a workout without a course stays quiet. Wind and bike have separate
volume sliders in the shared ride options; zero mutes a layer.

## Consequences

- About and `docs/CREDITS.md` list each source, author and licence. The texture README and
  checksum-verified preparation script document reproducible processing, without a new
  dependency. Builds and rides need no audio download.
- Rust tests cover speed response, cadence, coasting, muting and invalid input; application
  and Godot checks cover playback boundaries. A WAV preview uses the actual mixer so the
  subjective result can be reviewed by listening.
- Mono 24 kHz recorded textures keep the payload small (about 2.6 MB); generated airflow
  provides stereo width. The 48 kHz output preserves high-frequency detail when a texture
  speeds up. The wind is an audible approximation, not a physical wind simulation.
- Recorded cadence and wheel speed were not calibrated. Playback reference rates are
  documented listening choices. Rain currently strengthens the breeze; this does not add a
  separate rain recording or location-specific birds, traffic or water.

## Amendment — 2026-10-22: recorded shifting sound (R113)

A fourth CC0 recording supplies a 300 ms gear-change one-shot. The shared Rust shift-input
handler compares the accepted gear before and after every input, so keyboard, BLE and
remote shifts all trigger once, including opposing changes in one poll. Unchanged/limit
requests and profile edits produce no sound; a physical cassette reports no gear changes.

One-shots share bicycle volume and use brief edge fades plus subtle pitch and level
variation. Four bounded voices let fast changes overlap; another rapid shift replaces an
old voice rather than leaving a long queue. A non-playing ride cannot queue a shift, and
restarting the audio queue discards old voices. The listening preview includes separate
shifts and a rapid pair; signal and application tests cover timing, silence, limits,
pause, mute and cassette behavior. Existing controls and screenshots remain accurate.
