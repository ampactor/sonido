//! Audio-rate oscillators with anti-aliasing.
//!
//! Provides band-limited oscillators for synthesis using PolyBLEP
//! (Polynomial Band-Limited Step) to reduce aliasing artifacts.

use core::f32::consts::PI;
use libm::{floorf, sinf};

/// Euclidean remainder for f32, compatible with no_std.
#[inline]
fn rem_euclid_f32(a: f32, b: f32) -> f32 {
    let r = a - b * floorf(a / b);
    if r < 0.0 { r + b } else { r }
}

/// Oscillator waveform types
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum OscillatorWaveform {
    /// Sine waveform — pure fundamental tone.
    #[default]
    Sine,
    /// Triangle waveform — odd harmonics, softer than saw.
    Triangle,
    /// Sawtooth waveform — all harmonics, bright timbre.
    Saw,
    /// Square waveform (50% duty cycle) — odd harmonics, hollow timbre.
    Square,
    /// Pulse with variable duty cycle (0.0 to 1.0)
    Pulse(f32),
    /// White noise
    Noise,
}

/// Audio-rate oscillator with PolyBLEP anti-aliasing.
///
/// Designed for audio synthesis with support for:
/// - Multiple waveforms (sine, triangle, saw, square, pulse, noise)
/// - PolyBLEP anti-aliasing for non-sinusoidal waveforms
/// - Phase modulation for FM synthesis
/// - Hard sync capability
///
/// # Example
///
/// ```rust
/// use sonido_synth::{Oscillator, OscillatorWaveform};
///
/// let mut osc = Oscillator::new(48000.0);
/// osc.set_frequency(440.0); // A4
/// osc.set_waveform(OscillatorWaveform::Saw);
///
/// // Generate samples
/// let sample = osc.advance();
/// ```
#[derive(Debug, Clone)]
pub struct Oscillator {
    /// Current phase position [0.0, 1.0)
    phase: f32,
    /// Phase increment per sample
    phase_inc: f32,
    /// Sample rate in Hz
    sample_rate: f32,
    /// Frequency in Hz
    frequency: f32,
    /// Waveform type
    waveform: OscillatorWaveform,
    /// Noise state for pseudo-random generation
    noise_state: u32,
    /// Previous output for triangle integration
    prev_output: f32,
}

