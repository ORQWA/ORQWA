//! Continuous riding sound (R114): recorded textures with non-repeating grains, turbulent
//! airflow, independent pedal / freewheel rates, and recorded chain transfers (R116–R117).

use orqwa_domain::units::{MetersPerSecond, Rpm, Watts};

/// Samples per second of the ambient stereo stream.
pub const RATE: u32 = 48_000;
const RATE_F32: f32 = 48_000.0;
// The texture bandwidth fits even at 3× playback; the output stays at the device's typical
// rate so fast freewheel transients do not fold back into lower frequencies.
const TEXTURE_STEP: f32 = 24_000.0 / RATE_F32;
const DT: f32 = 1.0 / RATE_F32;
const WIND: &[u8] = include_bytes!("../audio/wind.pcm");
const CHAIN: &[u8] = include_bytes!("../audio/chain.pcm");
const FREEWHEEL: &[u8] = include_bytes!("../audio/freewheel.pcm");
const SHIFT: &[u8] = include_bytes!("../audio/shift.pcm");
const TRANSFERS: [&[u8]; 3] = [
    include_bytes!("../audio/shift-chain-1.pcm"),
    include_bytes!("../audio/shift-chain-2.pcm"),
    include_bytes!("../audio/shift-chain-3.pcm"),
];

/// Weather affects the soundscape, without changing the simulation's wind or resistance.
#[derive(Clone, Copy, Debug, Default)]
pub enum Weather {
    /// Light breeze.
    #[default]
    Clear,
    /// A stronger, gustier breeze.
    Cloudy,
    /// Nearly still air.
    Hazy,
    /// Blustery air.
    Rain,
}

impl Weather {
    fn breeze(self) -> f32 {
        match self {
            Self::Clear => 0.7,
            Self::Cloudy => 1.8,
            Self::Hazy => 0.25,
            Self::Rain => 3.5,
        }
    }
}

/// The two independently adjustable ride sounds, in percent (zero mutes that layer).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    /// Wind around the rider and through vegetation.
    pub wind: f64,
    /// Chain movement and coasting freewheel.
    pub bike: f64,
    /// The ride's visual weather preset.
    pub weather: Weather,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            wind: 50.0,
            bike: 50.0,
            weather: Weather::Clear,
        }
    }
}

/// Measurements for the next buffer; speed is simulated road speed, never trainer speed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Motion {
    /// Road speed.
    pub speed: MetersPerSecond,
    /// Selected cadence source, if it reports one; an explicit zero means no pedaling.
    pub cadence: Option<Rpm>,
    /// Selected power source, used for chain load and a missing-cadence fallback.
    pub power: Option<Watts>,
}

impl Motion {
    #[allow(clippy::cast_possible_truncation)] // bounded physical values for audio DSP
    fn targets(self) -> [f32; 3] {
        let speed = finite(self.speed.0, 40.0) as f32;
        let power = finite(self.power.unwrap_or_default().0, 2_500.0) as f32;
        let cadence = self.cadence.map_or_else(
            || {
                if power > 0.0 {
                    (speed * 7.5).clamp(40.0, 110.0)
                } else {
                    0.0
                }
            },
            |rpm| finite(rpm.0, 220.0) as f32,
        );
        [speed, cadence, power]
    }
}

fn finite(value: f64, max: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.0, max)
    } else {
        0.0
    }
}

/// Stateful mixer, owned by one ride; pull only the frames the output device can accept.
pub struct Sound {
    random: Random,
    wind: Texture,
    chain: Texture,
    freewheel: Texture,
    motion: [f32; 3],
    volume: [f32; 2],
    gust: f32,
    gust_target: f32,
    gust_left: u32,
    airflow: [[f32; 2]; 2],
    pedal: f32,
    coasting: f32,
    crank: f32,
    breeze: f32,
    shifts: [Option<ShiftVoice>; 4],
    next_shift: usize,
}

