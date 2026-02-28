//! Rational resampling — decimate, interpolate, and arbitrary P/Q rate conversion.
//!
//! Implements polyphase FIR resampling using windowed-sinc lowpass filters with
//! Blackman windowing. Supports integer decimation, integer interpolation, and
//! rational P/Q resampling (e.g., 44100 → 48000 Hz via P=160, Q=147).
//!
//! # Theory
//!
//! Resampling by rational factor P/Q is equivalent to upsampling by P, applying
//! a lowpass filter at `min(1/P, 1/Q)` (normalized frequency), then downsampling
//! by Q. The polyphase decomposition avoids explicit zero-insertion by computing
//! only the output samples actually needed.
//!
//! The anti-aliasing lowpass uses a windowed-sinc design:
//!   `h[n] = sinc(cutoff * (n - M/2)) * w[n]`
//! where `w[n]` is a Blackman window and the result is normalized to unity DC gain.
//!
//! Reference: P. P. Vaidyanathan, *Multirate Systems and Filter Banks*, Prentice Hall,
//! 1993, Chapter 4.
//!
//! # Example
//!
//! ```rust
//! use sonido_analysis::resample::{decimate, interpolate, resample};
//!
//! let sr = 48000.0_f32;
//! let signal: Vec<f32> = (0..4800)
//!     .map(|i| (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
//!     .collect();
//!
//! // Downsample 48 kHz → 24 kHz
//! let downsampled = decimate(&signal, 2, 0);
//! assert_eq!(downsampled.len(), signal.len() / 2);
//!
//! // Rational resampling: 44.1 kHz → 48 kHz
//! let resampled = resample(&signal, 160, 147, 0);
//! ```

use std::f32::consts::PI;

/// Compute windowed-sinc lowpass FIR coefficients.
///
/// Designs a Type I linear-phase FIR lowpass filter using the windowed-sinc
/// method with a Blackman window. The filter is normalized to have unity gain
/// at DC (sum of coefficients = 1.0).
///
/// The impulse response is:
///   `h[n] = sinc(cutoff * (n - M/2)) * w_blackman[n]`
/// where sinc(x) = sin(π·x) / (π·x) and M = num_taps - 1.
///
/// Blackman window:
///   `w[n] = 0.42 - 0.5·cos(2πn/M) + 0.08·cos(4πn/M)`
///
/// # Arguments
///
/// * `num_taps` - Number of filter taps (length of the coefficient vector).
///   Longer filters give sharper cutoff and better stopband rejection.
///   Odd tap counts produce a symmetric Type I filter.
/// * `cutoff` - Normalized cutoff frequency in the range (0.0, 1.0),
///   where 1.0 corresponds to the Nyquist frequency (fs/2).
///
/// # Returns
///
/// FIR coefficient vector of length `num_taps`, normalized to sum = 1.0.
///
/// Reference: A. V. Oppenheim and R. W. Schafer, *Discrete-Time Signal Processing*,
/// 3rd ed., Prentice Hall, 2009, Section 7.6.
pub fn design_lowpass(num_taps: usize, cutoff: f32) -> Vec<f32> {
    if num_taps == 0 {
        return Vec::new();
    }

    let m = num_taps - 1;
    let mut coeffs = Vec::with_capacity(num_taps);

    for n in 0..num_taps {
        let x = n as f32 - m as f32 / 2.0;

        // Windowed-sinc: sinc(cutoff * x) * blackman(n)
        let sinc = if x.abs() < 1e-7 {
            cutoff
        } else {
            (PI * cutoff * x).sin() / (PI * x)
        };

        // Blackman window: w[n] = 0.42 - 0.5*cos(2πn/M) + 0.08*cos(4πn/M)
        let window = if m == 0 {
            1.0
        } else {
            let phase = 2.0 * PI * n as f32 / m as f32;
            0.42 - 0.5 * phase.cos() + 0.08 * (2.0 * phase).cos()
        };

        coeffs.push(sinc * window);
    }

    // Normalize to unity DC gain (sum of coefficients = 1.0)
    let sum: f32 = coeffs.iter().sum();
    if sum.abs() > 1e-10 {
        for c in coeffs.iter_mut() {
            *c /= sum;
        }
    }

    coeffs
}

