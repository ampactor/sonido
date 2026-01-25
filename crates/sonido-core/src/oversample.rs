//! Generic oversampling wrapper for anti-aliased nonlinear processing.
//!
//! Nonlinear effects (distortion, waveshaping, saturation) generate harmonics
//! that can exceed Nyquist and alias back into the audible range. Oversampling
//! mitigates this by:
//!
//! 1. **Upsampling**: Increase sample rate by factor N (interpolation)
//! 2. **Processing**: Run the effect at N× sample rate (harmonics stay below Nyquist)
//! 3. **Downsampling**: Return to original rate (filter + decimation)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use sonido_core::{Effect, Oversampled};
//! use sonido_effects::Distortion;
//!
//! // Create a distortion effect
//! let dist = Distortion::new(48000.0);
//!
//! // Wrap it with 4x oversampling
//! let mut oversampled = Oversampled::<4, _>::new(dist, 48000.0);
//!
//! // Process audio - internally runs at 192kHz
//! let output = oversampled.process(0.5);
//! ```
//!
//! ## Supported Factors
//!
//! - `2`: 2× oversampling (good balance of quality/CPU)
//! - `4`: 4× oversampling (recommended for most distortion)
//! - `8`: 8× oversampling (high quality, more CPU)

use crate::Effect;

/// Maximum supported oversampling factor.
pub const MAX_OVERSAMPLE_FACTOR: usize = 8;

/// FIR filter order for anti-aliasing (taps = ORDER + 1).
const FILTER_ORDER: usize = 15;

/// Number of filter taps.
const FILTER_TAPS: usize = FILTER_ORDER + 1;

/// Oversampling wrapper for any effect.
///
/// Wraps an effect with upsampling and downsampling to reduce aliasing
/// from nonlinear processing. The inner effect runs at `FACTOR` times
/// the base sample rate.
///
/// # Type Parameters
///
/// - `FACTOR`: Oversampling factor (2, 4, or 8)
/// - `E`: The effect type being wrapped
///
/// # Memory Usage
///
/// Uses fixed-size arrays for filter state:
/// - Previous sample for interpolation: 4 bytes
/// - Downsample filter: `FILTER_TAPS` × `f32` = 64 bytes
/// - Upsample buffer: `MAX_OVERSAMPLE_FACTOR` × `f32` = 32 bytes
pub struct Oversampled<const FACTOR: usize, E: Effect> {
    /// The wrapped effect
    effect: E,
    /// Base sample rate (before oversampling)
    sample_rate: f32,
    /// Previous input sample for linear interpolation
    prev_sample: f32,
    /// Downsampling filter state (delay line)
    downsample_state: [f32; FILTER_TAPS],
    /// Buffer for upsampled/processed signal
    work_buffer: [f32; MAX_OVERSAMPLE_FACTOR],
}

impl<const FACTOR: usize, E: Effect> Oversampled<FACTOR, E> {
    /// Create a new oversampled effect wrapper.
    ///
    /// # Arguments
    /// * `effect` - The effect to wrap
    /// * `sample_rate` - Base sample rate in Hz
    ///
    /// # Panics
    /// Panics if `FACTOR` is not 2, 4, or 8.
    pub fn new(mut effect: E, sample_rate: f32) -> Self {
        assert!(
            FACTOR == 2 || FACTOR == 4 || FACTOR == 8,
            "Oversample factor must be 2, 4, or 8"
        );

        // Set the inner effect's sample rate to the oversampled rate
        effect.set_sample_rate(sample_rate * FACTOR as f32);

        Self {
            effect,
            sample_rate,
            prev_sample: 0.0,
            downsample_state: [0.0; FILTER_TAPS],
            work_buffer: [0.0; MAX_OVERSAMPLE_FACTOR],
        }
    }

    /// Get a reference to the inner effect.
    pub fn inner(&self) -> &E {
        &self.effect
    }

