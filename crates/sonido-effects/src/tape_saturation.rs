//! Tape saturation effect
//!
//! J37/Kramer MPX inspired tape warmth with soft saturation,
//! even harmonic enhancement, and high frequency rolloff.

use sonido_core::{
    Effect, OnePole, ParamDescriptor, ParamId, ParamScale, ParamUnit, ParameterInfo, SmoothedParam,
    db_to_linear, fast_exp2, linear_to_db,
};

/// Tape saturation effect
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Drive | 0.0–24.0 dB | 6.0 |
/// | 1 | Saturation | 0–100% | 50.0 |
/// | 2 | Output | -12.0–12.0 dB | -6.0 |
/// | 3 | HF Rolloff | 1000.0–20000.0 Hz | 12000.0 |
/// | 4 | Bias | -0.2–0.2 | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::TapeSaturation;
/// use sonido_core::Effect;
///
/// let mut tape = TapeSaturation::new(48000.0);
/// tape.set_drive(2.0);
/// tape.set_saturation(0.6);
/// tape.set_hf_rolloff(10000.0);
///
/// let input = 0.5;
/// let output = tape.process(input);
/// ```
pub struct TapeSaturation {
    sample_rate: f32,
    /// Input gain (drive) with smoothing
    drive: SmoothedParam,
    /// Output level with smoothing
    output_gain: SmoothedParam,
    /// Saturation amount (0.0 - 1.0) with smoothing
    saturation: SmoothedParam,
    /// High frequency rolloff filter (left channel)
    hf_filter: OnePole,
    /// High frequency rolloff filter (right channel)
    hf_filter_r: OnePole,
    /// HF rolloff frequency in Hz (shadow for get_param readback)
    hf_freq: f32,
    /// Bias (affects harmonic content)
    bias: f32,
}

impl Default for TapeSaturation {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl TapeSaturation {
    /// Create new tape saturation
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            drive: SmoothedParam::standard(db_to_linear(6.0), sample_rate),
            output_gain: SmoothedParam::standard(db_to_linear(-6.0), sample_rate),
            saturation: SmoothedParam::standard(0.5, sample_rate),
            hf_filter: OnePole::new(sample_rate, 12000.0),
            hf_filter_r: OnePole::new(sample_rate, 12000.0),
            hf_freq: 12000.0,
            bias: 0.0,
        }
    }

    /// Set input drive (1.0 = unity, higher = more saturation)
    pub fn set_drive(&mut self, drive: f32) {
        self.drive.set_target(drive.clamp(0.1, 10.0));
    }

    /// Get current drive target
    pub fn drive(&self) -> f32 {
        self.drive.target()
    }

    /// Set output gain
    pub fn set_output(&mut self, gain: f32) {
        self.output_gain.set_target(gain.clamp(0.0, 2.0));
    }

    /// Get current output gain target
    pub fn output(&self) -> f32 {
        self.output_gain.target()
    }

    /// Set saturation amount (0.0 - 1.0)
    pub fn set_saturation(&mut self, sat: f32) {
        self.saturation.set_target(sat.clamp(0.0, 1.0));
    }

    /// Get current saturation target
    pub fn saturation(&self) -> f32 {
        self.saturation.target()
    }

    /// Set high frequency rolloff point in Hz (1000-20000).
    pub fn set_hf_rolloff(&mut self, freq: f32) {
        let freq = freq.clamp(1000.0, 20000.0);
        self.hf_freq = freq;
        self.hf_filter.set_frequency(freq);
        self.hf_filter_r.set_frequency(freq);
    }

    /// Set tape bias (affects harmonic character)
    pub fn set_bias(&mut self, bias: f32) {
        self.bias = bias.clamp(-0.2, 0.2);
    }

    /// Get current bias
    pub fn bias(&self) -> f32 {
        self.bias
    }

    /// Tape saturation transfer function
    #[inline]
    fn saturate(&self, x: f32, drive: f32, saturation: f32) -> f32 {
        let driven = x * drive + self.bias;

        // Soft saturation with asymmetry for even harmonics
        // expf(x) = exp2(x * LOG2_E)
        let positive = if driven > 0.0 {
            1.0 - fast_exp2(-driven * 2.0 * core::f32::consts::LOG2_E)
        } else {
            -1.0 + fast_exp2(driven * 1.8 * core::f32::consts::LOG2_E) // Slight asymmetry
        };

        // Blend clean and saturated based on saturation amount
        let clean = driven.clamp(-1.0, 1.0);
        clean * (1.0 - saturation) + positive * saturation
    }
}

impl Effect for TapeSaturation {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let drive = self.drive.advance();
        let output_gain = self.output_gain.advance();
        let saturation = self.saturation.advance();

        // Apply saturation
        let saturated = self.saturate(input, drive, saturation);

