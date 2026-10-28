//! A narrow, bounded interface to the bundled hardened Opus implementation.

use opusic_c::{Application, Bitrate, Channels, Decoder, Encoder, SampleRate};
use orqwa_domain::voice::{FRAME_SAMPLES, PcmFrame};
use orqwa_net::state::RiderId;
use orqwa_net::voice::{MAX_AUDIO_BYTES, VoiceFrame};

/// Invalid speech data or an unavailable codec. Audio is never included in diagnostics.
#[derive(Debug, thiserror::Error)]
#[error("The speech frame could not be encoded or decoded.")]
pub struct CodecError;

/// One mono, constant-rate 24 kbit/s encoder, reused across every recipient.
pub struct SpeechEncoder {
    inner: Encoder,
    spurt: u32,
    sequence: u32,
}
impl SpeechEncoder {
    /// Creates the bounded 20 ms voice configuration with native hardening left enabled.
    ///
    /// # Errors
    /// Returns [`CodecError`] if the codec cannot be initialized.
    pub fn new() -> Result<Self, CodecError> {
        let mut inner = Encoder::new(Channels::Mono, SampleRate::Hz48000, Application::Voip)
            .map_err(|_| CodecError)?;
        inner
            .set_bitrate(Bitrate::Value(24_000))
            .map_err(|_| CodecError)?;
        inner.set_vbr(false).map_err(|_| CodecError)?;
        inner.set_dtx(false).map_err(|_| CodecError)?;
        inner.set_complexity(5).map_err(|_| CodecError)?;
        Ok(Self {
            inner,
            spurt: 0,
            sequence: 0,
        })
    }
    /// Encodes finite samples into a fixed payload. Activation changes reset codec prediction.
    ///
    /// # Errors
    /// Returns [`CodecError`] for invalid samples, activation, exhaustion or codec failure.
    pub fn encode(
        &mut self,
        samples: &PcmFrame,
        speaker: RiderId,
        spurt: u32,
    ) -> Result<VoiceFrame, CodecError> {
        if spurt == 0
            || samples
                .iter()
                .any(|v| !v.is_finite() || !(-1.0..=1.0).contains(v))
        {
            return Err(CodecError);
        }
        if spurt != self.spurt {
            self.inner.reset().map_err(|_| CodecError)?;
            self.spurt = spurt;
            self.sequence = 0;
        }
        let mut bytes = [0; MAX_AUDIO_BYTES];
        let len = self
            .inner
            .encode_float_to_slice(samples, &mut bytes)
            .map_err(|_| CodecError)?;
        // CBR without DTX gives precisely 60 compressed bytes per 20 ms frame.
        if len != 60 {
            return Err(CodecError);
        }
        let frame = VoiceFrame::new(speaker, spurt, self.sequence, &bytes[..len])
            .map_err(|_| CodecError)?;
        self.sequence = self.sequence.checked_add(1).ok_or(CodecError)?;
        Ok(frame)
    }
}

/// One speaker's decoder. No decoder state is shared between riders.
pub struct SpeechDecoder {
    inner: Decoder,
    spurt: u32,
}
impl SpeechDecoder {
    /// Initializes one mono decoder with a fixed 960-sample output capacity.
    ///
    /// # Errors
    /// Returns [`CodecError`] when the codec cannot be initialized.
    pub fn new() -> Result<Self, CodecError> {
        Ok(Self {
            inner: Decoder::new(Channels::Mono, SampleRate::Hz48000).map_err(|_| CodecError)?,
            spurt: 0,
        })
    }
    /// Rejects stereo and packets representing anything other than exactly twenty ms before
    /// decoding. Uses fixed slices rather than the wrapper's growable-vector helpers.
    ///
    /// # Errors
    /// Returns [`CodecError`] for malformed/composite-duration/stereo data or decoder failure.
    pub fn decode(&mut self, frame: &VoiceFrame) -> Result<PcmFrame, CodecError> {
        let bytes = frame.payload();
        if bytes[0] & 4 != 0
            || opusic_c::utils::get_nb_samples(bytes, SampleRate::Hz48000)
                .map_err(|_| CodecError)?
                != FRAME_SAMPLES
        {
            return Err(CodecError);
        }
        if self.spurt != frame.spurt() {
            self.inner.reset().map_err(|_| CodecError)?;
            self.spurt = frame.spurt();
        }
        self.decode_bytes(bytes)
    }
    /// Conceals one lost twenty-millisecond frame; callers bound concealment to three frames.
    ///
    /// # Errors
    /// Returns [`CodecError`] if the decoder cannot conceal the frame.
    pub fn conceal(&mut self) -> Result<PcmFrame, CodecError> {
        self.decode_bytes(&[])
    }
    fn decode_bytes(&mut self, bytes: &[u8]) -> Result<PcmFrame, CodecError> {
        let mut samples = [0.0; FRAME_SAMPLES];
        let len = self
            .inner
            .decode_float_to_slice(bytes, &mut samples, false)
            .map_err(|_| CodecError)?;
        if len != FRAME_SAMPLES || samples.iter().any(|s| !s.is_finite()) {
            return Err(CodecError);
        }
        for sample in &mut samples {
            *sample = sample.clamp(-1.0, 1.0);
        }
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_mono_speech_round_trips_and_rejects_wrong_durations_and_stereo() {
        let mut encoder = SpeechEncoder::new().unwrap();
        let mut decoder = SpeechDecoder::new().unwrap();
        let samples = std::array::from_fn(|index| {
            #[allow(clippy::cast_precision_loss)]
            let t = index as f32 / 48_000.0;
            (t * 440.0 * std::f32::consts::TAU).sin() * 0.2
        });
        let frame = encoder.encode(&samples, RiderId::HOST, 1).unwrap();
        assert_eq!(frame.payload().len(), 60);
        let recovered = decoder.decode(&frame).unwrap();
        assert!(recovered.iter().any(|v| v.abs() > 0.01));
        let mut stereo = frame.payload().to_vec();
        stereo[0] |= 4;
        assert!(
            decoder
                .decode(&VoiceFrame::new(RiderId::HOST, 1, 1, &stereo).unwrap())
                .is_err()
        );
        assert!(
            decoder
                .decode(&VoiceFrame::new(RiderId::HOST, 1, 1, &[0xff]).unwrap())
                .is_err()
        );
        assert!(
            encoder
                .encode(&[f32::NAN; FRAME_SAMPLES], RiderId::HOST, 1)
                .is_err()
        );
    }
}
