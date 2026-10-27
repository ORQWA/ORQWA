//! Bake the CC0 previews into small mono PCM textures. See `audio/README.md`.

use std::path::Path;

use orqwa_video::audio::Audio;

fn main() {
    let source = std::env::args().nth(1).expect("source directory");
    let only = std::env::args().nth(2);
    let destination = Path::new(env!("CARGO_MANIFEST_DIR")).join("audio");
    std::fs::create_dir_all(&destination).unwrap();
    for (name, recording, from_ms, duration_ms) in [
        ("wind", "wind", 12_000_u32, 36_000_u32),
        ("rain", "rain", 2_000, 20_000),
        ("chain", "bike", 8_000, 16_000),
        ("freewheel", "freewheel", 0, 2_000),
        ("shift", "shift", 80, 300),
        ("shift-chain-1", "transfer", 6_650, 1_100),
        ("shift-chain-2", "transfer", 8_050, 1_100),
        ("shift-chain-3", "transfer", 10_650, 1_100),
    ] {
        if only
            .as_ref()
            .is_some_and(|only| only != name && only != recording)
        {
            continue;
        }
        let mut audio = Audio::open(&Path::new(&source).join(format!("{recording}.mp3")))
            .unwrap()
            .unwrap();
        let mut frames = vec![[0.0; 2]; audio.rate() as usize * duration_ms as usize / 1_000 + 1];
        audio
            .read(
                i64::from(audio.rate()) * i64::from(from_ms) / 1_000,
                &mut frames,
            )
            .unwrap();
        let samples = prepare(&frames, audio.rate(), duration_ms);
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        std::fs::write(destination.join(format!("{name}.pcm")), bytes).unwrap();
        println!("{name}: {duration_ms} ms, 24000 Hz mono PCM16");
    }
}

#[allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss
)]
// Offline asset processing: all indices are bounded by the decoded recording and samples
// are explicitly clamped to PCM16. f64 carries exact indices for these short recordings.
fn prepare(frames: &[[f32; 2]], rate: u32, duration_ms: u32) -> Vec<i16> {
    let mut previous = 0.0;
    let mut high = 0.0;
    let mut low = [0.0; 2];
    let high_coefficient = (-std::f32::consts::TAU * 45.0 / rate as f32).exp();
    let low_coefficient = 1.0 - (-std::f32::consts::TAU * 8_000.0 / rate as f32).exp();
    let filtered: Vec<f32> = frames
        .iter()
        .map(|s| {
            let mono = f32::midpoint(s[0], s[1]);
            high = high_coefficient * (high + mono - previous);
            previous = mono;
            low[0] += (high - low[0]) * low_coefficient;
            low[1] += (low[0] - low[1]) * low_coefficient;
            low[1]
        })
        .collect();
    let samples: Vec<f32> = (0..duration_ms as usize * 24)
        .map(|i| {
            let position = i as f64 * f64::from(rate) / 24_000.0;
            let index = position as usize;
            let fraction = position.fract() as f32;
            filtered[index] * (1.0 - fraction) + filtered[index + 1] * fraction
        })
        .collect();
    let rms = (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt();
    let peak = samples.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    let gain = (0.18 / rms.max(0.000_001)).min(0.85 / peak.max(0.000_001));
    samples
        .into_iter()
        .map(|s| (s * gain * 32_767.0).round() as i16)
        .collect()
}
