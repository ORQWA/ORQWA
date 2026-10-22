# Riding sound textures

Bundled, headerless **24 kHz mono signed PCM16 little-endian** textures, embedded into the
Rust core. All four recordings are **CC0 1.0** (public domain); their source pages state
that licence. Credits also ship in About and in `docs/CREDITS.md`.

| Texture | Recording and author | Excerpt |
|---|---|---|
| `wind.pcm` | [Wind and Leaves.wav](https://freesound.org/people/ecfike/sounds/183496/) by **ecfike** | 12–48 s |
| `chain.pcm` | [bicycle forward movement](https://freesound.org/people/dawidwmika/sounds/380288/) by **dawidwmika** | 8–24 s, the steadier pedaling before the faster spin-up |
| `freewheel.pcm` | [bike-wheel-free-looped.flac](https://freesound.org/people/yurkobb/sounds/274481/) by **yurkobb** | first 2 s |
| `shift.pcm` | [bike_gear_shift.wav](https://freesound.org/people/ChemiCatz/sounds/267910/) by **ChemiCatz** | 0.08–0.38 s, one mechanical shift |

The source recordings' HQ MP3 previews are mixed to mono, high-passed at 45 Hz to remove DC
and rumble, low-passed twice at 8 kHz, resampled and adjusted to a target RMS of 0.18 with a
peak ceiling of 0.85. The exact pipeline is `examples/prepare_ambient.rs`.

Rebuild in the devcontainer with `sh scripts/audio/prepare.sh`. The script downloads pinned,
checksum-verified previews into a temporary directory and uses the existing Rust decoder;
no new dependency or FFmpeg executable is needed. Normal builds use the committed PCM
files and make no audio downloads. A changed upstream preview stops checksum validation.

The 48 kHz stereo mixer reads randomly placed, overlapping passages; it never loops an entire recording.
Wind adds continuously generated, filtered stereo turbulence and irregular gusts. Chain read
rate follows cadence, and the separately recorded freewheel follows road speed while
coasting. Playback rates are bounded for the source bandwidth. The recordings have no
calibrated cadence or road speed: their 1× references (90 rpm, 8 m/s) are listening choices,
not measurements. Listen to the result before changing those references or the mix.

Accepted virtual gear changes trigger the 300 ms shift recording once, with a short fade
at both edges and small pitch/level variation. Four bounded voices allow rapid shifts to
overlap without creating a backlog. The shared input path handles keyboard, BLE shifters
and phone remote; pressing at a gear limit, selecting the current gear, or changing the
profile does not create a shift. Paused, waiting, finished, video and route-less rides do
not queue shifts. Bicycle volume controls the one-shots too; restarting playback discards
pending old shifts. Physical cassette shifts are not reported to ORQWA and cannot trigger it.

For a 30-second preview, run in the container from `core/`:

```sh
cargo run --locked -p orqwa-app --example ambient_preview -- /tmp/ambient-preview.wav
```

The preview waits for 3 seconds, accelerates while pedaling, shifts at seconds 6, 9, 12 and
12.1 (a rapid pair), coasts from second 14, then
slows to a stop. It uses the live mixer and default 50% volumes. Listen with headphones to
judge the wind timbre, chain detail, shift clicks, freewheel transition and absence of seams; automated
tests check response and playback lifecycle but cannot establish perceived realism.
