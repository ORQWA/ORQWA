//! Experimental AEC3 with the existing bounded speaker gate as fallback.
//!
//! The gate listens for a friend's *speech* in what is about to play, not for any sound:
//! see [`Speech`].

use crate::speech::Speech;
use orqwa_domain::voice::PcmFrame;
use std::time::{Duration, Instant};

/// The experiment leaves unsupported backends and platforms on the existing gate.
pub(crate) struct EchoPrevention {
    gate: SpeakerGate,
    enabled: bool,
    supported: bool,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    canceller: Option<crate::acoustic::Canceller>,
    sequence: Option<u64>,
}

impl EchoPrevention {
    pub(crate) fn new(has_reference: bool) -> Self {
        Self {
            gate: SpeakerGate::new(),
            enabled: false,
            supported: has_reference
                && cfg!(any(target_os = "macos", target_os = "linux"))
                && std::env::var_os("ORQWA_VOICE_ECHO_GATE").is_none(),
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            canceller: None,
            sequence: None,
        }
    }

    pub(crate) fn enable(&mut self, enabled: bool) {
        if self.enabled != enabled {
            self.enabled = enabled;
            self.restart();
        }
    }

    pub(crate) fn restart(&mut self) {
        self.gate.restart();
        self.sequence = None;
        if !self.enabled || !self.supported {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            {
                self.canceller = None;
            }
            return;
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            self.canceller = None;
            if self.enabled && self.supported {
                match crate::acoustic::Canceller::new() {
                    Ok(canceller) => {
                        self.canceller = Some(canceller);
                        tracing::info!("voice speaker echo prevention: WebRTC AEC3 (experimental)");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "WebRTC unavailable; using voice speaker gate");
                    }
                }
            }
        }
    }

    pub(crate) fn observe(&mut self, playback: &PcmFrame, now: Instant) {
        self.gate.observe(playback, now);
    }

    pub(crate) fn references(&mut self, audio: &mut dyn orqwa_domain::voice::VoiceAudio) {
        let mut skipped = false;
        for _ in 0..crate::MAX_FRAMES_PER_WAKE {
            let Some(reference) = audio.render_reference() else {
                break;
            };
            if reference.at.elapsed() > crate::CAPTURE_FRESH {
                skipped = true;
                continue;
            }
            if skipped
                || self
                    .sequence
                    .is_some_and(|previous| reference.sequence != previous.wrapping_add(1))
            {
                self.restart();
                skipped = false;
            }
            self.sequence = Some(reference.sequence);
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            if let Some(canceller) = &self.canceller
                && let Err(error) = canceller.render(&reference.samples)
            {
                tracing::warn!(%error, "WebRTC reference failed; using voice speaker gate");
                self.canceller = None;
            }
        }
        if skipped {
            self.restart();
        }
    }

    pub(crate) fn apply(&mut self, samples: &mut PcmFrame, captured: Instant) {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if let Some(canceller) = &self.canceller {
            match canceller.capture(samples) {
                Ok(()) => return,
                Err(error) => {
                    tracing::warn!(%error, "WebRTC capture failed; using voice speaker gate");
                    self.canceller = None;
                }
            }
        }
        self.gate.apply(samples, captured);
    }
}

/// After speech stops its echo still has to leave the queue, the speaker and the room.
const TAIL: Duration = Duration::from_millis(300);
/// Gain change per sample: closing within 5 ms, opening over 20 ms, without clicks.
const CLOSE_STEP: f32 = 1.0 / 240.0;
const OPEN_STEP: f32 = 1.0 / 960.0;

pub(crate) struct SpeakerGate {
    speech: Speech,
    until: Option<Instant>,
    gain: f32,
}

impl SpeakerGate {
    pub(crate) fn new() -> Self {
        Self {
            speech: Speech::default(),
            until: None,
            gain: 1.0,
        }
    }

    /// Notes one frame that is about to play.
    pub(crate) fn observe(&mut self, playback: &PcmFrame, now: Instant) {
        if self.speech.observe(playback) {
            self.until = Some(now + TAIL);
        }
    }

    /// A newly opened microphone fades in, and stays closed if a friend is already talking.
    pub(crate) fn restart(&mut self) {
        self.gain = 0.0;
    }