impl Sound {
    /// A fresh, reproducible soundscape. Use a different seed for each ride.
    #[must_use]
    pub fn new(seed: u64) -> Self {
        let mut random = Random(seed.max(1));
        Self {
            wind: Texture::new(WIND, 3.7, &mut random),
            chain: Texture::new(CHAIN, 0.19, &mut random),
            freewheel: Texture::new(FREEWHEEL, 0.13, &mut random),
            random,
            motion: [0.0; 3],
            volume: [0.0; 2],
            gust: 1.0,
            gust_target: 1.0,
            gust_left: 0,
            airflow: [[0.0; 2]; 2],
            pedal: 0.0,
            coasting: 0.0,
            crank: 0.0,
            breeze: Weather::Clear.breeze(),
            shifts: [None; 4],
            next_shift: 0,
        }
    }

    /// Fade in the next output after playback was stopped and its queue discarded.
    pub fn restart(&mut self) {
        self.volume = [0.0; 2];
        self.shifts.fill(None);
    }

    /// Trigger a quiet lever action followed by the chain transferring and settling (R117).
    /// The chain waits for pedaling; bounded voices keep repeated requests from growing a queue.
    pub fn shift(&mut self) {
        let clip = TRANSFERS[self.random.next() as usize % TRANSFERS.len()];
        self.shifts[self.next_shift] = Some(ShiftVoice {
            lever: 0.0,
            transfer: 0.0,
            delay: 0.05,
            clip,
            pitch: 0.97 + self.random.unit() * 0.06,
            gain: 0.9 + self.random.unit() * 0.2,
            chain_rate: 1.0,
            engagement: 0.0,
        });
        self.next_shift = (self.next_shift + 1) % self.shifts.len();
    }

    /// Render stereo PCM, with smoothed controls to avoid clicks at telemetry updates.
    /// The caller handles pause/finish by stopping playback and discarding queued samples.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // bounded controls narrowed to PCM precision
    pub fn render(&mut self, frames: usize, motion: Motion, settings: Settings) -> Vec<[f32; 2]> {
        let targets = motion.targets();
        let volumes = [
            finite(settings.wind, 100.0) as f32 * 0.01,
            finite(settings.bike, 100.0) as f32 * 0.01,
        ];
        let pedaling = if targets[1] > 3.0 { 1.0 } else { 0.0 };
        let coasting = if targets[0] > 0.15 && targets[1] <= 3.0 {
            1.0
        } else {
            0.0
        };
        let mut out = Vec::with_capacity(frames);
        for _ in 0..frames {
            for (value, target) in self.motion.iter_mut().zip(targets) {
                approach(value, target, DT / 0.25);
            }
            for (value, target) in self.volume.iter_mut().zip(volumes) {
                approach(value, target, DT / 0.04);
            }
            approach(&mut self.pedal, pedaling, DT / 0.08);
            approach(&mut self.coasting, coasting, DT / 0.08);
            if self.gust_left == 0 {
                self.gust_target = 0.55 + self.random.unit();
                self.gust_left = RATE / 2 + self.random.next() % (RATE * 3);
            }
            self.gust_left -= 1;
            approach(&mut self.gust, self.gust_target, DT / 0.65);
            approach(&mut self.breeze, settings.weather.breeze(), DT / 0.8);
            let [speed, cadence, power] = self.motion;
            // Speed raises pressure and high-frequency energy, not the wind recording's pitch.
            let airspeed = speed + self.breeze * self.gust;
            let pressure = (airspeed / 14.0).powi(2).min(2.0);
            let brightness = (700.0 + airspeed * 130.0).min(6_000.0);
            let cutoff = brightness / (brightness + RATE_F32 / std::f32::consts::TAU);
            let rustle = self.wind.next(1.0, &mut self.random);
            let chain = self
                .chain
                .next((cadence / 90.0).clamp(0.35, 2.5), &mut self.random);
            let freewheel = self
                .freewheel
                .next((speed / 8.0).clamp(0.25, 3.0), &mut self.random);
            self.crank = (self.crank + cadence / 60.0 * DT).fract();
            // The two loaded strokes of each crank turn subtly colour the chain texture.
            let stroke = ((self.crank * 2.0).fract() - 0.5).abs() * 2.0;
            let chain_gain =
                self.pedal * (0.16 + 0.12 * (power / 350.0).min(1.0)) * (0.85 + 0.15 * stroke);
            let mut shift = 0.0;
            let mut transferring = 0.0;
            for voice in &mut self.shifts {
                if let Some(active) = voice {
                    if let Some((sample, engagement)) = active.next(targets[1]) {
                        shift += sample;
                        transferring += engagement;
                    } else {
                        *voice = None;
                    }
                }
            }
            // The recorded rattle replaces some steady chain noise, then blends back into it
            // as the chain seats. A hard shift has more bite under the rider's load.
            let bike = chain * chain_gain * (1.0 - 0.55 * transferring.min(1.0))
                + freewheel * self.coasting * 0.25
                + shift * (0.3 + 0.15 * (power / 350.0).min(1.0));
            let mut stereo = [0.0; 2];
            for (channel, sample) in stereo.iter_mut().enumerate() {
                let noise = self.random.unit() * 2.0 - 1.0;
                let filters = &mut self.airflow[channel];
                approach(&mut filters[0], noise, cutoff);
                approach(&mut filters[1], noise, DT * 160.0);
                let flow = filters[0] * 0.14 + filters[1] * 0.8;
                let wind = flow * pressure * self.gust * 0.35
                    + rustle * (0.06 + self.breeze * 0.025 + pressure * 0.16);
                // Conservative headroom even in a fast descent; keep the chain close and centred.
                *sample = (wind * self.volume[0] + bike * self.volume[1]).clamp(-0.95, 0.95);
            }
            out.push(stereo);
        }
        out
    }
}

