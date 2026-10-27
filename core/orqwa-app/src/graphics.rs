//! Graphics calibration from rendered frames (R151, #353). The front end draws the same
//! offline scene at each preset; this policy excludes warm-up and chooses for smooth 60 fps.

use std::time::Duration;

use crate::GraphicsQuality;

/// Seconds to compile shaders and settle each preset before measuring it.
const WARMUP: f64 = 3.0;
/// Seconds of rendered frames to measure per preset.
const SAMPLE: f64 = 3.0;
/// A little scheduling tolerance around the 60 fps budget, including 59.94 Hz displays.
const BUDGET: f64 = 1.05 / 60.0;

/// The result of one preset's measured frames.
#[derive(Debug, Clone, Copy)]
pub struct Measurement {
    /// The preset that was drawn, with its details following Auto.
    pub quality: GraphicsQuality,
    /// 95th percentile frame time, excluding shader warm-up.
    pub p95: Duration,
    /// Mean rendered frames per second.
    pub fps: f64,
}

/// One cancellable run. No settings are written until the front end applies a complete run.
#[derive(Debug, Default)]
pub struct Benchmark {
    warmup: f64,
    elapsed: f64,
    frames: Vec<f64>,
    measurements: Vec<Measurement>,
}

impl Benchmark {
    /// The preset to draw next; `None` after all four have been measured.
    #[must_use]
    pub fn quality(&self) -> Option<GraphicsQuality> {
        GraphicsQuality::ALL.get(self.measurements.len()).copied()
    }

    /// Records wall time between completed frames, in seconds. Invalid durations are ignored.
    /// Returns true when the front end must switch presets (or the run has finished).
    pub fn frame(&mut self, seconds: f64) -> bool {
        let Some(quality) = self.quality() else {
            return false;
        };
        if !seconds.is_finite() || seconds <= 0.0 || seconds > 60.0 {
            return false;
        }
        if self.warmup < WARMUP {
            self.warmup += seconds;
            return false;
        }
        self.elapsed += seconds;
        self.frames.push(seconds);
        if self.elapsed < SAMPLE {
            return false;
        }
        self.frames.sort_by(f64::total_cmp);
        let index = (self.frames.len() * 95 / 100).min(self.frames.len() - 1);
        #[allow(clippy::cast_precision_loss)] // a few thousand frames at most
        let fps = self.frames.len() as f64 / self.frames.iter().sum::<f64>();
        self.measurements.push(Measurement {
            quality,
            p95: Duration::from_secs_f64(self.frames[index]),
            fps,
        });
        self.frames.clear();
        self.warmup = 0.0;
        self.elapsed = 0.0;
        true
    }

    /// Measurements so far, from Low to Ultra.
    #[must_use]
    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    /// Highest preset whose slow frames meet the budget, or Low if none do. An incomplete
    /// run has no recommendation, including when cancelled or no frames were rendered.
    #[must_use]
    pub fn recommendation(&self) -> Option<Measurement> {
        if self.quality().is_some() {
            return None;
        }
        self.measurements
            .iter()
            .rev()
            .find(|sample| sample.p95.as_secs_f64() <= BUDGET)
            .or_else(|| self.measurements.first())
            .copied()
    }

    /// Whether this measurement met the smooth 60 fps target.
    #[must_use]
    pub fn meets_target(measurement: Measurement) -> bool {
        measurement.p95.as_secs_f64() <= BUDGET
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(run: &mut Benchmark, seconds: f64) {
        // Slow shader compilation is deliberately excluded from the recommendation.
        assert!(!run.frame(WARMUP));
        while !run.frame(seconds) {}
    }

    #[test]
    fn chooses_highest_preset_that_keeps_slow_frames_within_budget() {
        let mut run = Benchmark::default();
        for seconds in [0.008, 0.012, 0.016, 0.025] {
            measure(&mut run, seconds);
        }
        let chosen = run.recommendation().unwrap();
        assert_eq!(chosen.quality, GraphicsQuality::High);
        assert!(Benchmark::meets_target(chosen));
        assert_eq!(run.measurements().len(), 4);
    }

    #[test]
    fn low_is_the_fallback_and_ultra_is_available_on_fast_hardware() {
        for (seconds, expected, target) in [
            (0.05, GraphicsQuality::Low, false),
            (0.005, GraphicsQuality::Ultra, true),
        ] {
            let mut run = Benchmark::default();
            for _ in GraphicsQuality::ALL {
                measure(&mut run, seconds);
            }
            let chosen = run.recommendation().unwrap();
            assert_eq!(chosen.quality, expected);
            assert_eq!(Benchmark::meets_target(chosen), target);
            assert!(!run.frame(seconds), "completed runs ignore later frames");
        }
    }

    #[test]
    fn unfinished_and_invalid_samples_cannot_change_settings() {
        let mut run = Benchmark::default();
        for seconds in [0.0, -1.0, f64::NAN, f64::INFINITY, f64::MAX] {
            assert!(!run.frame(seconds));
        }
        assert!(run.measurements().is_empty());
        assert!(run.recommendation().is_none());
        measure(&mut run, 0.01);
        assert!(
            run.recommendation().is_none(),
            "cancellation is not a result"
        );
    }

    #[test]
    fn shader_compilation_overrun_never_shortens_measurement() {
        let mut run = Benchmark::default();
        assert!(!run.frame(20.0));
        for _ in 0..100 {
            assert!(!run.frame(0.01));
        }
        assert!(
            run.measurements().is_empty(),
            "one second is not a full sample"
        );
        while !run.frame(0.01) {}
        assert_eq!(run.measurements().len(), 1);
        assert_eq!(run.quality(), Some(GraphicsQuality::Medium));
    }

    #[test]
    fn repeated_stutters_fail_even_when_the_mean_is_fast() {
        let mut run = Benchmark::default();
        measure(&mut run, 0.01);
        for _ in 1..4 {
            assert!(!run.frame(WARMUP));
            let mut index = 0;
            loop {
                let done = run.frame(if index % 10 == 0 { 0.05 } else { 0.005 });
                index += 1;
                if done {
                    break;
                }
            }
        }
        assert_eq!(run.recommendation().unwrap().quality, GraphicsQuality::Low);
        assert!(run.measurements()[3].fps > 60.0);
    }
}
