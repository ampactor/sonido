//! Biquad-based filter effects.

use sonido_core::{
    Biquad, Effect, ParamDescriptor, ParamId, ParamScale, ParamUnit, SmoothedParam, gain,
    lowpass_coefficients, math::soft_limit,
};

/// Low-pass filter effect with smoothed parameter control.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Cutoff | 20.0–20000.0 Hz | 1000.0 |
/// | 1 | Resonance | 0.1–20.0 | 0.707 |
/// | 2 | Output | −20.0–20.0 dB | 0.0 |
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
    biquad_r: Biquad,
    cutoff: SmoothedParam,
    q: SmoothedParam,
    output_level: SmoothedParam,
    sample_rate: f32,
    needs_update: bool,
}

impl LowPassFilter {
    /// Create a new low-pass filter.
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = Self {
            biquad: Biquad::new(),
            biquad_r: Biquad::new(),
            cutoff: SmoothedParam::slow(1000.0, sample_rate),
            q: SmoothedParam::slow(0.707, sample_rate),
            output_level: gain::output_level_param(sample_rate),
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
        self.biquad_r.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.needs_update = false;
    }
}

impl Default for LowPassFilter {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for LowPassFilter {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        self.cutoff.advance();
        self.q.advance();
        let output_gain = self.output_level.advance();

        if self.needs_update || !self.cutoff.is_settled() || !self.q.is_settled() {
            self.update_coefficients();
        }

        soft_limit(self.biquad.process(input), 1.0) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.cutoff.advance();
        self.q.advance();
        let output_gain = self.output_level.advance();

        if self.needs_update || !self.cutoff.is_settled() || !self.q.is_settled() {
            self.update_coefficients();
        }

        let out_l = soft_limit(self.biquad.process(left), 1.0) * output_gain;
        let out_r = soft_limit(self.biquad_r.process(right), 1.0) * output_gain;

        (out_l, out_r)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.cutoff.set_sample_rate(sample_rate);
        self.q.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.needs_update = true;
        self.update_coefficients();
    }

    fn reset(&mut self) {
        self.biquad.clear();
        self.biquad_r.clear();
        self.cutoff.snap_to_target();
        self.q.snap_to_target();
        self.output_level.snap_to_target();
        self.needs_update = true;
        self.update_coefficients();
    }
}

sonido_core::impl_params! {
    LowPassFilter, this {
        [0] ParamDescriptor {
                name: "Cutoff",
                short_name: "Cutoff",
                unit: ParamUnit::Hertz,
                min: 20.0,
                max: 20000.0,
                default: 1000.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1200), "flt_cutoff")
            .with_scale(ParamScale::Logarithmic),
            get: this.cutoff.target(),
            set: |v| this.set_cutoff_hz(v);

        [1] ParamDescriptor {
                name: "Resonance",
                short_name: "Reso",
                unit: ParamUnit::Ratio,
                min: 0.1,
                max: 20.0,
                default: 0.707,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1201), "flt_resonance"),
            get: this.q.target(),
            set: |v| this.set_q(v);

        [2] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1202), "flt_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
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

        assert!(
            (output - 1.0).abs() < 0.05,
            "DC should pass, got {}",
            output
        );
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
            let input = libm::sinf(2.0 * core::f32::consts::PI * 10000.0 * t);
            let output = filter.process(input);
            sum += output.abs();
        }

        let avg = sum / 1000.0;
        assert!(
            avg < 0.1,
            "High frequencies should be attenuated, avg {}",
            avg
        );
    }
}
