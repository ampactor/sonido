//! Generic oversampling wrapper for anti-aliased nonlinear processing.
//!
//! Nonlinear effects (distortion, waveshaping, saturation) generate harmonics
//! that can exceed Nyquist and alias back into the audible range. Oversampling
//! mitigates this by:
//!
//! 1. **Upsampling**: Increase sample rate by factor N (windowed-sinc interpolation)
//! 2. **Processing**: Run the effect at N× sample rate (harmonics stay below Nyquist)
//! 3. **Downsampling**: Return to original rate (FIR lowpass + decimation)
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
///
/// Increased from 15 to 47 for >80 dB stopband attenuation. The 48-tap
/// symmetric FIR provides sufficient transition-band steepness to keep
/// alias products inaudible even for aggressive waveshaping.
const FILTER_ORDER: usize = 47;

/// Number of filter taps.
const FILTER_TAPS: usize = FILTER_ORDER + 1;

/// Number of history samples needed for the upsampling sinc kernel.
/// The kernel spans UPSAMPLE_TAPS input samples centered on the interpolation point.
const UPSAMPLE_TAPS: usize = 8;

/// Oversampling wrapper for any effect.
///
/// Wraps an effect with upsampling and downsampling to reduce aliasing
/// from nonlinear processing. The inner effect runs at `FACTOR` times
/// the base sample rate.
///
/// # Why Oversampling Matters
///
/// Nonlinear operations (distortion, waveshaping, saturation) generate new
/// harmonic content that may exceed the Nyquist frequency and alias back
/// into the audible band. Oversampling solves this by temporarily increasing
/// the sample rate so that generated harmonics remain below Nyquist, then
/// filtering and decimating back to the original rate.
///
/// For example, a hard-clipped 5 kHz sine at 48 kHz generates harmonics at
/// 15, 25, 35 kHz... The 25 kHz harmonic aliases to 23 kHz. With 4x
/// oversampling (192 kHz effective rate), all harmonics up to 96 kHz are
/// represented without aliasing.
///
/// # Signal Path
///
/// ```text
/// Input → Windowed-Sinc Interpolation (upsample) → Effect at N×fs → FIR Lowpass → Decimation → Output
/// ```
///
/// The upsampling uses windowed-sinc interpolation with a Blackman-Harris window
/// over an 8-sample kernel for each polyphase sub-filter. This eliminates the
/// HF rolloff inherent in linear interpolation and provides clean interpolation
/// with >92 dB sidelobe suppression from the window.
///
/// The downsampling uses a 48-tap Kaiser-windowed sinc FIR filter (beta ≈ 8.0)
/// with cutoff frequencies tuned per oversampling factor (0.45×, 0.22×, 0.11×
/// Nyquist for 2×, 4×, 8× respectively). Stopband attenuation exceeds 80 dB.
///
/// # Type Parameters
///
/// - `FACTOR`: Oversampling factor (2, 4, or 8)
/// - `E`: The effect type being wrapped
///
/// # Memory Usage
///
/// Uses fixed-size arrays for filter state, suitable for `no_std`:
/// - Input history for sinc interpolation: `UPSAMPLE_TAPS` × `f32` = 32 bytes
/// - Downsample FIR filter state: `FILTER_TAPS` × `f32` = 192 bytes
/// - Upsample work buffer: `MAX_OVERSAMPLE_FACTOR` × `f32` = 32 bytes
///
/// # References
///
/// - A.V. Oppenheim & R.W. Schafer, "Discrete-Time Signal Processing", Ch. 4 & 7
/// - F.J. Harris, "On the Use of Windows for Harmonic Analysis with the DFT",
///   Proc. IEEE, 1978 (window functions and sidelobe behavior)
pub struct Oversampled<const FACTOR: usize, E: Effect> {
    /// The wrapped effect
    effect: E,
    /// Base sample rate (before oversampling)
    sample_rate: f32,
    /// Input sample history for windowed-sinc upsampling.
    /// Stores the last UPSAMPLE_TAPS input samples as a circular buffer.
    input_history: [f32; UPSAMPLE_TAPS],
    /// Write position in the input history circular buffer.
    history_pos: usize,
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
            input_history: [0.0; UPSAMPLE_TAPS],
            history_pos: 0,
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

