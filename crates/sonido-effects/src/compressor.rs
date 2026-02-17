//! Dynamics compressor with soft-knee characteristics.
//!
//! A feed-forward compressor that reduces dynamic range by attenuating
//! signals above a threshold.
//!
//! # Signal Flow
//!
//! ```text
//! Input → Envelope Follower → Gain Computer → Gain Reduction → Output
//!                                    ↓
//!                              Makeup Gain
//! ```
//!
//! # Parameters
//!
//! | Parameter | Range | Description |
//! |-----------|-------|-------------|
//! | Threshold | -40 to 0 dB | Level where compression begins |
//! | Ratio | 1:1 to 20:1 | Compression strength (∞:1 = limiter) |
//! | Attack | 0.1-100 ms | How fast gain reduction engages |
//! | Release | 10-1000 ms | How fast gain reduction releases |
//! | Makeup | 0-20 dB | Output level compensation |
//!
//! # Tips
//!
//! - **Fast attack** (< 5ms): Catches transients, can sound "squashed"
//! - **Slow attack** (> 20ms): Lets transients through, more natural
//! - **Fast release** (< 100ms): Pumping effect, good for drums
//! - **Slow release** (> 200ms): Smooth, transparent compression

use sonido_core::{
    Effect, EnvelopeFollower, ParamDescriptor, ParamId, ParamUnit, ParameterInfo, SmoothedParam,
    db_to_linear, fast_db_to_linear, fast_linear_to_db, linear_to_db,
};

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
            threshold_db: -18.0,
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
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Threshold | -60.0–0.0 dB | -18.0 |
/// | 1 | Ratio | 1.0–20.0 | 4.0 |
/// | 2 | Attack | 0.1–100.0 ms | 10.0 |
/// | 3 | Release | 10.0–1000.0 ms | 100.0 |
/// | 4 | Makeup Gain | 0.0–24.0 dB | 0.0 |
/// | 5 | Knee | 0.0–12.0 dB | 6.0 |
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
    /// Last computed gain reduction in dB (always non-positive).
    last_gain_reduction_db: f32,
}

impl Compressor {
    /// Create a new compressor with default settings.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope_follower: EnvelopeFollower::new(sample_rate),
            gain_computer: GainComputer::new(),
            makeup_gain: SmoothedParam::standard(1.0, sample_rate),
            sample_rate,
            last_gain_reduction_db: 0.0,
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
        self.envelope_follower
            .set_attack_ms(attack_ms.clamp(0.1, 100.0));
    }

    /// Set release time in milliseconds.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.envelope_follower
            .set_release_ms(release_ms.clamp(10.0, 1000.0));
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

    /// Returns the last computed gain reduction in dB (always non-positive).
    ///
    /// A value of 0.0 means no compression is occurring. A value of -6.0
    /// means the signal is being reduced by 6 dB.
    pub fn gain_reduction_db(&self) -> f32 {
        self.last_gain_reduction_db
    }
}

impl Effect for Compressor {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let envelope = self.envelope_follower.process(input);
        let envelope_db = fast_linear_to_db(envelope);
        let gain_reduction_db = self.gain_computer.compute_gain_db(envelope_db);
        self.last_gain_reduction_db = gain_reduction_db;
        let gain_linear = fast_db_to_linear(gain_reduction_db);
        let makeup = self.makeup_gain.advance();

        input * gain_linear * makeup
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Linked stereo: detect envelope from both channels (sum), apply same gain
        // This prevents image shifting that would occur with independent compression
        let sum = (left + right) * 0.5;
        let envelope = self.envelope_follower.process(sum);
        let envelope_db = fast_linear_to_db(envelope);
        let gain_reduction_db = self.gain_computer.compute_gain_db(envelope_db);
        self.last_gain_reduction_db = gain_reduction_db;
        let gain_linear = fast_db_to_linear(gain_reduction_db);
        let makeup = self.makeup_gain.advance();

