//! Render a listening preview using exactly the live ride mixer (no trainer or Godot needed).

use std::io::Write;

use orqwa_app::ambient::{Motion, RATE, Settings, Sound};
use orqwa_domain::units::{MetersPerSecond, Rpm, Watts};

fn main() -> std::io::Result<()> {
    let output = std::env::args().nth(1).expect("output WAV path");
    let mut sound = Sound::new(259);
    let mut samples = Vec::new();
    for frame in 0..30 * 50 {
        let time = f64::from(frame) / 50.0;
        let (speed, cadence, power) = if time < 3.0 {
            (0.0, 0.0, 0.0)
        } else if time < 14.0 {
            (
                ((time - 3.0) * 1.4).min(14.0),
                60.0 + (time - 3.0) * 5.0,
                250.0,
            )
        } else if time < 24.0 {
            (14.0 - (time - 14.0) * 0.6, 0.0, 0.0)
        } else {
            ((8.0 - (time - 24.0) * 2.0).max(0.0), 0.0, 0.0)
        };
        samples.extend(sound.render(
            (RATE / 50) as usize,
            Motion {
                speed: MetersPerSecond(speed),
                cadence: Some(Rpm(cadence)),
                power: Some(Watts(power)),
            },
            Settings::default(),
        ));
    }
    let mut file = std::fs::File::create(output)?;
    let size = u32::try_from(samples.len() * 4).expect("short preview");
    file.write_all(b"RIFF")?;
    file.write_all(&(size + 36).to_le_bytes())?;
    file.write_all(b"WAVEfmt \x10\0\0\0\x01\0\x02\0")?;
    file.write_all(&RATE.to_le_bytes())?;
    file.write_all(&(RATE * 4).to_le_bytes())?;
    file.write_all(b"\x04\0\x10\0data")?;
    file.write_all(&size.to_le_bytes())?;
    for frame in samples {
        for sample in frame {
            #[allow(clippy::cast_possible_truncation)] // mixer keeps samples within PCM16
            let value = (sample * 32_767.0).round() as i16;
            file.write_all(&value.to_le_bytes())?;
        }
    }
    Ok(())
}