    /// Get anti-aliasing FIR filter coefficients for the current oversampling factor.
    ///
    /// The coefficients are pre-computed Kaiser-windowed sinc values (beta ≈ 8.0),
    /// stored as static arrays. Each factor has a different cutoff frequency
    /// corresponding to the Nyquist of the original (non-oversampled) sample rate:
    /// - 2×: cutoff at 0.45 × oversampled Nyquist (slight transition band margin)
    /// - 4×: cutoff at 0.22 × oversampled Nyquist
    /// - 8×: cutoff at 0.11 × oversampled Nyquist
    ///
    /// All coefficient sets achieve >80 dB stopband attenuation.
    #[inline]
    fn get_coefficients(&self) -> &'static [f32; FILTER_TAPS] {
        match FACTOR {
            2 => &COEFFS_2X,
            4 => &COEFFS_4X,
            8 => &COEFFS_8X,
            _ => unreachable!(),
        }
    }

    /// Get the polyphase upsampling kernel for the current factor.
    ///
    /// Returns a reference to a `[FACTOR][UPSAMPLE_TAPS]` array of precomputed
    /// windowed-sinc values. Each row is one polyphase sub-filter corresponding
    /// to a fractional phase offset `(p+1)/FACTOR` for sub-sample `p`.
    #[inline]
    fn get_upsample_kernel(&self) -> &'static [[f32; UPSAMPLE_TAPS]] {
        match FACTOR {
            2 => &UPSAMPLE_KERNEL_2X,
            4 => &UPSAMPLE_KERNEL_4X,
            8 => &UPSAMPLE_KERNEL_8X,
            _ => unreachable!(),
        }
    }

    /// Upsample using windowed-sinc interpolation.
    ///
    /// For each output sub-sample, convolves the input history with the
    /// appropriate polyphase component of the sinc kernel. This provides
    /// band-limited interpolation superior to linear interpolation, with
    /// >90 dB sidelobe suppression from the Blackman-Harris window.
    ///
    /// The polyphase decomposition avoids computing zeros: instead of
    /// zero-stuffing and filtering, we directly evaluate the sinc kernel
    /// at the required fractional offsets.
    #[inline]
    fn upsample(&mut self, input: f32) {
        // Push new sample into history
        self.input_history[self.history_pos] = input;
        self.history_pos = (self.history_pos + 1) % UPSAMPLE_TAPS;

        let kernel = self.get_upsample_kernel();

        for p in 0..FACTOR {
            let mut sum = 0.0;
            let k = &kernel[p];
            for t in 0..UPSAMPLE_TAPS {
                // Read from history in correct order (oldest to newest)
                let idx = (self.history_pos + t) % UPSAMPLE_TAPS;
                sum += self.input_history[idx] * k[t];
            }
            self.work_buffer[p] = sum * FACTOR as f32;
        }
    }

    /// Downsample with FIR anti-aliasing filter and decimation.
    ///
    /// All upsampled/processed samples are pushed through the FIR filter's
    /// delay line, but only the final sample is used as output (decimation).
    /// This is equivalent to filtering the entire oversampled signal and then
    /// keeping every FACTOR-th sample, but more efficient since we only compute
    /// the convolution sum at the decimation points.
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
        // Upsample via windowed-sinc interpolation
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
        self.input_history = [0.0; UPSAMPLE_TAPS];
        self.history_pos = 0;
        self.downsample_state = [0.0; FILTER_TAPS];
        self.work_buffer = [0.0; MAX_OVERSAMPLE_FACTOR];
        self.effect.reset();
    }

    fn latency_samples(&self) -> usize {
        // Filter latency (group delay) + inner effect latency
        // FIR filter group delay = (taps - 1) / 2 for symmetric filter
        // Plus sinc interpolation latency = UPSAMPLE_TAPS / 2
        let filter_latency = FILTER_ORDER / 2;
        let upsample_latency = UPSAMPLE_TAPS / 2;
        filter_latency + upsample_latency + self.effect.latency_samples()
    }
}