        // High frequency rolloff
        self.hf_filter.process(saturated) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.advance();
        let output_gain = self.output_gain.advance();
        let saturation = self.saturation.advance();

        // Process left with its own filter state
        let sat_l = self.saturate(left, drive, saturation);
        let out_l = self.hf_filter.process(sat_l) * output_gain;

        // Process right with separate filter state
        let sat_r = self.saturate(right, drive, saturation);
        let out_r = self.hf_filter_r.process(sat_r) * output_gain;

        (out_l, out_r)
    }

    fn reset(&mut self) {
        self.hf_filter.reset();
        self.hf_filter_r.reset();
        self.drive.snap_to_target();
        self.output_gain.snap_to_target();
        self.saturation.snap_to_target();
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.hf_filter.set_sample_rate(sample_rate);
        self.hf_filter_r.set_sample_rate(sample_rate);
        self.drive.set_sample_rate(sample_rate);
        self.output_gain.set_sample_rate(sample_rate);
        self.saturation.set_sample_rate(sample_rate);
    }
}

impl ParameterInfo for TapeSaturation {
    fn param_count(&self) -> usize {
        5
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(
                ParamDescriptor {
                    name: "Drive",
                    short_name: "Drive",
                    unit: ParamUnit::Decibels,
                    min: 0.0,
                    max: 24.0,
                    default: 6.0,
                    step: 0.5,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1400), "tape_drive"),
            ),
            1 => Some(
                ParamDescriptor {
                    name: "Saturation",
                    short_name: "Sat",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1401), "tape_saturation"),
            ),
            2 => Some(
                ParamDescriptor {
                    name: "Output",
                    short_name: "Output",
                    unit: ParamUnit::Decibels,
                    min: -12.0,
                    max: 12.0,
                    default: -6.0,
                    step: 0.5,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1402), "tape_output"),
            ),
            3 => Some(
                ParamDescriptor {
                    name: "HF Rolloff",
                    short_name: "HFRoll",
                    unit: ParamUnit::Hertz,
                    min: 1000.0,
                    max: 20000.0,
                    default: 12000.0,
                    step: 100.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1403), "tape_hf_rolloff")
                .with_scale(ParamScale::Logarithmic),
            ),
            4 => Some(
                ParamDescriptor {
                    name: "Bias",
                    short_name: "Bias",
                    unit: ParamUnit::None,
                    min: -0.2,
                    max: 0.2,
                    default: 0.0,
                    step: 0.01,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1404), "tape_bias"),
            ),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => linear_to_db(self.drive.target()),
            1 => self.saturation.target() * 100.0,
            2 => linear_to_db(self.output_gain.target()),
            3 => self.hf_freq,
            4 => self.bias,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_drive(db_to_linear(value.clamp(0.0, 24.0))),
            1 => self.set_saturation(value / 100.0),
            2 => self.set_output(db_to_linear(value.clamp(-12.0, 12.0))),
            3 => self.set_hf_rolloff(value.clamp(1000.0, 20000.0)),
            4 => self.set_bias(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tape_saturation_basic() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_drive(2.0);
        tape.reset(); // Snap to target for test

        for _ in 0..100 {
            let output = tape.process(0.5);
            assert!(output.is_finite());
            assert!(output.abs() <= 2.0);
        }
    }

    #[test]
    fn test_tape_saturation_asymmetry() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_saturation(1.0);
        tape.set_drive(3.0);
        tape.reset();

        // Let filter settle
        for _ in 0..100 {
            tape.process(0.0);
        }

        let pos = tape.process(0.5);
        tape.reset();
        for _ in 0..100 {
            tape.process(0.0);
        }
        let neg = tape.process(-0.5);

        // Output should be slightly asymmetric
        assert!(
            (pos.abs() - neg.abs()).abs() > 0.001,
            "Should be asymmetric"
        );
    }

    #[test]
    fn test_tape_saturation_reset() {
        let mut tape = TapeSaturation::new(48000.0);

        for _ in 0..100 {
            tape.process(1.0);
        }

        tape.reset();

        // After reset, processing zero should produce zero (filter state cleared)
        let output = tape.process(0.0);
        assert!(
            output.abs() < 1e-6,
            "After reset, zero input should produce near-zero output, got {output}"
        );
    }

    #[test]
    fn test_tape_saturation_smoothing() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_drive(1.0);
        tape.reset();

        // Set new drive target
        tape.set_drive(5.0);

        // First sample should not be at full drive yet
        let first = tape.process(0.5);

        // Process more samples to let smoothing settle
        for _ in 0..1000 {
            tape.process(0.5);
        }

        let settled = tape.process(0.5);

        // Settled value should be different (more saturated with higher drive)
        assert!(settled != first, "Smoothing should gradually change drive");
    }
}
