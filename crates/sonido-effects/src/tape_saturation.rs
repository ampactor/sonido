//! Tape saturation effect
//!
//! J37/Kramer MPX inspired tape warmth with soft saturation,
//! even harmonic enhancement, and high frequency rolloff.

use sonido_core::Effect;
use libm::expf;
use core::f32::consts::PI;

/// Tape saturation effect
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
    /// Input gain (drive)
    drive: f32,
    /// Output level
    output_gain: f32,
    /// Saturation amount (0.0 - 1.0)
    saturation: f32,
    /// High frequency rolloff state (one-pole)
    hf_state: f32,
    hf_coeff: f32,
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
        let mut ts = Self {
            sample_rate,
            drive: 1.0,
            output_gain: 1.0,
            saturation: 0.5,
            hf_state: 0.0,
            hf_coeff: 0.0,
            bias: 0.0,
        };
        ts.set_hf_rolloff(12000.0);
        ts
    }

    /// Set input drive (1.0 = unity, higher = more saturation)
    pub fn set_drive(&mut self, drive: f32) {
        self.drive = drive.clamp(0.1, 10.0);
    }

    /// Get current drive
    pub fn drive(&self) -> f32 {
        self.drive
    }

    /// Set output gain
    pub fn set_output(&mut self, gain: f32) {
        self.output_gain = gain.clamp(0.0, 2.0);
    }

    /// Get current output gain
    pub fn output(&self) -> f32 {
        self.output_gain
    }

    /// Set saturation amount (0.0 - 1.0)
    pub fn set_saturation(&mut self, sat: f32) {
        self.saturation = sat.clamp(0.0, 1.0);
    }

    /// Get current saturation
    pub fn saturation(&self) -> f32 {
        self.saturation
    }

    /// Set high frequency rolloff point in Hz
    pub fn set_hf_rolloff(&mut self, freq: f32) {
        let freq = freq.clamp(1000.0, 20000.0);
        self.hf_coeff = expf(-2.0 * PI * freq / self.sample_rate);
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
    fn saturate(&self, x: f32) -> f32 {
        let driven = x * self.drive + self.bias;

        // Soft saturation with asymmetry for even harmonics
        let positive = if driven > 0.0 {
            1.0 - expf(-driven * 2.0)
        } else {
            -1.0 + expf(driven * 1.8) // Slight asymmetry
        };

        // Blend clean and saturated based on saturation amount
        let clean = driven.clamp(-1.0, 1.0);
        clean * (1.0 - self.saturation) + positive * self.saturation
    }
}

impl Effect for TapeSaturation {
    fn process(&mut self, input: f32) -> f32 {
        // Apply saturation
        let saturated = self.saturate(input);

        // High frequency rolloff (one-pole lowpass)
        self.hf_state = saturated + self.hf_coeff * (self.hf_state - saturated);

        // Output
        self.hf_state * self.output_gain
    }

    fn reset(&mut self) {
        self.hf_state = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        // Preserve HF rolloff frequency
        let old_freq = if self.hf_coeff > 0.0 && self.hf_coeff < 1.0 {
            -self.sample_rate * self.hf_coeff.ln() / (2.0 * PI)
        } else {
            12000.0
        };
        self.sample_rate = sample_rate;
        self.set_hf_rolloff(old_freq);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tape_saturation_basic() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_drive(2.0);

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
        assert!((pos.abs() - neg.abs()).abs() > 0.001, "Should be asymmetric");
    }

    #[test]
    fn test_tape_saturation_reset() {
        let mut tape = TapeSaturation::new(48000.0);

        for _ in 0..100 {
            tape.process(1.0);
        }

        tape.reset();
        assert_eq!(tape.hf_state, 0.0);
    }
}