// ============================================================================
// Downsample Filter Coefficients (48-tap Kaiser-windowed sinc, beta ≈ 8.0)
// ============================================================================
//
// Lowpass FIR filter coefficients for anti-aliasing during downsampling.
//
// Design method: Kaiser-windowed sinc with beta = 8.0 for >80 dB stopband
// attenuation. The 48-tap symmetric FIR provides linear phase, preserving
// waveform shape through the oversampling path.
//
// Each coefficient set targets a different normalized cutoff frequency:
//   - 2×: cutoff at 0.45 × oversampled Nyquist
//   - 4×: cutoff at 0.22 × oversampled Nyquist
//   - 8×: cutoff at 0.11 × oversampled Nyquist
//
// The coefficients are symmetric (linear phase), so group delay is
// constant at (FILTER_ORDER / 2) = 23.5 samples at the oversampled rate.
//
// Coefficients computed via: h[n] = sinc(2*fc*(n - M/2)) * w_kaiser[n]
// where M = FILTER_ORDER, fc = normalized cutoff, and w_kaiser is the
// Kaiser window with the specified beta.
//
// Reference: A.V. Oppenheim & R.W. Schafer, "Discrete-Time Signal Processing",
// Chapter 7 (FIR filter design using the window method).
// J.F. Kaiser, "Nonrecursive Digital Filter Design Using the I0-sinh Window",
// Proc. IEEE Int. Symp. Circuits & Systems, 1974.

/// 2× oversampling downsample filter coefficients.
///
/// Half-band lowpass FIR with cutoff at 0.45 × oversampled Nyquist.
/// Design: Kaiser-windowed sinc, beta = 8.0, >80 dB stopband attenuation.
/// 48 taps, symmetric (linear phase). Coefficient sum = 1.0 (unity DC gain).
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static COEFFS_2X: [f32; FILTER_TAPS] = [
     0.000_030_805,  0.000_036_066, -0.000_173_870, -0.000_246_921,
     0.000_420_167,  0.000_880_584, -0.000_601_289, -0.002_237_806,
     0.000_256_421,  0.004_509_578,  0.001_430_439, -0.007_530_765,
    -0.005_580_807,  0.010_513_124,  0.013_481_583, -0.011_805_484,
    -0.026_534_781,  0.008_541_637,  0.046_881_021,  0.004_832_767,
    -0.081_356_477, -0.046_700_191,  0.178_198_253,  0.412_755_947,
     0.412_755_947,  0.178_198_253, -0.046_700_191, -0.081_356_477,
     0.004_832_767,  0.046_881_021,  0.008_541_637, -0.026_534_781,
    -0.011_805_484,  0.013_481_583,  0.010_513_124, -0.005_580_807,
    -0.007_530_765,  0.001_430_439,  0.004_509_578,  0.000_256_421,
    -0.002_237_806, -0.000_601_289,  0.000_880_584,  0.000_420_167,
    -0.000_246_921, -0.000_173_870,  0.000_036_066,  0.000_030_805,
];

