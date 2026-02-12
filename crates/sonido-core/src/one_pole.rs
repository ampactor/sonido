//! One-pole lowpass filter for tone controls and HF rolloff.
//!
//! A single-pole IIR lowpass with the difference equation:
//!
//! ```text
//! y[n] = x[n] + coeff * (y[n-1] - x[n])
//!      = (1 - coeff) * x[n] + coeff * y[n-1]
//! ```
//!
//! where `coeff = exp(-2π * freq / sample_rate)`.
//!
//! This is the simplest possible lowpass — 6 dB/octave rolloff, zero latency,
//! one multiply per sample. Used for tone controls, DC blocking feedback paths,
//! and high-frequency damping in delay/reverb algorithms.
//!
//! # Usage
//!
//! ```rust
//! use sonido_core::OnePole;
//!
//! let mut lp = OnePole::new(48000.0, 4000.0);
//! let filtered = lp.process(1.0);
//! assert!(filtered < 1.0); // attenuated above cutoff
//! ```
//!
//! # Reference
//!
//! Julius O. Smith III, "Introduction to Digital Filters with Audio Applications",
//! Section: One-Pole Filter.

use crate::flush_denormal;
use libm::expf;

/// One-pole (6 dB/oct) lowpass filter.
///
/// # Parameters
///
/// - `freq`: Cutoff frequency in Hz (−3 dB point)
/// - `sample_rate`: Sample rate in Hz
///
/// # Invariants
///
/// - `coeff` is always in [0, 1) for stable operation
/// - `state` is flushed to zero when below 1e-20 (denormal protection)
#[derive(Debug, Clone)]
pub struct OnePole {
    state: f32,
    coeff: f32,
    sample_rate: f32,
    freq: f32,
}

impl OnePole {
    /// Create a new one-pole lowpass filter.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `freq_hz` - Cutoff frequency in Hz (20.0 to sample_rate/2)
    pub fn new(sample_rate: f32, freq_hz: f32) -> Self {
        let mut filter = Self {
            state: 0.0,
            coeff: 0.0,
            sample_rate,
            freq: freq_hz,
        };
        filter.recalculate_coeff();
        filter
    }

    /// Set the cutoff frequency and recalculate the coefficient.
    ///
    /// Range: 20.0 to `sample_rate / 2` Hz.
    pub fn set_frequency(&mut self, freq_hz: f32) {
        self.freq = freq_hz;
        self.recalculate_coeff();
    }

    /// Process one sample through the lowpass filter.
    ///
    /// Returns the filtered output.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        // y[n] = x[n] + coeff * (y[n-1] - x[n])
        self.state = flush_denormal(input + self.coeff * (self.state - input));
        self.state
    }

    /// Reset filter state to zero.
    pub fn reset(&mut self) {
        self.state = 0.0;
    }

    /// Update sample rate and recalculate the coefficient.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.recalculate_coeff();
    }

    /// Recalculate the one-pole coefficient from frequency and sample rate.
    ///
    /// `coeff = exp(-2π * freq / sample_rate)`. Higher freq → lower coeff →
    /// less filtering. At freq = 0, coeff ≈ 1 (full filter). At Nyquist,
    /// coeff ≈ 0 (no filter).
    fn recalculate_coeff(&mut self) {
        self.coeff = expf(-core::f32::consts::TAU * self.freq / self.sample_rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_dc() {
        let mut lp = OnePole::new(48000.0, 1000.0);
        // Run DC signal until settled
        let mut out = 0.0;
        for _ in 0..48000 {
            out = lp.process(1.0);
        }
        assert!(
            (out - 1.0).abs() < 1e-4,
            "DC should pass through, got {out}"
        );
    }

    #[test]
    fn attenuates_high_freq() {
        let mut lp = OnePole::new(48000.0, 100.0); // very low cutoff
        // Feed a high-frequency signal (alternating +1/-1 = Nyquist)
        let mut sum = 0.0f32;
        for i in 0..4800 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            sum += lp.process(input).abs();
        }
        let avg = sum / 4800.0;
        assert!(
            avg < 0.05,
            "Nyquist signal should be heavily attenuated, avg = {avg}"
        );
    }

    #[test]
    fn reset_clears_state() {
        let mut lp = OnePole::new(48000.0, 1000.0);
        lp.process(1.0);
        lp.process(1.0);
        lp.reset();
        // After reset, first sample should start from zero
        let out = lp.process(0.0);
        assert_eq!(out, 0.0);
    }
}
