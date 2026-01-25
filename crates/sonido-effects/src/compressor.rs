//! Dynamics compressor with envelope follower and gain reduction.

use sonido_core::{Effect, SmoothedParam, EnvelopeFollower};
use libm::{log10f, powf};

/// Converts linear amplitude to decibels.
#[inline]
fn linear_to_db(linear: f32) -> f32 {
    20.0 * log10f(linear.max(1e-6))
}

/// Converts decibels to linear amplitude.
#[inline]
fn db_to_linear(db: f32) -> f32 {
    powf(10.0, db / 20.0)
}

/// Gain computer for calculating compression curve.
#[derive(Debug, Clone)]
struct GainComputer {
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
}

impl GainComputer {
    fn new() -> Self {
        Self {
            threshold_db: -20.0,
            ratio: 4.0,
            knee_db: 6.0,
        }
    }

    #[inline]
    fn compute_gain_db(&self, input_db: f32) -> f32 {
        let overshoot = input_db - self.threshold_db;

        if overshoot <= -self.knee_db / 2.0 {
            0.0
        } else if overshoot > self.knee_db / 2.0 {
            let gain_reduction = overshoot * (1.0 - 1.0 / self.ratio);
            -gain_reduction
        } else {
            let knee_factor = (overshoot + self.knee_db / 2.0) / self.knee_db;
            let gain_reduction = knee_factor * knee_factor * overshoot * (1.0 - 1.0 / self.ratio);
            -gain_reduction
        }
    }
}

/// Dynamics compressor effect.
///
/// # Example
///
/// ```rust
/// use sonido_effects::Compressor;
/// use sonido_core::Effect;
///
/// let mut comp = Compressor::new(44100.0);
/// comp.set_threshold_db(-20.0);
/// comp.set_ratio(4.0);
/// comp.set_attack_ms(5.0);
/// comp.set_release_ms(50.0);
///
/// let input = 0.5;
/// let output = comp.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Compressor {
    envelope_follower: EnvelopeFollower,
    gain_computer: GainComputer,
    makeup_gain: SmoothedParam,
    sample_rate: f32,
}

impl Compressor {
    /// Create a new compressor with default settings.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope_follower: EnvelopeFollower::new(sample_rate),
            gain_computer: GainComputer::new(),
            makeup_gain: SmoothedParam::with_config(1.0, sample_rate, 10.0),
            sample_rate,
        }
    }

    /// Set threshold in dB.
    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.gain_computer.threshold_db = threshold_db.clamp(-60.0, 0.0);
    }

    /// Set compression ratio.
    pub fn set_ratio(&mut self, ratio: f32) {
        self.gain_computer.ratio = ratio.clamp(1.0, 20.0);
    }

    /// Set attack time in milliseconds.
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.envelope_follower.set_attack_ms(attack_ms.clamp(0.1, 100.0));
    }

    /// Set release time in milliseconds.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.envelope_follower.set_release_ms(release_ms.clamp(10.0, 1000.0));
    }

    /// Set knee width in dB.
    pub fn set_knee_db(&mut self, knee_db: f32) {
        self.gain_computer.knee_db = knee_db.clamp(0.0, 12.0);
    }

    /// Set makeup gain in dB.
    pub fn set_makeup_gain_db(&mut self, gain_db: f32) {
        let linear = db_to_linear(gain_db.clamp(0.0, 24.0));
        self.makeup_gain.set_target(linear);
    }
}

impl Effect for Compressor {
    fn process(&mut self, input: f32) -> f32 {
        let envelope = self.envelope_follower.process(input);
        let envelope_db = linear_to_db(envelope);
        let gain_reduction_db = self.gain_computer.compute_gain_db(envelope_db);
        let gain_linear = db_to_linear(gain_reduction_db);
        let makeup = self.makeup_gain.next();

        input * gain_linear * makeup
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.envelope_follower.set_sample_rate(sample_rate);
        self.makeup_gain.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.envelope_follower.reset();
        self.makeup_gain.snap_to_target();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_basic() {
        let mut comp = Compressor::new(44100.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);

        for _ in 0..100 {
            let output = comp.process(0.1);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_compressor_reduces_peaks() {
        let mut comp = Compressor::new(44100.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);
        comp.set_attack_ms(1.0);
        comp.reset();

        let mut output = 0.0;
        for _ in 0..1000 {
            output = comp.process(0.5);
        }

        assert!(output.abs() < 0.5, "Output should be compressed, got {}", output);
    }
}