fn approach(value: &mut f32, target: f32, coefficient: f32) {
    *value += (target - *value) * coefficient;
}

struct Random(u64);

impl Random {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        // Upper half avoids the short low-bit pattern of some PRNGs.
        u32::try_from(self.0 >> 32).unwrap_or_default()
    }

    #[allow(clippy::cast_precision_loss)] // 24 random bits are exactly representable in f32
    fn unit(&mut self) -> f32 {
        (self.next() >> 8) as f32 / 16_777_216.0
    }
}

/// Two overlapping, randomly positioned passages. Complementary smooth windows hide seams;
/// varying the starting point preserves a recording's timbre without repeating a timeline.
struct Texture {
    pcm: &'static [u8],
    starts: [f32; 2],
    ages: [f32; 2],
    length: f32,
}

impl Texture {
    fn new(pcm: &'static [u8], seconds: f32, random: &mut Random) -> Self {
        let mut texture = Self {
            pcm,
            starts: [0.0; 2],
            ages: [0.0, 0.5],
            length: seconds * RATE_F32,
        };
        texture.starts = [texture.start(random), texture.start(random)];
        texture
    }

    #[allow(clippy::cast_precision_loss)] // assets have fewer than 2^24 samples
    fn start(&self, random: &mut Random) -> f32 {
        random.unit()
            * (self.pcm.len() as f32 / 2.0 - self.length * 3.0 * TEXTURE_STEP - 2.0).max(0.0)
    }

    fn next(&mut self, rate: f32, random: &mut Random) -> f32 {
        let mut sample = 0.0;
        for voice in 0..2 {
            let phase = self.ages[voice];
            let triangle = 1.0 - (phase * 2.0 - 1.0).abs();
            let window = triangle * triangle * (3.0 - 2.0 * triangle);
            sample += pcm_at(self.pcm, self.starts[voice]) * window;
            // Integrate the read cursor: changing rate must not jump within a live grain.
            self.starts[voice] += rate * TEXTURE_STEP;
            self.ages[voice] += 1.0 / self.length;
            if self.ages[voice] >= 1.0 {
                self.ages[voice] -= 1.0;
                self.starts[voice] = self.start(random);
            }
        }
        sample
    }
}

