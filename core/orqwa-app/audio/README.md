# Riding sound textures

Bundled, headerless **24 kHz mono signed PCM16 little-endian** textures, embedded into the
Rust core. All six recordings are **CC0 1.0** (public domain); their source pages state
that licence. Credits also ship in About and in `docs/CREDITS.md`.

| Texture | Recording and author | Excerpt |
|---|---|---|
| `wind.pcm` | [Wind and Leaves.wav](https://freesound.org/people/ecfike/sounds/183496/) by **ecfike** | 12–48 s |
| `rain.pcm` | [Light Rain](https://freesound.org/people/soundrecorder7/sounds/167034/) by **soundrecorder7** | 2–22 s, continuous rainfall without thunder |
| `chain.pcm` | [bicycle forward movement](https://freesound.org/people/dawidwmika/sounds/380288/) by **dawidwmika** | 8–24 s, the steadier pedaling before the faster spin-up |
| `freewheel.pcm` | [bike-wheel-free-looped.flac](https://freesound.org/people/yurkobb/sounds/274481/) by **yurkobb** | first 2 s |
| `shift.pcm` | [bike_gear_shift.wav](https://freesound.org/people/ChemiCatz/sounds/267910/) by **ChemiCatz** | 0.08–0.38 s, one mechanical shift |
| `shift-chain-{1,2,3}.pcm` | [changingGears.wav](https://freesound.org/people/stib/sounds/494322/) by **stib** | 6.65–7.75 s, 8.05–9.15 s and 10.65–11.75 s; three complete chain transfers and their settling tails |

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

The Rain preset adds a separate recorded rainfall texture at natural pitch, including while
stationary or coasting. Random overlapping passages avoid a repeating timeline. Weather
changes fade rainfall in or out over about a second, independently of speed and cadence;
Wind volume controls both wind and rain. Rain uses the same pause, finish and screen-exit
playback boundaries as the other riding sound.

Accepted virtual gear changes trigger a quiet lever action, followed by one of three
recorded chain transfers: rattle as the chain climbs or drops, engagement and settling back
into the steady chain texture. The source captures real front-ring transfers; it is a generic
drivetrain texture, not a calibrated recording of each virtual rear cog or direction.
The lever stays near natural pitch. Chain playback follows cadence, with small pitch/level
variation and more bite under load; edge fades and a slight reduction in the steady chain
layer blend it in. If the rider shifts while coasting, the lever sounds immediately and the
chain waits for pedaling. Stopping the pedals during a transfer fades it out until pedaling
resumes. Four bounded voices allow rapid shifts to overlap without creating a backlog.
The shared input path handles keyboard, BLE shifters
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
judge the wind timbre, chain detail, chain transfers, freewheel transition and absence of seams; automated
tests check response and playback lifecycle but cannot establish perceived realism.

For a focused bicycle-only preview, add `--shifts` after the output path. It shifts at 2 and
5 seconds at 60 rpm, at 8 and 8.2 seconds at 90 rpm, then shifts while coasting at 12 seconds.
Pedaling resumes at 15 seconds at 120 rpm, engaging that pending transfer; another shift
follows at 19 seconds. Bicycle volume is 75%, wind is muted, and each section uses a different
load. This preview also uses the live mixer.

For a weather-only preview, add `--rain` after the output path. The rider stays stationary
with Bicycle sound muted and Wind sound at 50%: Clear for 5 seconds, Rain for 15 seconds,
then Clear for 10 seconds. Check that the rainfall is distinct from wind, stays continuous
without repeated seams and fades with the preset changes.