impl Default for Oscillator {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Oscillator {
    /// Create a new oscillator with the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            phase: 0.0,
            phase_inc: 440.0 / sample_rate,
            sample_rate,
            frequency: 440.0,
            waveform: OscillatorWaveform::Sine,
            noise_state: 0x12345678,
            prev_output: 0.0,
        }
    }

    /// Set frequency in Hz.
    pub fn set_frequency(&mut self, freq_hz: f32) {
        self.frequency = freq_hz.max(0.0);
        self.phase_inc = self.frequency / self.sample_rate;
    }

    /// Get current frequency in Hz.
    pub fn frequency(&self) -> f32 {
        self.frequency
    }

    /// Set waveform type.
    pub fn set_waveform(&mut self, waveform: OscillatorWaveform) {
        self.waveform = waveform;
    }

    /// Get current waveform.
    pub fn waveform(&self) -> OscillatorWaveform {
        self.waveform
    }

    /// Set sample rate and recalculate phase increment.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.phase_inc = self.frequency / self.sample_rate;
    }

    /// Get current sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Reset phase to 0.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_output = 0.0;
    }

    /// Hard sync: reset phase to 0 (used for oscillator sync).
    ///
    /// Call this when a master oscillator completes a cycle.
    pub fn sync(&mut self) {
        self.phase = 0.0;
    }

    /// Set phase directly (0.0 to 1.0).
    pub fn set_phase(&mut self, phase: f32) {
        self.phase = phase.clamp(0.0, 1.0);
    }

    /// Get current phase.
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Generate next sample.
    #[inline]
    pub fn advance(&mut self) -> f32 {
        let output = self.generate_sample();
        self.advance_phase();
        output
    }

    /// Generate next sample with phase modulation (for FM synthesis).
    ///
    /// # Arguments
    /// * `phase_mod` - Phase modulation amount in radians
    #[inline]
    pub fn advance_with_pm(&mut self, phase_mod: f32) -> f32 {
        // Convert radians to normalized phase
        let mod_phase = phase_mod / (2.0 * PI);
        let modulated_phase = rem_euclid_f32(self.phase + mod_phase, 1.0);

        let output = self.generate_sample_at_phase(modulated_phase);
        self.advance_phase();
        output
    }

    #[inline]
    fn advance_phase(&mut self) {
        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    #[inline]
    fn generate_sample(&mut self) -> f32 {
        self.generate_sample_at_phase(self.phase)
    }

    /// Generate a sample at a specific phase position.
    ///
    /// Each waveform uses a different anti-aliasing strategy:
    /// - **Sine**: No aliasing possible (single harmonic), uses `sinf` directly.
    /// - **Saw**: Naive ramp with PolyBLEP correction at the phase-wrap discontinuity.
    /// - **Square/Pulse**: Naive bipolar signal with PolyBLEP at both rising and falling edges.
    /// - **Triangle**: Leaky integration of a PolyBLEP-corrected square wave. This
    ///   produces better results than direct PolyBLEP on a triangle because the triangle's
    ///   discontinuity is in the derivative (slope change), not the waveform itself.
    ///   The leaky integrator (coefficient 0.999) provides DC blocking at ~7 Hz.
    /// - **Noise**: Xorshift32 PRNG, inherently broadband with no aliasing concern.
    #[inline]
    fn generate_sample_at_phase(&mut self, phase: f32) -> f32 {
        match self.waveform {
            OscillatorWaveform::Sine => sinf(phase * 2.0 * PI),

            OscillatorWaveform::Saw => {
                // Naive saw: 2 * phase - 1
                let naive = 2.0 * phase - 1.0;
                // Apply PolyBLEP at discontinuity
                naive - poly_blep(phase, self.phase_inc)
            }

            OscillatorWaveform::Square => self.generate_pulse(phase, 0.5),

            OscillatorWaveform::Pulse(duty) => self.generate_pulse(phase, duty.clamp(0.01, 0.99)),

            OscillatorWaveform::Triangle => {
                // Integrate a square wave for triangle
                // This gives better anti-aliasing than naive triangle
                let square = if phase < 0.5 { 1.0 } else { -1.0 };
                let blep_square = square + poly_blep(phase, self.phase_inc)
                    - poly_blep(rem_euclid_f32(phase + 0.5, 1.0), self.phase_inc);

                // Leaky integrator for DC stability
                self.prev_output = 0.999 * self.prev_output + blep_square * self.phase_inc * 4.0;
                self.prev_output
            }

            OscillatorWaveform::Noise => self.generate_noise(),
        }
    }

    #[inline]
    fn generate_pulse(&self, phase: f32, duty: f32) -> f32 {
        // Naive pulse
        let naive = if phase < duty { 1.0 } else { -1.0 };

        // PolyBLEP at rising edge (phase = 0)
        let blep1 = poly_blep(phase, self.phase_inc);
        // PolyBLEP at falling edge (phase = duty)
        let blep2 = poly_blep(rem_euclid_f32(phase - duty + 1.0, 1.0), self.phase_inc);

        naive + blep1 - blep2
    }

    #[inline]
    fn generate_noise(&mut self) -> f32 {
        // Simple xorshift PRNG
        let mut x = self.noise_state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise_state = x;

        // Convert to float in [-1, 1]
        (x as i32 as f32) / (i32::MAX as f32)
    }
}

