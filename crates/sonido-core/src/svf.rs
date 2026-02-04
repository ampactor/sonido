//! State Variable Filter implementation
//!
//! A versatile filter that can output lowpass, highpass, bandpass, and notch
//! simultaneously. Well-suited for modulation due to stability at high frequencies.

use core::f32::consts::PI;
use libm::tanf;

use crate::Effect;
use crate::flush_denormal;

/// State Variable Filter output type
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SvfOutput {
    #[default]
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
}

/// State Variable Filter
///
/// Based on the Chamberlin SVF topology with improved numerical stability.
/// Can output lowpass, highpass, bandpass, and notch simultaneously.
///
/// # Example
///
/// ```rust
/// use sonido_core::{StateVariableFilter, SvfOutput, Effect};
///
/// let mut svf = StateVariableFilter::new(48000.0);
/// svf.set_cutoff(1000.0);
/// svf.set_resonance(2.0);
/// svf.set_output_type(SvfOutput::Lowpass);
///
/// let output = svf.process(0.5);
/// ```
#[derive(Debug, Clone)]
pub struct StateVariableFilter {
    // Filter state
    ic1eq: f32,
    ic2eq: f32,

    // Coefficients
    g: f32,
    k: f32,

    // Parameters
    sample_rate: f32,
    cutoff: f32,
    resonance: f32,
    output_type: SvfOutput,
}

impl Default for StateVariableFilter {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl StateVariableFilter {
    /// Create a new SVF with given sample rate
    pub fn new(sample_rate: f32) -> Self {
        let mut svf = Self {
            ic1eq: 0.0,
            ic2eq: 0.0,
            g: 0.0,
            k: 0.0,
            sample_rate,
            cutoff: 1000.0,
            resonance: 0.707,
            output_type: SvfOutput::Lowpass,
        };
        svf.update_coefficients();
        svf
    }

    /// Set cutoff frequency in Hz
    pub fn set_cutoff(&mut self, freq: f32) {
        self.cutoff = freq.clamp(20.0, self.sample_rate * 0.49);
        self.update_coefficients();
    }

    /// Get current cutoff frequency
    pub fn cutoff(&self) -> f32 {
        self.cutoff
    }

    /// Set resonance (Q factor, 0.5 to ~20)
    pub fn set_resonance(&mut self, q: f32) {
        self.resonance = q.clamp(0.5, 20.0);
        self.update_coefficients();
    }

    /// Get current resonance
    pub fn resonance(&self) -> f32 {
        self.resonance
    }

    /// Set output type
    pub fn set_output_type(&mut self, output_type: SvfOutput) {
        self.output_type = output_type;
    }

    /// Get current output type
    pub fn output_type(&self) -> SvfOutput {
        self.output_type
    }

    fn update_coefficients(&mut self) {
        self.g = tanf(PI * self.cutoff / self.sample_rate);
        self.k = 1.0 / self.resonance;
    }

    /// Process and return all outputs (lp, hp, bp, notch)
    ///
    /// Useful when you need multiple filter outputs simultaneously.
    pub fn process_all(&mut self, input: f32) -> (f32, f32, f32, f32) {
        let v3 = input - self.ic2eq;
        let v1 = (self.g * v3 + self.ic1eq) / (1.0 + self.g * (self.g + self.k));
        let v2 = self.ic2eq + self.g * v1;

        self.ic1eq = flush_denormal(2.0 * v1 - self.ic1eq);
        self.ic2eq = flush_denormal(2.0 * v2 - self.ic2eq);

        let lp = v2;
        let bp = v1;
        let hp = input - self.k * v1 - v2;
        let notch = lp + hp;

        (lp, hp, bp, notch)
    }
}

impl Effect for StateVariableFilter {
    fn process(&mut self, input: f32) -> f32 {
        let (lp, hp, bp, notch) = self.process_all(input);

        match self.output_type {
            SvfOutput::Lowpass => lp,
            SvfOutput::Highpass => hp,
            SvfOutput::Bandpass => bp,
            SvfOutput::Notch => notch,
        }
    }

    fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.update_coefficients();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_svf_lowpass_dc() {
        let mut svf = StateVariableFilter::new(48000.0);
        svf.set_cutoff(1000.0);
        svf.set_output_type(SvfOutput::Lowpass);

        // DC should pass through lowpass
        let mut output = 0.0;
        for _ in 0..1000 {
            output = svf.process(1.0);
        }
        assert!((output - 1.0).abs() < 0.05, "DC should pass, got {}", output);
    }

    #[test]
    fn test_svf_highpass_blocks_dc() {
        let mut svf = StateVariableFilter::new(48000.0);
        svf.set_cutoff(1000.0);
        svf.set_output_type(SvfOutput::Highpass);

        // DC should be blocked by highpass
        let mut output = 0.0;
        for _ in 0..1000 {
            output = svf.process(1.0);
        }
        assert!(output.abs() < 0.1, "DC should be blocked, got {}", output);
    }

    #[test]
    fn test_svf_process_all() {
        let mut svf = StateVariableFilter::new(48000.0);
        svf.set_cutoff(1000.0);

        let (lp, hp, bp, notch) = svf.process_all(1.0);

        // All outputs should be finite
        assert!(lp.is_finite());
        assert!(hp.is_finite());
        assert!(bp.is_finite());
        assert!(notch.is_finite());
    }

    #[test]
    fn test_svf_reset() {
        let mut svf = StateVariableFilter::new(48000.0);

        // Process some samples
        for _ in 0..100 {
            svf.process(1.0);
        }

        // Reset
        svf.reset();

        assert_eq!(svf.ic1eq, 0.0);
        assert_eq!(svf.ic2eq, 0.0);
    }
}