    /// Get a mutable reference to the inner effect.
    pub fn inner_mut(&mut self) -> &mut E {
        &mut self.effect
    }

    /// Unwrap and return the inner effect.
    pub fn into_inner(self) -> E {
        self.effect
    }

    /// Get the oversampling factor.
    pub fn factor(&self) -> usize {
        FACTOR
    }

    /// Get filter coefficients for the current factor.
    #[inline]
    fn get_coefficients(&self) -> &'static [f32; FILTER_TAPS] {
        match FACTOR {
            2 => &COEFFS_2X,
            4 => &COEFFS_4X,
            8 => &COEFFS_8X,
            _ => unreachable!(),
        }
    }

    /// Upsample using linear interpolation.
    #[inline]
    fn upsample(&mut self, input: f32) {
        let step = 1.0 / FACTOR as f32;
        for i in 0..FACTOR {
            let t = (i as f32 + 1.0) * step;
            self.work_buffer[i] = self.prev_sample + t * (input - self.prev_sample);
        }
        self.prev_sample = input;
    }

    /// Downsample with anti-aliasing filter.
    #[inline]
    fn downsample(&mut self) -> f32 {
        let coeffs = self.get_coefficients();
        let mut output = 0.0;

        for i in 0..FACTOR {
            // Shift delay line
            for j in (1..FILTER_TAPS).rev() {
                self.downsample_state[j] = self.downsample_state[j - 1];
            }
            self.downsample_state[0] = self.work_buffer[i];

            // Compute filtered output on last sample (decimation point)
            if i == FACTOR - 1 {
                for (j, &coeff) in coeffs.iter().enumerate() {
                    output += self.downsample_state[j] * coeff;
                }
            }
        }

        output
    }
}

impl<const FACTOR: usize, E: Effect> Effect for Oversampled<FACTOR, E> {
    fn process(&mut self, input: f32) -> f32 {
        // Upsample via linear interpolation
        self.upsample(input);

        // Process each upsampled sample through the effect
        for i in 0..FACTOR {
            self.work_buffer[i] = self.effect.process(self.work_buffer[i]);
        }

        // Downsample with anti-aliasing
        self.downsample()
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        debug_assert_eq!(input.len(), output.len());
        for (inp, out) in input.iter().zip(output.iter_mut()) {
            *out = self.process(*inp);
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        // Inner effect runs at oversampled rate
        self.effect.set_sample_rate(sample_rate * FACTOR as f32);
    }

    fn reset(&mut self) {
        self.prev_sample = 0.0;
        self.downsample_state = [0.0; FILTER_TAPS];
        self.work_buffer = [0.0; MAX_OVERSAMPLE_FACTOR];
        self.effect.reset();
    }

    fn latency_samples(&self) -> usize {
        // Filter latency (group delay) + inner effect latency
        // FIR filter group delay = (taps - 1) / 2 for symmetric filter
        let filter_latency = FILTER_ORDER / 2;
        filter_latency + self.effect.latency_samples()
    }
}

// ============================================================================
// Filter Coefficients
// ============================================================================
//
// Lowpass FIR filter coefficients for anti-aliasing during downsampling.
// Generated using windowed-sinc with Kaiser window.

/// 2× oversampling filter coefficients (cutoff = 0.4 × Nyquist)
#[rustfmt::skip]
static COEFFS_2X: [f32; FILTER_TAPS] = [
    -0.00152541,  0.00000000,  0.01309369,  0.00000000,
    -0.05738920,  0.00000000,  0.29581875,  0.50000434,
     0.29581875,  0.00000000, -0.05738920,  0.00000000,
     0.01309369,  0.00000000, -0.00152541,  0.00000000,
];

/// 4× oversampling filter coefficients (cutoff = 0.2 × Nyquist)
#[rustfmt::skip]
static COEFFS_4X: [f32; FILTER_TAPS] = [
    0.0018645282, 0.0068257641, 0.0172712655, 0.0342604001,
    0.0571166576, 0.0830896230, 0.1078345458, 0.1260221675,
    0.1332946246, 0.1260221675, 0.1078345458, 0.0830896230,
    0.0571166576, 0.0342604001, 0.0172712655, 0.0068257641,
];

/// 8× oversampling filter coefficients (cutoff = 0.1 × Nyquist)
#[rustfmt::skip]
static COEFFS_8X: [f32; FILTER_TAPS] = [
    0.0048323092, 0.0131400047, 0.0264623493, 0.0438249658,
    0.0634416395, 0.0828886958, 0.0994801510, 0.1107812341,
    0.1151296104, 0.1107812341, 0.0994801510, 0.0828886958,
    0.0634416395, 0.0438249658, 0.0264623493, 0.0131400047,
];

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48000.0;

