//! Experimental local WebRTC AEC3, separate from codecs and group transport.

use orqwa_domain::voice::{FRAME_SAMPLES, PcmFrame, SAMPLE_RATE};
use webrtc_audio_processing::{Config, Error, Processor, config::EchoCanceller};

/// WebRTC works in 10 ms blocks; the transport keeps its existing 20 ms packets.
const BLOCK: usize = FRAME_SAMPLES / 2;

pub(crate) struct Canceller {
    processor: Processor,
}

impl Canceller {
    pub(crate) fn new() -> Result<Self, Error> {
        let processor = Processor::new(SAMPLE_RATE)?;
        processor.set_config(Config {
            echo_canceller: Some(EchoCanceller::default()),
            ..Config::default()
        });
        Ok(Self { processor })
    }

    pub(crate) fn render(&self, frame: &PcmFrame) -> Result<(), Error> {
        for block in frame.as_chunks::<BLOCK>().0 {
            self.processor.analyze_render_frame([block])?;
        }
        Ok(())
    }

    pub(crate) fn capture(&self, frame: &mut PcmFrame) -> Result<(), Error> {
        for block in frame.as_chunks_mut::<BLOCK>().0 {
            self.processor.process_capture_frame([block])?;
        }
        // The codec boundary must remain finite even if a native processor misbehaves.
        for sample in frame {
            *sample = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    #[test]
    fn silence_stays_finite_and_silent() {
        let canceller = Canceller::new().unwrap();
        for _ in 0..50 {
            let mut frame = [0.0; FRAME_SAMPLES];
            canceller.render(&frame).unwrap();
            canceller.capture(&mut frame).unwrap();
            assert!(frame.iter().all(|sample| sample.abs() < 1e-6));
        }
    }

    #[test]
    fn delayed_room_echo_is_reduced_and_a_local_reply_survives_overlap() {
        let canceller = Canceller::new().unwrap();
        let mut history = VecDeque::from([[0.0; FRAME_SAMPLES]; 4]);
        let mut random = 7_u32;
        let mut filtered = 0.0_f32;
        let mut echo_energy = 0.0_f64;
        let mut residual_energy = 0.0_f64;
        let mut local_energy = 0.0_f64;
        let mut local_samples = Vec::new();
        let mut processed_samples = Vec::new();
        for frame_index in 0..450 {
            let render = std::array::from_fn(|_| {
                random = random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                #[allow(clippy::cast_precision_loss)] // bounded synthetic audio fixture
                let noise = (random >> 8) as f32 / 16_777_216.0 - 0.5;
                filtered = filtered * 0.8 + noise * 0.2;
                filtered
            });
            let echo = history.pop_front().unwrap();
            history.push_back(render);
            canceller.render(&render).unwrap();
            let local: PcmFrame = std::array::from_fn(|sample| {
                if frame_index < 250 {
                    return 0.0;
                }
                #[allow(clippy::cast_precision_loss)] // under ten seconds of synthetic speech
                let time = (frame_index * FRAME_SAMPLES + sample) as f32 / SAMPLE_RATE as f32;
                let phase = std::f32::consts::TAU * time;
                let envelope = 0.6 + 0.4 * (phase * 3.0).sin();
                envelope * (0.12 * (phase * 277.0).sin() + 0.04 * (phase * 554.0).sin())
            });
            let mut capture = std::array::from_fn(|sample| echo[sample] * 0.6 + local[sample]);
            canceller.capture(&mut capture).unwrap();
            for sample in 0..FRAME_SAMPLES {
                if (150..250).contains(&frame_index) {
                    echo_energy += f64::from(echo[sample] * 0.6).powi(2);
                    residual_energy += f64::from(capture[sample]).powi(2);
                }
                if frame_index >= 300 {
                    local_energy += f64::from(local[sample]).powi(2);
                    local_samples.push(local[sample]);
                    processed_samples.push(capture[sample]);
                }
            }
        }
        assert!(
            residual_energy < echo_energy * 0.1,
            "at least 10 dB echo reduction after adaptation: {residual_energy}/{echo_energy}"
        );
        // AEC's filter bank introduces a short signal delay. Compare the local reply at
        // lags below one processing block instead of requiring its waveform at zero lag.
        let local_projection = (0..BLOCK)
            .map(|lag| {
                local_samples
                    .iter()
                    .zip(processed_samples.iter().skip(lag))
                    .step_by(8)
                    .map(|(local, processed)| f64::from(*local) * f64::from(*processed) * 8.0)
                    .sum::<f64>()
            })
            .fold(0.0_f64, f64::max);
        assert!(
            local_projection > local_energy * 0.35,
            "a local reply must survive simultaneous remote audio: {local_projection}/{local_energy}"
        );
    }
}