    /// Fades captured samples out while a friend's speech or its echo is audible.
    pub(crate) fn apply(&mut self, samples: &mut PcmFrame, captured: Instant) {
        let held = self.until.is_some_and(|until| captured < until);
        for sample in samples {
            self.gain = if held {
                (self.gain - CLOSE_STEP).max(0.0)
            } else {
                (self.gain + OPEN_STEP).min(1.0)
            };
            *sample *= self.gain;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::sounds::{fan, voice};
    use orqwa_domain::voice::FRAME_SAMPLES;

    fn passes(gate: &mut SpeakerGate, at: Instant) -> bool {
        let mut samples = [0.5; FRAME_SAMPLES];
        gate.apply(&mut samples, at);
        samples[FRAME_SAMPLES - 1] > 0.49
    }
    fn at(start: Instant, frame: u32) -> Instant {
        start + Duration::from_millis(20) * frame
    }

    #[test]
    fn a_friends_speech_and_its_echo_tail_are_never_retransmitted() {
        let start = Instant::now();
        let mut gate = SpeakerGate::new();
        for index in 0..10 {
            gate.observe(&[0.0; FRAME_SAMPLES], at(start, index));
            assert!(passes(&mut gate, at(start, index)));
        }
        // A single loud frame, such as a click, does not hold the microphone.
        gate.observe(&voice(0.2, 10), at(start, 10));
        assert!(passes(&mut gate, at(start, 10)));
        gate.observe(&voice(0.2, 11), at(start, 11));
        let mut samples = [0.5; FRAME_SAMPLES];
        gate.apply(&mut samples, at(start, 11));
        assert!(samples[FRAME_SAMPLES - 1].abs() < f32::EPSILON);
        // No click: the gain moves by a small step per sample.
        assert!(
            samples
                .windows(2)
                .all(|pair| (pair[0] - pair[1]).abs() <= 0.5 / 200.0)
        );
        gate.observe(&[0.0; FRAME_SAMPLES], at(start, 12));
        assert!(!passes(&mut gate, at(start, 25)));
        // Opening fades in over one frame.
        let _ = passes(&mut gate, at(start, 27));
        assert!(passes(&mut gate, at(start, 28)));
    }

    #[test]
    fn a_friends_fan_never_closes_the_microphone_however_loud() {
        let start = Instant::now();
        let mut gate = SpeakerGate::new();
        for index in 0..100 {
            // From silence straight to a fan at about -20 dBFS.
            let frame = if index < 10 {
                [0.0; FRAME_SAMPLES]
            } else {
                fan(0.35, index)
            };
            gate.observe(&frame, at(start, index));
            assert!(passes(&mut gate, at(start, index)), "frame {index}");
        }
    }

    #[test]
    fn a_steady_whine_is_learned_as_background_and_speech_still_stands_out() {
        let start = Instant::now();
        let mut gate = SpeakerGate::new();
        for index in 0..50 {
            gate.observe(&[0.0; FRAME_SAMPLES], at(start, index));
        }
        // A trainer's whine has a pitch, so at first it holds like speech; within 1.5 s it
        // is known as background.
        for index in 50..200 {
            gate.observe(&voice(0.02, index), at(start, index));
        }
        assert!(passes(&mut gate, at(start, 200)));
        assert!(passes(&mut gate, at(start, 201)));
        // Their speech over the same whine still holds the microphone.
        gate.observe(&voice(0.15, 201), at(start, 201));
        gate.observe(&voice(0.15, 202), at(start, 202));
        assert!(!passes(&mut gate, at(start, 203)));
        // While their gate holds for us, silence arrives; their whine afterwards is still
        // known, so we can answer at once.
        for index in 203..300 {
            gate.observe(&[0.0; FRAME_SAMPLES], at(start, index));
        }
        // Their gate fades in: a frame quieter than their room, then the room itself.
        gate.observe(&voice(0.004, 300), at(start, 300));
        for index in 301..320 {
            gate.observe(&voice(0.02, index), at(start, index));
        }
        assert!(passes(&mut gate, at(start, 320)));
    }
}
