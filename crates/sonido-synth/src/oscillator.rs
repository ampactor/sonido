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
    /// Previous actual (modulated) phase for PM discontinuity detection
    prev_phase_actual: f32,
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
            prev_phase_actual: 0.0,
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
        self.prev_phase_actual = 0.0;
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
        self.prev_phase_actual = self.phase;
        self.advance_phase();
        output
    }

    /// Generate next sample with phase modulation (for FM synthesis).
    ///
    /// Tracks the actual modulated phase for PolyBLEP discontinuity detection,
    /// so anti-aliasing corrections are applied at the true discontinuity
    /// positions rather than the nominal accumulator positions.
    ///
    /// # Arguments
    /// * `phase_mod` - Phase modulation amount in radians
    #[inline]
    pub fn advance_with_pm(&mut self, phase_mod: f32) -> f32 {
        // Convert radians to normalized phase
        let mod_phase = phase_mod / (2.0 * PI);
        let modulated_phase = rem_euclid_f32(self.phase + mod_phase, 1.0);

        // Compute effective phase increment from actual modulated phase movement
        let effective_dt = {
            let delta = modulated_phase - self.prev_phase_actual;
            // Handle wrap-around: if delta is very negative, a forward wrap occurred
            if delta < -0.5 {
                delta + 1.0
            } else if delta > 0.5 {
                delta - 1.0
            } else {
                delta
            }
        };
        let effective_dt_abs = libm::fabsf(effective_dt).max(self.phase_inc);

        let output = self.generate_sample_at_phase_with_dt(modulated_phase, effective_dt_abs);
        self.prev_phase_actual = modulated_phase;
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

    /// Generate a sample at a specific phase position using the oscillator's
    /// nominal phase increment for PolyBLEP width.
    #[inline]
    fn generate_sample_at_phase(&mut self, phase: f32) -> f32 {
        self.generate_sample_at_phase_with_dt(phase, self.phase_inc)
    }

    /// Generate a sample at a specific phase position with explicit PolyBLEP width.
    ///
    /// Each waveform uses a different anti-aliasing strategy:
    /// - **Sine**: No aliasing possible (single harmonic), uses `sinf` directly.
    /// - **Saw**: Naive ramp with 4th-order PolyBLEP correction at the phase-wrap
    ///   discontinuity.
    /// - **Square/Pulse**: Naive bipolar signal with PolyBLEP at both rising and
    ///   falling edges.
    /// - **Triangle**: Leaky integration of a PolyBLEP-corrected square wave. This
    ///   produces better results than direct PolyBLEP on a triangle because the
    ///   triangle's discontinuity is in the derivative (slope change), not the
    ///   waveform itself. The leaky integrator coefficient adapts to frequency for
    ///   stable DC blocking across the audible range.
    /// - **Noise**: Xorshift32 PRNG, inherently broadband with no aliasing concern.
    ///
    /// # Arguments
    /// * `phase` - Phase position in [0.0, 1.0)
    /// * `dt` - Phase increment for PolyBLEP window width
    #[inline]
    fn generate_sample_at_phase_with_dt(&mut self, phase: f32, dt: f32) -> f32 {
        match self.waveform {
            OscillatorWaveform::Sine => sinf(phase * 2.0 * PI),

            OscillatorWaveform::Saw => {
                // Naive saw: 2 * phase - 1
                let naive = 2.0 * phase - 1.0;
                // Apply 4th-order PolyBLEP at discontinuity
                naive - poly_blep(phase, dt)
            }

            OscillatorWaveform::Square => self.generate_pulse_with_dt(phase, 0.5, dt),

            OscillatorWaveform::Pulse(duty) => {
                self.generate_pulse_with_dt(phase, duty.clamp(0.01, 0.99), dt)
            }

            OscillatorWaveform::Triangle => {
                // Integrate a square wave for triangle
                // This gives better anti-aliasing than naive triangle
                let square = if phase < 0.5 { 1.0 } else { -1.0 };
                let blep_square =
                    square + poly_blep(phase, dt) - poly_blep(rem_euclid_f32(phase + 0.5, 1.0), dt);

                // Leaky integrator with frequency-adaptive coefficient for DC stability.
                // At low frequencies the coefficient approaches 1.0 (minimal leakage);
                // clamped to 0.9 minimum to prevent runaway at very high frequencies.
                let leak = 1.0 - (self.frequency / self.sample_rate).min(0.1);
                self.prev_output = leak * self.prev_output + blep_square * dt * 4.0;
                self.prev_output
            }

            OscillatorWaveform::Noise => self.generate_noise(),
        }
    }

    #[inline]
    fn generate_pulse_with_dt(&self, phase: f32, duty: f32, dt: f32) -> f32 {
        // Naive pulse
        let naive = if phase < duty { 1.0 } else { -1.0 };

        // PolyBLEP at rising edge (phase = 0)
        let blep1 = poly_blep(phase, dt);
        // PolyBLEP at falling edge (phase = duty)
        let blep2 = poly_blep(rem_euclid_f32(phase - duty + 1.0, 1.0), dt);

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

/// 4th-order PolyBLEP (Polynomial Band-Limited Step) correction.
///
/// Applies a C²-continuous, degree-4 piecewise polynomial correction near
/// waveform discontinuities. The correction window spans 2 samples on each
/// side of the discontinuity (4 samples total), providing roughly 50 dB of
/// alias suppression — a significant improvement over the 2nd-order (~30 dB)
/// version with its 1-sample window.
///
/// The polynomial is derived by fitting a degree-4 curve to the ideal BLEP
/// (Band-Limited stEP) residual with C² continuity at the piece boundary
/// (n=1) and smooth exit at the window boundary (n=2). The second piece
/// uses a monomial `a·(2-n)⁴` which naturally provides C∞ at n=2.
///
/// Reference: Välimäki et al., "Antialiasing Oscillators", IEEE Signal
/// Processing Magazine, 2010.
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
    // Coefficients derived from C²-continuous fit to ideal BLEP residual:
    //   p₁(n) = A₄·n⁴ + A₃·n³ + A₂·n² + A₁·n + A₀  for n ∈ [0,1)
    //   p₂(n) = C·(2-n)⁴                                for n ∈ [1,2)
    // with p₁(0) = -1, p₁'(0) = 0, p₁''(0) = 1, C⁰/C¹/C² at n=1, p₂(2) = 0.
    const A4: f32 = -43.0 / 48.0; // ≈ -0.8958
    const A3: f32 = 7.0 / 6.0; //  ≈  1.1667
    const A2: f32 = 0.5;
    const A0: f32 = -1.0;
    const C: f32 = -11.0 / 48.0; // ≈ -0.2292

    let dt2 = 2.0 * dt;
    if t < dt2 {
        // Forward: within 2 samples past discontinuity
        let n = t / dt;
        if n < 1.0 {
            let n2 = n * n;
            A4 * n2 * n2 + A3 * n2 * n + A2 * n2 + A0
        } else {
            let u = 2.0 - n;
            let u2 = u * u;
            C * u2 * u2
        }
    } else if t > 1.0 - dt2 {
        // Backward: within 2 samples before discontinuity (antisymmetric mirror)
        let n = (1.0 - t) / dt;
        if n < 1.0 {
            let n2 = n * n;
            -(A4 * n2 * n2 + A3 * n2 * n + A2 * n2 + A0)
        } else {
            let u = 2.0 - n;
            let u2 = u * u;
            -(C * u2 * u2)
        }
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
                (-2.0..=2.0).contains(&sample),
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
                (-2.0..=2.0).contains(&sample),
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

    /// Compute DFT magnitude in dB at a specific frequency bin.
    ///
    /// Uses Goertzel's algorithm — O(N) for a single bin, no full FFT needed.
    fn goertzel_magnitude_db(signal: &[f32], bin: usize, n: usize) -> f32 {
        let coeff = 2.0 * libm::cosf(2.0 * PI * bin as f32 / n as f32);
        let mut s0: f64 = 0.0;
        let mut s1: f64 = 0.0;
        let mut s2: f64;
        for &x in signal.iter().take(n) {
            s2 = s1;
            s1 = s0;
            s0 = f64::from(x) + f64::from(coeff) * s1 - s2;
        }
        let real = s0 - s1 * f64::from(libm::cosf(2.0 * PI * bin as f32 / n as f32));
        let imag = s1 * f64::from(libm::sinf(2.0 * PI * bin as f32 / n as f32));
        let magnitude = libm::sqrt(real * real + imag * imag) / (n as f64 / 2.0);
        20.0 * libm::log10(magnitude.max(1e-12)) as f32
    }

    /// Verify 4th-order PolyBLEP alias suppression on a 5 kHz sawtooth at 48 kHz.
    ///
    /// A 5 kHz saw has harmonics at 10, 15, 20 kHz (all below Nyquist=24 kHz).
    /// The first alias folds from 48-5=43 kHz down to 43-24=19 kHz (close to
    /// the 4th harmonic at 20 kHz). We measure the fundamental level and check
    /// that alias energy at non-harmonic frequencies is suppressed below -45 dB.
    #[test]
    fn test_saw_alias_suppression_5khz() {
        let sr = 48000.0;
        let freq = 5000.0;
        let n = 48000; // 1 second = exact integer cycles of 5kHz at 48kHz

        let mut osc = Oscillator::new(sr);
        osc.set_frequency(freq);
        osc.set_waveform(OscillatorWaveform::Saw);

        extern crate alloc;
        use alloc::vec::Vec;
        let samples: Vec<f32> = (0..n).map(|_| osc.advance()).collect();

        // Fundamental at 5kHz = bin 5000
        let fundamental_db = goertzel_magnitude_db(&samples, 5000, n);

        // Check alias frequencies: these are non-harmonic frequencies where
        // aliased energy folds. For a 5kHz saw at 48kHz:
        // - Harmonic 10 (50kHz) folds to 48-50+48 = 46kHz -> folds to 48-46=2kHz? No.
        //   Actually: 50kHz aliases to 50-48=2kHz. Check bin 2000.
        // - Harmonic 11 (55kHz) aliases to 55-48=7kHz. Check bin 7000.
        // - Harmonic 12 (60kHz) aliases to 60-48=12kHz. Check bin 12000.
        // These alias bins should NOT coincide with real harmonics.
        let alias_bins = [2000_usize, 7000, 12000];

        for &bin in &alias_bins {
            let alias_db = goertzel_magnitude_db(&samples, bin, n);
            let suppression = fundamental_db - alias_db;
            assert!(
                suppression > 45.0,
                "Alias at bin {} is only {:.1} dB below fundamental (need >45 dB). \
                 fundamental={:.1} dB, alias={:.1} dB",
                bin,
                suppression,
                fundamental_db,
                alias_db,
            );
        }
    }
}