/// PolyBLEP (Polynomial Band-Limited Step) correction.
///
/// Applies a 2nd-order polynomial correction near waveform discontinuities
/// to suppress aliasing. The correction is non-zero only within one sample
/// of the discontinuity (width = dt = frequency/sample_rate), making it
/// extremely cheap to compute.
///
/// The polynomial approximates the residual between a naive (infinite-bandwidth)
/// step function and an ideal band-limited step. The 2nd-order approximation
/// provides roughly 30 dB of alias suppression relative to naive generation.
///
/// # Arguments
/// * `t` - Current phase position in [0.0, 1.0)
/// * `dt` - Phase increment per sample (frequency / sample_rate)
///
/// # Returns
/// Correction value to subtract from (or add to) the naive waveform.
/// Returns 0.0 when phase is far from a discontinuity.
#[inline]
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        // Just past discontinuity
        let t = t / dt;
        2.0 * t - t * t - 1.0
    } else if t > 1.0 - dt {
        // Just before discontinuity
        let t = (t - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oscillator_frequency_440hz() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(440.0);
        osc.set_waveform(OscillatorWaveform::Sine);

        // Count zero crossings to verify frequency
        let mut zero_crossings: i32 = 0;
        let mut prev = 0.0;
        let samples = 48000; // 1 second

        for _ in 0..samples {
            let sample = osc.advance();
            if prev <= 0.0 && sample > 0.0 {
                zero_crossings += 1;
            }
            prev = sample;
        }

        // Should have ~440 positive zero crossings (one per cycle)
        assert!(
            (zero_crossings - 440).abs() <= 2,
            "Expected ~440 zero crossings, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_oscillator_frequency_1000hz() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(1000.0);
        osc.set_waveform(OscillatorWaveform::Sine);

        let mut zero_crossings: i32 = 0;
        let mut prev = 0.0;

        for _ in 0..48000 {
            let sample = osc.advance();
            if prev <= 0.0 && sample > 0.0 {
                zero_crossings += 1;
            }
            prev = sample;
        }

        assert!(
            (zero_crossings - 1000).abs() <= 2,
            "Expected ~1000 zero crossings, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_oscillator_frequency_10000hz() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(10000.0);
        osc.set_waveform(OscillatorWaveform::Sine);

        let mut zero_crossings: i32 = 0;
        let mut prev = 0.0;

        for _ in 0..48000 {
            let sample = osc.advance();
            if prev <= 0.0 && sample > 0.0 {
                zero_crossings += 1;
            }
            prev = sample;
        }

        assert!(
            (zero_crossings - 10000).abs() <= 5,
            "Expected ~10000 zero crossings, got {}",
            zero_crossings
        );
    }

    #[test]
    fn test_sine_output_range() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_waveform(OscillatorWaveform::Sine);

        for _ in 0..10000 {
            let sample = osc.advance();
            assert!(
                (-1.0..=1.0).contains(&sample),
                "Sine out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_saw_output_range() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_waveform(OscillatorWaveform::Saw);

        for _ in 0..10000 {
            let sample = osc.advance();
            assert!(
                (-1.5..=1.5).contains(&sample),
                "Saw out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_square_output_range() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_waveform(OscillatorWaveform::Square);

        for _ in 0..10000 {
            let sample = osc.advance();
            assert!(
                (-1.5..=1.5).contains(&sample),
                "Square out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_noise_output_range() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_waveform(OscillatorWaveform::Noise);

        for _ in 0..10000 {
            let sample = osc.advance();
            assert!(
                (-1.0..=1.0).contains(&sample),
                "Noise out of range: {}",
                sample
            );
        }
    }

    #[test]
    fn test_hard_sync() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(440.0);

        // Advance some samples
        for _ in 0..100 {
            osc.advance();
        }

        assert!(osc.phase() > 0.0);

        // Sync
        osc.sync();
        assert_eq!(osc.phase(), 0.0);
    }

    #[test]
    fn test_phase_modulation() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(440.0);
        osc.set_waveform(OscillatorWaveform::Sine);

        // With no PM, should be same as normal
        let normal = osc.advance();
        osc.reset();
        let with_zero_pm = osc.advance_with_pm(0.0);

        assert!(
            (normal - with_zero_pm).abs() < 0.001,
            "PM with 0 should match normal"
        );

        // sin(x + pi) = -sin(x), at phase 0: sin(0) = 0, sin(pi) = 0
        // Test at a different phase where the effect is visible
        osc.set_phase(0.25);
        let at_quarter = osc.advance_with_pm(0.0);
        osc.set_phase(0.25);
        let at_quarter_plus_half = osc.advance_with_pm(PI);

        assert!(
            (at_quarter + at_quarter_plus_half).abs() < 0.01,
            "PM with PI should invert: {} vs {}",
            at_quarter,
            at_quarter_plus_half
        );
    }

    #[test]
    fn test_pulse_duty_cycle() {
        let mut osc = Oscillator::new(48000.0);
        osc.set_frequency(100.0);

        // 25% duty cycle
        osc.set_waveform(OscillatorWaveform::Pulse(0.25));

        let mut positive_count = 0;
        let mut total = 0;

        for _ in 0..48000 {
            let sample = osc.advance();
            if sample > 0.0 {
                positive_count += 1;
            }
            total += 1;
        }

        let ratio = positive_count as f32 / total as f32;
        assert!(
            (ratio - 0.25).abs() < 0.05,
            "Expected ~25% positive samples, got {:.1}%",
            ratio * 100.0
        );
    }
}