/// 4× oversampling downsample filter coefficients.
///
/// Lowpass FIR with cutoff at 0.225 × oversampled Nyquist.
/// Design: Kaiser-windowed sinc, beta = 8.0, >80 dB stopband attenuation.
/// 48 taps, symmetric (linear phase). Coefficient sum = 1.0 (unity DC gain).
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static COEFFS_4X: [f32; FILTER_TAPS] = [
    -0.000_024_878, -0.000_018_386,  0.000_099_637,  0.000_356_694,
     0.000_606_960,  0.000_504_624, -0.000_306_528, -0.001_807_286,
    -0.003_265_620, -0.003_321_654, -0.000_720_197,  0.004_528_492,
     0.010_279_768,  0.012_555_426,  0.007_422_453, -0.006_132_875,
    -0.023_880_170, -0.036_335_063, -0.031_920_603, -0.002_418_197,
     0.051_797_410,  0.119_686_124,  0.182_344_206,  0.219_969_664,
     0.219_969_664,  0.182_344_206,  0.119_686_124,  0.051_797_410,
    -0.002_418_197, -0.031_920_603, -0.036_335_063, -0.023_880_170,
    -0.006_132_875,  0.007_422_453,  0.012_555_426,  0.010_279_768,
     0.004_528_492, -0.000_720_197, -0.003_321_654, -0.003_265_620,
    -0.001_807_286, -0.000_306_528,  0.000_504_624,  0.000_606_960,
     0.000_356_694,  0.000_099_637, -0.000_018_386, -0.000_024_878,
];

/// 8× oversampling downsample filter coefficients.
///
/// Lowpass FIR with cutoff at 0.1125 × oversampled Nyquist.
/// Design: Kaiser-windowed sinc, beta = 8.0, >80 dB stopband attenuation.
/// 48 taps, symmetric (linear phase). Coefficient sum = 1.0 (unity DC gain).
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static COEFFS_8X: [f32; FILTER_TAPS] = [
     0.000_028_500,  0.000_093_778,  0.000_197_285,  0.000_311_875,
     0.000_369_874,  0.000_260_731, -0.000_153_988, -0.001_004_212,
    -0.002_355_577, -0.004_143_828, -0.006_116_044, -0.007_799_182,
    -0.008_515_014, -0.007_452_927, -0.003_799_063,  0.003_095_272,
     0.013_537_148,  0.027_347_166,  0.043_785_295,  0.061_575_686,
     0.079_039_414,  0.094_320_218,  0.105_665_797,  0.111_711_797,
     0.111_711_797,  0.105_665_797,  0.094_320_218,  0.079_039_414,
     0.061_575_686,  0.043_785_295,  0.027_347_166,  0.013_537_148,
     0.003_095_272, -0.003_799_063, -0.007_452_927, -0.008_515_014,
    -0.007_799_182, -0.006_116_044, -0.004_143_828, -0.002_355_577,
    -0.001_004_212, -0.000_153_988,  0.000_260_731,  0.000_369_874,
     0.000_311_875,  0.000_197_285,  0.000_093_778,  0.000_028_500,
];

// ============================================================================
// Upsampling Polyphase Sinc Kernels (Blackman-Harris windowed)
// ============================================================================
//
// Each kernel is a 2D array [FACTOR][UPSAMPLE_TAPS] representing polyphase
// sub-filters for windowed-sinc interpolation. The sinc is centered at
// index (UPSAMPLE_TAPS - 1) / 2 = 3.5 (between taps 3 and 4).
//
// For each output sub-sample p (0..FACTOR), the fractional offset from center
// is d = -0.5 + (p + 1) / FACTOR. The kernel evaluates:
//   h[p][t] = sinc(t - center - d) * blackman_harris(t, UPSAMPLE_TAPS)
//
// The Blackman-Harris window provides >92 dB sidelobe suppression, ensuring
// that interpolation images are well below the noise floor.
//
// Each row is normalized to sum to 1/FACTOR. The upsample function multiplies
// by FACTOR to compensate, yielding unity DC gain overall.
//
// Reference: F.J. Harris, "On the Use of Windows for Harmonic Analysis with
// the DFT", Proc. IEEE, 1978.

/// Polyphase upsampling kernel for 2× oversampling.
///
/// 2 phases × 8 taps. Blackman-Harris windowed sinc.
/// Phase 0: symmetric sinc centered between taps 3 and 4.
/// Phase 1: sinc peaked at tap 4 (integer sample position).
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static UPSAMPLE_KERNEL_2X: [[f32; UPSAMPLE_TAPS]; 2] = [
    // Phase 0: offset +0.0 from center (symmetric half-sample interpolation)
    [ -0.000_002_729,  0.002_126_604, -0.035_328_366,  0.283_204_492,
       0.283_204_492, -0.035_328_366,  0.002_126_604, -0.000_002_729 ],
    // Phase 1: offset +0.5 from center (integer sample, identity-like)
    [  0.000_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000,
       0.500_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000 ],
];