#[derive(Clone, Copy)]
struct ShiftVoice {
    lever: f32,
    transfer: f32,
    delay: f32,
    clip: &'static [u8],
    pitch: f32,
    gain: f32,
    chain_rate: f32,
    engagement: f32,
}

impl ShiftVoice {
    #[allow(clippy::cast_precision_loss)] // short recorded excerpt, exact sample count in f32
    fn next(&mut self, cadence: f32) -> Option<(f32, f32)> {
        let lever_length = (SHIFT.len() / 2) as f32;
        let transfer_length = (self.clip.len() / 2) as f32;
        if self.lever >= lever_length && self.transfer >= transfer_length {
            return None;
        }
        let fade = (self.lever / 48.0)
            .min((lever_length - self.lever) / 120.0)
            .min(1.0);
        let mut sample = pcm_at(SHIFT, self.lever) * fade.max(0.0) * 0.12;
        self.lever = (self.lever + self.pitch * TEXTURE_STEP).min(lever_length);
        let pedaling = cadence > 3.0;
        approach(
            &mut self.engagement,
            if pedaling { 1.0 } else { 0.0 },
            DT / 0.008,
        );
        if pedaling {
            approach(
                &mut self.chain_rate,
                (cadence / 90.0).clamp(0.4, 2.2) * self.pitch,
                DT / 0.02,
            );
        }
        let mut engagement = 0.0;
        if self.engagement > 0.000_1 && self.transfer < transfer_length {
            self.delay -= self.chain_rate * DT;
            if self.delay <= 0.0 {
                let remaining = transfer_length - self.transfer;
                let fade = (self.transfer / 120.0).min(remaining / 1_800.0).min(1.0);
                engagement = fade * self.engagement;
                sample += pcm_at(self.clip, self.transfer) * engagement;
                self.transfer += self.chain_rate * TEXTURE_STEP;
            }
        }
        Some((sample * self.gain, engagement))
    }
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // bounded PCM index
fn pcm_at(pcm: &[u8], position: f32) -> f32 {
    let index = position as usize;
    let read = |index: usize| {
        pcm.get(index * 2..index * 2 + 2).map_or(0.0, |bytes| {
            f32::from(i16::from_le_bytes([bytes[0], bytes[1]])) / 32_768.0
        })
    };
    let fraction = position.fract();
    read(index) * (1.0 - fraction) + read(index + 1) * fraction
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moving(speed: f64, cadence: f64) -> Motion {
        Motion {
            speed: MetersPerSecond(speed),
            cadence: Some(Rpm(cadence)),
            power: Some(Watts(250.0)),
        }
    }

    #[test]
    fn shifts_follow_bicycle_volume_and_discard_pending_transfers_on_restart() {
        let settings = Settings {
            wind: 0.0,
            bike: 100.0,
            ..Settings::default()
        };
        let mut sound = Sound::new(259);
        let _ = sound.render(RATE as usize, Motion::default(), settings);
        sound.shift();
        let heard = sound.render(RATE as usize / 2, Motion::default(), settings);
        assert!(
            energy(&heard) > 0.000_000_01,
            "the lever is audible even while the chain is stationary"
        );
        let tail = sound.render(RATE as usize, Motion::default(), settings);
        assert!(
            energy(&tail) < 0.000_000_01,
            "the lever ends instead of looping while the chain waits"
        );
        sound.shift();
        let muted = Settings {
            bike: 0.0,
            ..settings
        };
        let mut silent = Sound::new(259);
        silent.shift();
        assert!(energy(&silent.render(RATE as usize * 2, moving(8.0, 90.0), muted)) < 0.000_000_01);
        sound.restart();
        assert!(
            sound.shifts.iter().all(Option::is_none),
            "restart must also discard chain transfers still waiting for pedaling"
        );
        assert!(
            energy(&sound.render(RATE as usize, Motion::default(), settings)) < 0.000_000_01,
            "a restarted output must discard a pending old shift"
        );
        for _ in 0..24 {
            sound.shift();
        }
        assert_eq!(sound.shifts.iter().flatten().count(), 4);
        let rapid = sound.render(RATE as usize * 3, moving(8.0, 90.0), settings);
        assert!(
            rapid
                .iter()
                .flatten()
                .all(|sample| sample.is_finite() && sample.abs() < 1.0)
        );
        assert!(
            sound.shifts.iter().all(Option::is_none),
            "rapid presses must not leave a long backlog"
        );
    }

    fn shift_voice() -> ShiftVoice {
        let mut sound = Sound::new(259);
        sound.shift();
        sound.shifts[0].expect("accepted shift")
    }

    fn transfer_samples(voice: &mut ShiftVoice, cadence: f32, frames: u32) -> Vec<[f32; 2]> {
        (0..frames)
            .map(|_| [voice.next(cadence).map_or(0.0, |(sample, _)| sample); 2])
            .collect()
    }

    #[test]
    fn shifting_includes_chain_movement_after_the_lever_and_then_settles() {
        for clip in TRANSFERS {
            let mut voice = ShiftVoice {
                clip,
                ..shift_voice()
            };
            let _ = transfer_samples(&mut voice, 90.0, RATE * 35 / 100);
            let chain = transfer_samples(&mut voice, 90.0, RATE / 2);
            assert!(
                energy(&chain) > 0.001,
                "the chain must remain audible after the lever ends"
            );
            let _ = transfer_samples(&mut voice, 90.0, RATE);
            assert!(
                voice.next(90.0).is_none(),
                "the chain must seat rather than loop"
            );
        }
    }

    #[test]
    fn coasting_defers_chain_transfer_until_pedaling_resumes() {
        let mut voice = shift_voice();
        let _ = transfer_samples(&mut voice, 0.0, RATE);
        let stopped = transfer_samples(&mut voice, 0.0, RATE);
        assert!(energy(&stopped) < f32::EPSILON);
        let resumed = transfer_samples(&mut voice, 90.0, RATE);
        assert!(
            energy(&resumed) > 0.001,
            "a shift while coasting must engage the chain when pedaling resumes"
        );
        let _ = transfer_samples(&mut voice, 0.0, RATE);
        let stopped = transfer_samples(&mut voice, 0.0, RATE / 2);
        assert!(
            energy(&stopped) < f32::EPSILON,
            "stopping the pedals stops the transferring chain too"
        );
        let _ = transfer_samples(&mut voice, 90.0, RATE);
        assert!(voice.next(90.0).is_none());
    }

    #[test]
    fn faster_cadence_moves_the_chain_between_gears_sooner() {
        let duration = |cadence| {
            let mut voice = shift_voice();
            (0..RATE * 4)
                .take_while(|_| voice.next(cadence).is_some())
                .count()
        };
        let slow = duration(60.0);
        let fast = duration(120.0);
        assert!(slow > fast * 3 / 2 && slow < (RATE * 3) as usize);
    }

    fn energy(samples: &[[f32; 2]]) -> f32 {
        #[allow(clippy::cast_precision_loss)]
        let count = samples.len() as f32;
        samples
            .iter()
            .map(|s| s[0] * s[0] + s[1] * s[1])
            .sum::<f32>()
            / count
    }

    fn crossings(samples: &[[f32; 2]]) -> usize {
        samples
            .windows(2)
            .filter(|pair| pair[0][0] * pair[1][0] < 0.0)
            .count()
    }

    fn settled(motion: Motion, settings: Settings) -> Vec<[f32; 2]> {
        let mut sound = Sound::new(259);
        let _ = sound.render(RATE as usize * 2, motion, settings);
        sound.render(RATE as usize * 2, motion, settings)
    }

    #[test]
    fn wind_pressure_rises_with_speed_without_replaying_a_loop() {
        let settings = Settings {
            bike: 0.0,
            ..Settings::default()
        };
        let slow = settled(moving(3.0, 90.0), settings);
        let fast = settled(moving(18.0, 90.0), settings);
        assert!(energy(&fast) > energy(&slow) * 5.0);
        let mut sound = Sound::new(259);
        let first = sound.render(RATE as usize * 4, moving(10.0, 90.0), settings);
        let next = sound.render(RATE as usize * 4, moving(10.0, 90.0), settings);
        assert_ne!(first, next, "steady riding must not repeat a wind buffer");
        assert!(fast.iter().any(|s| (s[0] - s[1]).abs() > 0.001));
    }

    #[test]
    fn chain_follows_cadence_and_coasting_uses_freewheel() {
        let settings = Settings {
            wind: 0.0,
            ..Settings::default()
        };
        let pedaling = settled(moving(8.0, 90.0), settings);
        let coasting = settled(moving(8.0, 0.0), settings);
        let stopped = settled(moving(0.0, 0.0), settings);
        assert!(energy(&pedaling) > 0.00001);
        assert!(energy(&coasting) > 0.00001);
        assert!(energy(&stopped) < 0.000_000_01);
        let faster_chain = settled(moving(8.0, 120.0), settings);
        assert!(
            crossings(&faster_chain) > crossings(&pedaling) * 6 / 5,
            "higher cadence speeds up the chain texture, rather than just turning it up"
        );
        let faster_freewheel = settled(moving(16.0, 0.0), settings);
        assert!(
            crossings(&faster_freewheel) > crossings(&coasting) * 3 / 2,
            "freewheel tick rate follows wheel speed: {} versus {} crossings",
            crossings(&faster_freewheel),
            crossings(&coasting)
        );
        let mut sound = Sound::new(259);
        let _ = sound.render(RATE as usize, moving(8.0, 90.0), settings);
        assert!(sound.pedal > 0.99 && sound.coasting < 0.01);
        let _ = sound.render(RATE as usize, moving(8.0, 0.0), settings);
        assert!(sound.coasting > 0.99 && sound.pedal < 0.01);
    }

    #[test]
    fn mute_and_invalid_telemetry_are_safe() {
        let muted = Settings {
            wind: 0.0,
            bike: 0.0,
            weather: Weather::Rain,
        };
        assert!(
            settled(moving(30.0, 180.0), muted)
                .iter()
                .flatten()
                .all(|s| s.abs() < f32::EPSILON)
        );
        let motion = Motion {
            speed: MetersPerSecond(f64::NAN),
            cadence: Some(Rpm(f64::INFINITY)),
            power: None,
        };
        assert!(
            settled(motion, Settings::default())
                .iter()
                .flatten()
                .all(|s| s.is_finite() && s.abs() < 1.0)
        );
        assert_eq!(
            moving(8.0, 0.0).targets()[1],
            0.0,
            "zero cadence wins over stale power"
        );
        assert!(
            Motion {
                cadence: None,
                ..moving(8.0, 0.0)
            }
            .targets()[1]
                > 0.0
        );
    }

    #[test]
    fn buffer_boundaries_keep_the_stream_continuous_and_restart_fades_in() {
        let motion = moving(10.0, 90.0);
        let settings = Settings::default();
        let mut whole = Sound::new(259);
        let expected = whole.render(4_096, motion, settings);
        let mut split = Sound::new(259);
        let mut actual = split.render(1_024, motion, settings);
        actual.extend(split.render(3_072, motion, settings));
        assert_eq!(
            actual, expected,
            "buffer size must not restart grains or gusts"
        );
        let _ = split.render(RATE as usize, motion, settings);
        split.restart();
        let first = split.render(1, motion, settings);
        assert!(
            first[0].iter().all(|sample| sample.abs() < 0.001),
            "resume must fade in instead of starting at the old full volume"
        );
    }
}