/// Apply a FIR filter to a signal using direct convolution.
///
/// Pads the input with `(coeffs.len() - 1) / 2` zeros on each side to produce
/// an output of the same length as the input (linear-phase delay compensation).
///
/// # Arguments
///
/// * `signal` - Input samples
/// * `coeffs` - FIR filter coefficients (assumed symmetric / linear-phase)
///
/// # Returns
///
/// Filtered output, same length as `signal`.
fn apply_fir(signal: &[f32], coeffs: &[f32]) -> Vec<f32> {
    if coeffs.is_empty() || signal.is_empty() {
        return signal.to_vec();
    }

    let half_delay = (coeffs.len() - 1) / 2;
    let mut output = Vec::with_capacity(signal.len());

    for i in 0..signal.len() {
        let mut acc = 0.0f32;
        for (k, &c) in coeffs.iter().enumerate() {
            // Map tap k to signal index with delay compensation
            let j = i + k;
            if j >= half_delay && j - half_delay < signal.len() {
                acc += c * signal[j - half_delay];
            }
        }
        output.push(acc);
    }

    output
}

/// Compute the greatest common divisor of two integers.
fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

/// Decimate a signal by an integer factor.
///
/// Applies a windowed-sinc anti-aliasing lowpass FIR filter with cutoff at
/// `0.9 / factor` (normalized, leaving a 10% guard band), then downsamples
/// by selecting every `factor`-th sample.
///
/// The output sample rate is `fs / factor`. Signal components above the new
/// Nyquist (`fs / (2 * factor)`) are attenuated by the anti-aliasing filter.
///
/// # Arguments
///
/// * `signal` - Input samples at the original sample rate
/// * `factor` - Integer decimation factor (must be ≥ 1)
/// * `filter_order` - FIR filter length. Pass `0` to use the automatic default
///   of `4 * factor * 10 + 1` taps, which gives approximately 60 dB stopband
///   rejection for most decimation factors.
///
/// # Returns
///
/// Decimated output of length `signal.len() / factor`.
///
/// Reference: R. E. Crochiere and L. R. Rabiner, *Multirate Digital Signal Processing*,
/// Prentice Hall, 1983, Chapter 2.
pub fn decimate(signal: &[f32], factor: usize, filter_order: usize) -> Vec<f32> {
    assert!(factor >= 1, "decimation factor must be >= 1");

    if factor == 1 {
        return signal.to_vec();
    }

    let num_taps = if filter_order == 0 {
        4 * factor * 10 + 1
    } else {
        filter_order
    };

    // Anti-aliasing cutoff: just below new Nyquist (0.9 / factor normalized)
    let cutoff = 0.9 / factor as f32;
    let coeffs = design_lowpass(num_taps, cutoff);

    let filtered = apply_fir(signal, &coeffs);

    // Downsample: keep every factor-th sample
    filtered.into_iter().step_by(factor).collect()
}

