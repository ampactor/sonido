//! Biquad-based filter effects.

use sonido_core::{Effect, SmoothedParam, Biquad, lowpass_coefficients, ParameterInfo, ParamDescriptor, ParamUnit};

/// Low-pass filter effect with smoothed parameter control.
///
/// # Example
///
/// ```rust
/// use sonido_effects::LowPassFilter;
/// use sonido_core::Effect;
///
/// let mut filter = LowPassFilter::new(44100.0);
/// filter.set_cutoff_hz(1000.0);
/// filter.set_q(0.707);
///
/// let input = 0.5;
/// let output = filter.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct LowPassFilter {
    biquad: Biquad,
    cutoff: SmoothedParam,
    q: SmoothedParam,
    sample_rate: f32,
    needs_update: bool,
}

impl LowPassFilter {
    /// Create a new low-pass filter.
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = Self {
            biquad: Biquad::new(),
            cutoff: SmoothedParam::with_config(1000.0, sample_rate, 20.0),
            q: SmoothedParam::with_config(0.707, sample_rate, 20.0),
            sample_rate,
            needs_update: true,
        };
        filter.update_coefficients();
        filter
    }

    /// Set cutoff frequency in Hz.
    pub fn set_cutoff_hz(&mut self, cutoff: f32) {
        let clamped = cutoff.clamp(20.0, self.sample_rate * 0.49);
        self.cutoff.set_target(clamped);
        self.needs_update = true;
    }

    /// Set Q factor (resonance).
    pub fn set_q(&mut self, q: f32) {
        let clamped = q.clamp(0.1, 20.0);
        self.q.set_target(clamped);
        self.needs_update = true;
    }

    fn update_coefficients(&mut self) {
        let cutoff = self.cutoff.get();
        let q = self.q.get();

        let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(cutoff, q, self.sample_rate);
        self.biquad.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.needs_update = false;
    }
}

impl Effect for LowPassFilter {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        self.cutoff.advance();
        self.q.advance();

        if self.needs_update || !self.cutoff.is_settled() || !self.q.is_settled() {
            self.update_coefficients();
        }

        self.biquad.process(input)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.cutoff.set_sample_rate(sample_rate);
        self.q.set_sample_rate(sample_rate);
        self.needs_update = true;
        self.update_coefficients();
    }

    fn reset(&mut self) {
        self.biquad.clear();
        self.cutoff.snap_to_target();
        self.q.snap_to_target();
        self.needs_update = true;
        self.update_coefficients();
    }
}

impl ParameterInfo for LowPassFilter {
    fn param_count(&self) -> usize {
        2
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Cutoff",
                short_name: "Cutoff",
                unit: ParamUnit::Hertz,
                min: 20.0,
                max: 20000.0,
                default: 1000.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Resonance",
                short_name: "Reso",
                unit: ParamUnit::Ratio,
                min: 0.1,
                max: 20.0,
                default: 0.707,
                step: 0.01,
            }),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.cutoff.target(),
            1 => self.q.target(),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_cutoff_hz(value),
            1 => self.set_q(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowpass_dc_pass() {
        let mut filter = LowPassFilter::new(44100.0);
        filter.set_cutoff_hz(1000.0);
        filter.reset();

        let mut output = 0.0;
        for _ in 0..1000 {
            output = filter.process(1.0);
        }

        assert!((output - 1.0).abs() < 0.05, "DC should pass, got {}", output);
    }

    #[test]
    fn test_lowpass_attenuates_high_freq() {
        let mut filter = LowPassFilter::new(44100.0);
        filter.set_cutoff_hz(100.0);
        filter.reset();

        // High frequency signal
        let mut sum = 0.0;
        for i in 0..1000 {
            let t = i as f32 / 44100.0;
            let input = (2.0 * core::f32::consts::PI * 10000.0 * t).sin();
            let output = filter.process(input);
            sum += output.abs();
        }

        let avg = sum / 1000.0;
        assert!(avg < 0.1, "High frequencies should be attenuated, avg {}", avg);
    }
}