/// Polyphase upsampling kernel for 4× oversampling.
///
/// 4 phases × 8 taps. Blackman-Harris windowed sinc.
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static UPSAMPLE_KERNEL_4X: [[f32; UPSAMPLE_TAPS]; 4] = [
    // Phase 0: offset -0.25 from center
    [ -0.000_001_070,  0.000_860_076, -0.015_431_116,  0.206_168_674,
       0.068_722_891, -0.011_022_226,  0.000_703_698, -0.000_000_927 ],
    // Phase 1: offset +0.0 from center (symmetric)
    [ -0.000_001_365,  0.001_063_302, -0.017_664_183,  0.141_602_246,
       0.141_602_246, -0.017_664_183,  0.001_063_302, -0.000_001_365 ],
    // Phase 2: offset +0.25 from center
    [ -0.000_000_927,  0.000_703_698, -0.011_022_226,  0.068_722_891,
       0.206_168_674, -0.015_431_116,  0.000_860_076, -0.000_001_070 ],
    // Phase 3: offset +0.5 from center (integer sample, identity-like)
    [  0.000_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000,
       0.250_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000 ],
];

/// Polyphase upsampling kernel for 8× oversampling.
///
/// 8 phases × 8 taps. Blackman-Harris windowed sinc.
#[allow(clippy::excessive_precision)]
#[rustfmt::skip]
static UPSAMPLE_KERNEL_8X: [[f32; UPSAMPLE_TAPS]; 8] = [
    // Phase 0: offset -0.375 from center
    [ -0.000_000_312,  0.000_255_581, -0.004_811_972,  0.115_723_327,
       0.016_531_904, -0.002_887_183,  0.000_188_908, -0.000_000_252 ],
    // Phase 1: offset -0.25 from center
    [ -0.000_000_535,  0.000_430_038, -0.007_715_558,  0.103_084_337,
       0.034_361_446, -0.005_511_113,  0.000_351_849, -0.000_000_464 ],
    // Phase 2: offset -0.125 from center
    [ -0.000_000_659,  0.000_520_804, -0.008_966_517,  0.087_851_770,
       0.052_711_062, -0.007_587_053,  0.000_471_204, -0.000_000_613 ],
    // Phase 3: offset +0.0 from center (symmetric)
    [ -0.000_000_682,  0.000_531_651, -0.008_832_092,  0.070_801_123,
       0.070_801_123, -0.008_832_092,  0.000_531_651, -0.000_000_682 ],
    // Phase 4: offset +0.125 from center
    [ -0.000_000_613,  0.000_471_204, -0.007_587_053,  0.052_711_062,
       0.087_851_770, -0.008_966_517,  0.000_520_804, -0.000_000_659 ],
    // Phase 5: offset +0.25 from center
    [ -0.000_000_464,  0.000_351_849, -0.005_511_113,  0.034_361_446,
       0.103_084_337, -0.007_715_558,  0.000_430_038, -0.000_000_535 ],
    // Phase 6: offset +0.375 from center
    [ -0.000_000_252,  0.000_188_908, -0.002_887_183,  0.016_531_904,
       0.115_723_327, -0.004_811_972,  0.000_255_581, -0.000_000_312 ],
    // Phase 7: offset +0.5 from center (integer sample, identity-like)
    [  0.000_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000,
       0.125_000_000,  0.000_000_000,  0.000_000_000,  0.000_000_000 ],
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
        for _ in 0..500 {
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
        for _ in 0..500 {
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
        for _ in 0..100 {
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

        for _ in 0..500 {
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

        for _ in 0..500 {
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