/// Interpolate a signal by an integer factor.
///
/// Upsamples by inserting `factor - 1` zeros between each input sample, then
/// applies a windowed-sinc lowpass FIR filter with cutoff at `0.9 / factor`
/// to reconstruct the interpolated waveform. The filter output is scaled by
/// `factor` to restore unity gain.
///
/// The output sample rate is `fs * factor`. Signal content is preserved up to
/// the original Nyquist (`fs / 2`); spectral images above this frequency are
/// suppressed by the anti-aliasing filter.
///
/// # Arguments
///
/// * `signal` - Input samples at the original (lower) sample rate
/// * `factor` - Integer interpolation factor (must be ≥ 1)
/// * `filter_order` - FIR filter length. Pass `0` for the automatic default
///   of `4 * factor * 10 + 1` taps.
///
/// # Returns
///
/// Interpolated output of length `signal.len() * factor`.
///
/// Reference: P. P. Vaidyanathan, *Multirate Systems and Filter Banks*, Chapter 4.
pub fn interpolate(signal: &[f32], factor: usize, filter_order: usize) -> Vec<f32> {
    assert!(factor >= 1, "interpolation factor must be >= 1");

    if factor == 1 {
        return signal.to_vec();
    }

    let num_taps = if filter_order == 0 {
        4 * factor * 10 + 1
    } else {
        filter_order
    };

    // Lowpass cutoff to suppress images: 0.9 / factor (normalized)
    let cutoff = 0.9 / factor as f32;
    let coeffs = design_lowpass(num_taps, cutoff);

    // Zero-insert: expand to factor * len
    let upsampled_len = signal.len() * factor;
    let mut upsampled = vec![0.0f32; upsampled_len];
    for (i, &s) in signal.iter().enumerate() {
        upsampled[i * factor] = s;
    }

    // Filter and scale by factor to restore unity gain
    let filtered = apply_fir(&upsampled, &coeffs);
    filtered.into_iter().map(|x| x * factor as f32).collect()
}