        let gain = gain_linear * makeup;
        (left * gain, right * gain)
    }

    /// Process a block of stereo samples with linked stereo detection.
    ///
    /// Optimized block override that eliminates vtable dispatch overhead and
    /// enables compiler inlining of the full processing chain. Produces
    /// bit-identical output to calling [`process_stereo`](Effect::process_stereo)
    /// per sample.
    ///
    /// The envelope is derived from the mid signal `(L + R) / 2` so that
    /// both channels receive identical gain reduction, preventing stereo
    /// image shift.
    fn process_block_stereo(
        &mut self,
        left_in: &[f32],
        right_in: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        debug_assert_eq!(left_in.len(), right_in.len());
        debug_assert_eq!(left_in.len(), left_out.len());
        debug_assert_eq!(left_out.len(), right_out.len());

        // Hoist gain computer constants out of the loop
        let threshold_db = self.gain_computer.threshold_db;
        let ratio = self.gain_computer.ratio;
        let knee_db = self.gain_computer.knee_db;
        let half_knee = knee_db / 2.0;
        let inv_ratio_complement = 1.0 - 1.0 / ratio;

        for i in 0..left_in.len() {
            let left = left_in[i];
            let right = right_in[i];

            // Linked stereo detection: envelope from mid signal
            let sum = (left + right) * 0.5;
            let envelope = self.envelope_follower.process(sum);

            // Gain computer (inlined from GainComputer::compute_gain_db)
            let envelope_db = fast_linear_to_db(envelope);
            let overshoot = envelope_db - threshold_db;

            let gain_reduction_db = if overshoot <= -half_knee {
                0.0
            } else if overshoot > half_knee {
                -(overshoot * inv_ratio_complement)
            } else {
                let knee_factor = (overshoot + half_knee) / knee_db;
                -(knee_factor * knee_factor * overshoot * inv_ratio_complement)
            };

            self.last_gain_reduction_db = gain_reduction_db;
            let gain_linear = fast_db_to_linear(gain_reduction_db);
            let makeup = self.makeup_gain.advance();

            let gain = gain_linear * makeup;
            left_out[i] = left * gain;
            right_out[i] = right * gain;
        }
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

impl ParameterInfo for Compressor {
    fn param_count(&self) -> usize {
        6
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(
                ParamDescriptor::gain_db("Threshold", "Thresh", -60.0, 0.0, -18.0)
                    .with_id(ParamId(300), "comp_thresh"),
            ),
            1 => Some(
                ParamDescriptor {
                    name: "Ratio",
                    short_name: "Ratio",
                    unit: ParamUnit::Ratio,
                    min: 1.0,
                    max: 20.0,
                    default: 4.0,
                    step: 0.1,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(301), "comp_ratio"),
            ),
            2 => Some(
                ParamDescriptor {
                    name: "Attack",
                    short_name: "Attack",
                    unit: ParamUnit::Milliseconds,
                    min: 0.1,
                    max: 100.0,
                    default: 10.0,
                    step: 0.1,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(302), "comp_attack"),
            ),
            3 => Some(
                ParamDescriptor::time_ms("Release", "Release", 10.0, 1000.0, 100.0)
                    .with_id(ParamId(303), "comp_release"),
            ),
            4 => Some(
                ParamDescriptor::gain_db("Makeup Gain", "Makeup", 0.0, 24.0, 0.0)
                    .with_id(ParamId(304), "comp_makeup"),
            ),
            5 => Some(
                ParamDescriptor::gain_db("Knee", "Knee", 0.0, 12.0, 6.0)
                    .with_id(ParamId(305), "comp_knee"),
            ),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.gain_computer.threshold_db,
            1 => self.gain_computer.ratio,
            2 => self.envelope_follower.attack_ms(),
            3 => self.envelope_follower.release_ms(),
            4 => linear_to_db(self.makeup_gain.target()),
            5 => self.gain_computer.knee_db,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_threshold_db(value),
            1 => self.set_ratio(value),
            2 => self.set_attack_ms(value),
            3 => self.set_release_ms(value),
            4 => self.set_makeup_gain_db(value),
            5 => self.set_knee_db(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::{vec, vec::Vec};

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
    fn test_compressor_block_stereo_matches_per_sample() {
        let sample_rate = 48000.0;

        // Build two identical compressors
        let mut comp_ref = Compressor::new(sample_rate);
        comp_ref.set_threshold_db(-12.0);
        comp_ref.set_ratio(6.0);
        comp_ref.set_attack_ms(5.0);
        comp_ref.set_release_ms(80.0);
        comp_ref.set_makeup_gain_db(3.0);
        comp_ref.set_knee_db(4.0);

        let mut comp_block = comp_ref.clone();

        // Generate test signal: stereo sine-ish sweep with asymmetric levels
        let n = 512;
        let left_in: Vec<f32> = (0..n).map(|i| libm::sinf(i as f32 * 0.05) * 0.8).collect();
        let right_in: Vec<f32> = (0..n).map(|i| libm::cosf(i as f32 * 0.07) * 0.6).collect();

        // Per-sample reference
        let mut left_ref = vec![0.0f32; n];
        let mut right_ref = vec![0.0f32; n];
        for i in 0..n {
            let (l, r) = comp_ref.process_stereo(left_in[i], right_in[i]);
            left_ref[i] = l;
            right_ref[i] = r;
        }

        // Block processing
        let mut left_out = vec![0.0f32; n];
        let mut right_out = vec![0.0f32; n];
        comp_block.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

        // Must be bit-identical
        for i in 0..n {
            assert_eq!(
                left_out[i].to_bits(),
                left_ref[i].to_bits(),
                "Left mismatch at sample {i}: block={} ref={}",
                left_out[i],
                left_ref[i],
            );
            assert_eq!(
                right_out[i].to_bits(),
                right_ref[i].to_bits(),
                "Right mismatch at sample {i}: block={} ref={}",
                right_out[i],
                right_ref[i],
            );
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

        assert!(
            output.abs() < 0.5,
            "Output should be compressed, got {}",
            output
        );
    }
}