    /// Simple pass-through effect for testing
    struct Passthrough;

    impl Effect for Passthrough {
        fn process(&mut self, input: f32) -> f32 {
            input
        }
        fn set_sample_rate(&mut self, _: f32) {}
        fn reset(&mut self) {}
    }

    /// Gain effect for testing
    struct Gain(f32);

    impl Effect for Gain {
        fn process(&mut self, input: f32) -> f32 {
            input * self.0
        }
        fn set_sample_rate(&mut self, _: f32) {}
        fn reset(&mut self) {}
    }

    #[test]
    fn passthrough_dc_unity() {
        let mut oversampled = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);

        // Let filter settle with DC signal
        for _ in 0..200 {
            oversampled.process(1.0);
        }

        // Should pass through unity DC signal
        let output = oversampled.process(1.0);
        assert!(
            (output - 1.0).abs() < 0.02,
            "Passthrough should be near unity, got {}",
            output
        );
    }

    #[test]
    fn gain_preserved() {
        let mut oversampled = Oversampled::<4, _>::new(Gain(0.5), SAMPLE_RATE);

        // Let filter settle
        for _ in 0..200 {
            oversampled.process(1.0);
        }

        let output = oversampled.process(1.0);
        assert!(
            (output - 0.5).abs() < 0.02,
            "Gain should be ~0.5, got {}",
            output
        );
    }

    #[test]
    fn inner_access() {
        let mut oversampled = Oversampled::<2, _>::new(Gain(1.0), SAMPLE_RATE);

        // Should be able to modify inner effect
        oversampled.inner_mut().0 = 2.0;
        assert_eq!(oversampled.inner().0, 2.0);
    }

    #[test]
    fn reset_clears_state() {
        let mut oversampled = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);

        // Process some signal
        for _ in 0..100 {
            oversampled.process(1.0);
        }

        // Reset
        oversampled.reset();

        // Process zeros - output should trend toward zero
        let mut output = 0.0;
        for _ in 0..50 {
            output = oversampled.process(0.0);
        }
        assert!(
            output.abs() < 0.1,
            "After reset and zero input, output should approach zero, got {}",
            output
        );
    }

    #[test]
    fn factor_2_works() {
        let mut oversampled = Oversampled::<2, _>::new(Passthrough, SAMPLE_RATE);
        assert_eq!(oversampled.factor(), 2);

        for _ in 0..200 {
            oversampled.process(1.0);
        }
        let output = oversampled.process(1.0);
        assert!(
            (output - 1.0).abs() < 0.05,
            "Factor 2 passthrough should be ~1.0, got {}",
            output
        );
    }

    #[test]
    fn factor_8_works() {
        let mut oversampled = Oversampled::<8, _>::new(Passthrough, SAMPLE_RATE);
        assert_eq!(oversampled.factor(), 8);

        for _ in 0..200 {
            oversampled.process(1.0);
        }
        let output = oversampled.process(1.0);
        assert!(
            (output - 1.0).abs() < 0.02,
            "Factor 8 passthrough should be ~1.0, got {}",
            output
        );
    }
}