/// Rational resampling by the factor P/Q.
///
/// Converts a signal from one sample rate to another by the rational ratio P/Q,
/// where P is the upsampling (interpolation) factor and Q is the downsampling
/// (decimation) factor. For example, converting 44100 Hz → 48000 Hz uses P=160, Q=147
/// (since 48000/44100 = 160/147 in lowest terms).
///
/// The output length is `ceil(input.len() * P / Q)`.
///
/// # Algorithm
///
/// Polyphase decomposition avoids explicit zero-insertion:
/// 1. Simplify P and Q by their GCD.
/// 2. Design a single prototype lowpass FIR with cutoff `min(0.9/P, 0.9/Q)`.
/// 3. Decompose into P polyphase sub-filters.
/// 4. For each output sample `m`, determine: which input samples contribute,
///    and which polyphase sub-filter to use.
///    - Input frame index: `n = floor(m * Q / P)`
///    - Sub-filter phase: `k = (m * Q) mod P`
/// 5. Apply sub-filter `k` to input samples starting at `n`.
///
/// This computes exactly the samples that would be in the P-upsampled then
/// Q-downsampled result, with O(filter_len / P) multiplications per output sample.
///
/// # Arguments
///
/// * `signal` - Input samples at the source sample rate
/// * `p` - Upsampling factor (numerator of the rate ratio; must be ≥ 1)
/// * `q` - Downsampling factor (denominator of the rate ratio; must be ≥ 1)
/// * `filter_order` - Total prototype FIR length. Pass `0` for the automatic
///   default of `4 * max(P, Q) * 10 + 1` taps.
///
/// # Returns
///
/// Resampled output of length `ceil(signal.len() * P / Q)`.
///
/// Reference: P. P. Vaidyanathan, *Multirate Systems and Filter Banks*,
/// Prentice Hall, 1993, Section 4.3 (Polyphase Representation).
pub fn resample(signal: &[f32], p: usize, q: usize, filter_order: usize) -> Vec<f32> {
    assert!(p >= 1, "upsample factor P must be >= 1");
    assert!(q >= 1, "downsample factor Q must be >= 1");

    // Simplify by GCD
    let g = gcd(p, q);
    let p = p / g;
    let q = q / g;

    if p == 1 && q == 1 {
        return signal.to_vec();
    }

    let num_taps = if filter_order == 0 {
        4 * p.max(q) * 10 + 1
    } else {
        filter_order
    };

    // Prototype lowpass: cutoff at min(1/P, 1/Q) with 10% guard band
    let cutoff = 0.9 / p.max(q) as f32;
    let prototype = design_lowpass(num_taps, cutoff);

    // Output length: ceil(input_len * P / Q)
    let out_len = (signal.len() * p).div_ceil(q);

    // Taps per polyphase sub-filter
    let taps_per_phase = num_taps.div_ceil(p);

    // Decompose prototype into P polyphase sub-filters.
    // Sub-filter k contains prototype taps at indices k, k+P, k+2P, ...
    // polyphase[k][i] = prototype[k + i*P]  (with zero-padding for out-of-range)
    let mut polyphase = vec![vec![0.0f32; taps_per_phase]; p];
    for (tap_idx, &coeff) in prototype.iter().enumerate() {
        let k = tap_idx % p;
        let i = tap_idx / p;
        polyphase[k][i] = coeff;
    }

    let mut output = Vec::with_capacity(out_len);

    for m in 0..out_len {
        // Which input frame and sub-filter phase?
        let full_idx = m * q; // position in the P-upsampled sequence
        let n = full_idx / p; // corresponding input sample index
        let k = full_idx % p; // polyphase branch

        // The sub-filter k is convolved with input starting at n, going backward:
        // y[m] = sum_i  polyphase[k][i] * x[n - i]
        let sub_filter = &polyphase[k];
        let mut acc = 0.0f32;
        for (i, &coeff) in sub_filter.iter().enumerate() {
            if n >= i && (n - i) < signal.len() {
                acc += coeff * signal[n - i];
            }
        }

        // Scale by P so that for p==q, output ≈ input
        output.push(acc * p as f32);
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a sine wave at `frequency` Hz, sampled at `sample_rate` Hz.
    fn sine_wave(frequency: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
        (0..num_samples)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect()
    }

    /// Estimate peak amplitude of a frequency bin via the DFT of a windowed block.
    fn spectral_peak_at(signal: &[f32], freq_hz: f32, sample_rate: f32) -> f32 {
        let n = signal.len();
        // Goertzel / direct DFT for a single frequency
        let mut re = 0.0f32;
        let mut im = 0.0f32;
        for (i, &s) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq_hz * i as f32 / sample_rate;
            re += s * phase.cos();
            im += s * phase.sin();
        }
        (re * re + im * im).sqrt() / n as f32
    }

    #[test]
    fn test_design_lowpass_symmetry() {
        // A linear-phase FIR must have symmetric coefficients.
        let coeffs = design_lowpass(65, 0.4);
        let n = coeffs.len();
        for i in 0..n / 2 {
            assert!(
                (coeffs[i] - coeffs[n - 1 - i]).abs() < 1e-6,
                "Coefficients not symmetric at index {}: {} vs {}",
                i,
                coeffs[i],
                coeffs[n - 1 - i]
            );
        }
    }

    #[test]
    fn test_design_lowpass_unity_dc() {
        // DC gain should be 1.0 (sum of coefficients).
        for &num_taps in &[11usize, 31, 65, 127] {
            let coeffs = design_lowpass(num_taps, 0.5);
            let sum: f32 = coeffs.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-5,
                "DC gain not ~1.0 for {} taps: got {}",
                num_taps,
                sum
            );
        }
    }

    #[test]
    fn test_decimate_by_2() {
        // 1 kHz sine at 48 kHz decimated to 24 kHz.
        // The tone should be preserved in the output.
        let sr = 48000.0;
        let signal = sine_wave(1000.0, sr, 4800);
        let decimated = decimate(&signal, 2, 0);

        assert_eq!(decimated.len(), signal.len() / 2);

        // 1 kHz should be well below the new Nyquist (12 kHz), so it survives.
        let peak = spectral_peak_at(&decimated[100..], 1000.0, sr / 2.0);
        assert!(
            peak > 0.3,
            "1 kHz tone should survive decimation, peak={}",
            peak
        );
    }

    #[test]
    fn test_interpolate_by_2() {
        // 1 kHz sine at 24 kHz interpolated to 48 kHz.
        // The tone at 1 kHz should be preserved; images at 23 kHz etc. should not appear.
        let sr = 24000.0;
        let signal = sine_wave(1000.0, sr, 2400);
        let interpolated = interpolate(&signal, 2, 0);

        assert_eq!(interpolated.len(), signal.len() * 2);

        // The 1 kHz component should be present in the output at 48 kHz.
        let peak_signal = spectral_peak_at(&interpolated[200..], 1000.0, sr * 2.0);
        assert!(
            peak_signal > 0.3,
            "1 kHz tone should be preserved after interpolation, peak={}",
            peak_signal
        );

        // Image at 23 kHz (24 kHz - 1 kHz) should be strongly attenuated.
        let peak_image = spectral_peak_at(&interpolated[200..], 23000.0, sr * 2.0);
        assert!(
            peak_image < peak_signal * 0.1,
            "Image at 23 kHz should be attenuated: signal={}, image={}",
            peak_signal,
            peak_image
        );
    }

    #[test]
    fn test_resample_identity() {
        // resample(x, 2, 2) should simplify to (1,1) and return x unchanged.
        let signal: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let result = resample(&signal, 2, 2, 0);
        assert_eq!(result.len(), signal.len());
        for (a, b) in signal.iter().zip(result.iter()) {
            assert!(
                (a - b).abs() < 1e-5,
                "Identity resample mismatch: {} vs {}",
                a,
                b
            );
        }
    }

    #[test]
    fn test_resample_44100_to_48000() {
        // 44.1 kHz → 48 kHz: P=160, Q=147.
        // A 1 kHz tone should survive the conversion.
        let sr_in = 44100.0f32;
        let signal = sine_wave(1000.0, sr_in, 44100); // 1 second
        let resampled = resample(&signal, 160, 147, 0);

        let expected_len = (44100usize * 160).div_ceil(147);
        assert_eq!(
            resampled.len(),
            expected_len,
            "Output length should be ceil(44100 * 160 / 147) = {}",
            expected_len
        );

        // 1 kHz should be clearly present at the output sample rate of 48 kHz
        let peak = spectral_peak_at(&resampled[4800..], 1000.0, 48000.0);
        assert!(
            peak > 0.2,
            "1 kHz tone should survive 44.1→48 kHz resampling, peak={}",
            peak
        );
    }

    #[test]
    fn test_decimate_rejects_above_nyquist() {
        // A tone near the decimated Nyquist should be attenuated.
        // Decimating by 4 from 48 kHz → 12 kHz. Nyquist = 6 kHz.
        // A 5 kHz tone (below Nyquist) should survive; 5.5 kHz is borderline; 10 kHz should be rejected.
        let sr = 48000.0;
        let n = 4800;

        let safe_tone = sine_wave(2000.0, sr, n); // well below new Nyquist
        let alias_tone = sine_wave(10000.0, sr, n); // well above new Nyquist

        let dec_safe = decimate(&safe_tone, 4, 0);
        let dec_alias = decimate(&alias_tone, 4, 0);

        let peak_safe = spectral_peak_at(&dec_safe[20..], 2000.0, sr / 4.0);
        let peak_alias: f32 =
            dec_alias.iter().map(|x| x.abs()).sum::<f32>() / dec_alias.len() as f32;

        assert!(
            peak_safe > 0.2,
            "2 kHz should survive decimation by 4, peak={}",
            peak_safe
        );
        assert!(
            peak_alias < 0.05,
            "10 kHz should be rejected by decimation lowpass, mean_abs={}",
            peak_alias
        );
    }

    #[test]
    fn test_resample_rational_length() {
        // Verify output length formula for several P/Q pairs.
        let signal = vec![0.0f32; 1000];
        let cases = [(3, 2), (2, 3), (7, 5), (5, 7), (160, 147)];
        for (p, q) in cases {
            let result = resample(&signal, p, q, 0);
            let g = gcd(p, q);
            let pr = p / g;
            let qr = q / g;
            let expected = (1000 * pr).div_ceil(qr);
            assert_eq!(
                result.len(),
                expected,
                "Length mismatch for P={}, Q={}: got {}, expected {}",
                p,
                q,
                result.len(),
                expected
            );
        }
    }
}
