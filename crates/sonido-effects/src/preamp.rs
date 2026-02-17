//! Clean preamp stage
//!
//! High-headroom, zero-latency preamp that handles hot signals
//! without clipping until you want it to.

use libm::tanhf;
use sonido_core::{
    Effect, ParamDescriptor, ParamId, ParamUnit, SmoothedParam, db_to_linear, linear_to_db,
};

/// Clean preamp stage
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Gain | -20.0–20.0 dB | 0.0 |
/// | 1 | Output | -20.0–20.0 dB | 0.0 |
/// | 2 | Headroom | 6.0–40.0 dB | 20.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::CleanPreamp;
/// use sonido_core::Effect;
///
/// let mut preamp = CleanPreamp::new(48000.0);
/// preamp.set_gain_db(12.0);
/// preamp.set_output_db(-6.0);
///
/// let input = 0.5;
/// let output = preamp.process(input);
/// ```
pub struct CleanPreamp {
    /// Input gain with smoothing
    gain: SmoothedParam,
    /// Output level with smoothing
    output: SmoothedParam,
    /// Headroom in dB before clipping
    headroom_db: f32,
    /// Cached linear threshold derived from `headroom_db` (avoids per-sample `powf`)
    cached_threshold: f32,
    /// Sample rate
    sample_rate: f32,
}

impl Default for CleanPreamp {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl CleanPreamp {
    /// Create new clean preamp
    pub fn new(sample_rate: f32) -> Self {
        Self {
            gain: SmoothedParam::standard(1.0, sample_rate),
            output: SmoothedParam::standard(1.0, sample_rate),
            headroom_db: 20.0, // +20dB headroom
            cached_threshold: db_to_linear(20.0),
            sample_rate,
        }
    }

    /// Set gain in dB
    pub fn set_gain_db(&mut self, db: f32) {
        self.gain.set_target(db_to_linear(db));
    }

    /// Get current gain in dB
    pub fn gain_db(&self) -> f32 {
        linear_to_db(self.gain.target())
    }

    /// Set output in dB
    pub fn set_output_db(&mut self, db: f32) {
        self.output.set_target(db_to_linear(db));
    }

    /// Get current output in dB
    pub fn output_db(&self) -> f32 {
        linear_to_db(self.output.target())
    }

    /// Set headroom in dB
    pub fn set_headroom_db(&mut self, db: f32) {
        self.headroom_db = db.clamp(6.0, 40.0);
        self.cached_threshold = db_to_linear(self.headroom_db);
    }

    /// Get current headroom in dB
    pub fn headroom_db(&self) -> f32 {
        self.headroom_db
    }
}

impl Effect for CleanPreamp {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let gain = self.gain.advance();
        let output_level = self.output.advance();

        // Simple gain stage - clean until clipping threshold
        let gained = input * gain;
        let threshold = self.cached_threshold;

        // Soft clip only at extreme levels
        let output = if gained.abs() > threshold {
            threshold * gained.signum() * (1.0 + tanhf(gained.abs() / threshold - 1.0))
        } else {
            gained
        };

        output * output_level
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Dual-mono: process each channel independently with same settings
        let gain = self.gain.advance();
        let output_level = self.output.advance();
        let threshold = self.cached_threshold;

        // Process left
        let gained_l = left * gain;
        let out_l = if gained_l.abs() > threshold {
            threshold * gained_l.signum() * (1.0 + tanhf(gained_l.abs() / threshold - 1.0))
        } else {
            gained_l
        } * output_level;

        // Process right
        let gained_r = right * gain;
        let out_r = if gained_r.abs() > threshold {
            threshold * gained_r.signum() * (1.0 + tanhf(gained_r.abs() / threshold - 1.0))
        } else {
            gained_r
        } * output_level;

        (out_l, out_r)
    }

    fn reset(&mut self) {
        self.gain.snap_to_target();
        self.output.snap_to_target();
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.gain.set_sample_rate(sample_rate);
        self.output.set_sample_rate(sample_rate);
    }

    fn latency_samples(&self) -> usize {
        0 // Zero latency
    }
}

sonido_core::impl_params! {
    CleanPreamp, this {
        [0] ParamDescriptor::gain_db("Gain", "Gain", -20.0, 20.0, 0.0)
                .with_id(ParamId(100), "pre_gain"),
            get: this.gain_db(),
            set: |v| this.set_gain_db(v);

        [1] ParamDescriptor::gain_db("Output", "Output", -20.0, 20.0, 0.0)
                .with_id(ParamId(101), "pre_output"),
            get: this.output_db(),
            set: |v| this.set_output_db(v);

        [2] ParamDescriptor {
                name: "Headroom",
                short_name: "Hroom",
                unit: ParamUnit::Decibels,
                min: 6.0,
                max: 40.0,
                default: 20.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(102), "pre_headroom"),
            get: this.headroom_db,
            set: |v| this.set_headroom_db(v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamp_unity() {
        let mut preamp = CleanPreamp::new(48000.0);
        preamp.set_gain_db(0.0);
        preamp.set_output_db(0.0);
        preamp.reset(); // Snap to target for test

        let output = preamp.process(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_preamp_gain() {
        let mut preamp = CleanPreamp::new(48000.0);
        preamp.set_gain_db(6.0); // ~2x gain
        preamp.set_output_db(0.0);
        preamp.reset(); // Snap to target for test

        let output = preamp.process(0.25);
        assert!((output - 0.5).abs() < 0.05, "Expected ~0.5, got {}", output);
    }

    #[test]
    fn test_preamp_soft_clip() {
        let mut preamp = CleanPreamp::new(48000.0);
        preamp.set_gain_db(40.0); // Heavy gain
        preamp.set_headroom_db(6.0); // Low headroom
        preamp.reset(); // Snap to target for test

        // Should soft clip, not hard clip
        let output = preamp.process(0.5);
        assert!(output.is_finite());
        // Should be limited but not exactly at threshold
    }

    #[test]
    fn test_preamp_zero_latency() {
        let preamp = CleanPreamp::new(48000.0);
        assert_eq!(preamp.latency_samples(), 0);
    }

    #[test]
    fn test_preamp_smoothing() {
        let mut preamp = CleanPreamp::new(48000.0);
        preamp.set_gain_db(0.0);
        preamp.reset();

        // Set new gain target
        preamp.set_gain_db(12.0);

        // First sample should not be at full gain yet
        let first = preamp.process(0.5);

        // Process more samples to let smoothing settle
        for _ in 0..1000 {
            preamp.process(0.5);
        }

        let settled = preamp.process(0.5);

        // Settled value should be higher than first (more gain applied)
        assert!(settled > first, "Smoothing should gradually increase gain");
    }
}
