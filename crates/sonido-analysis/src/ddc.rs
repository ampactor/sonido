//! Digital Down-Conversion (DDC) — shifts a bandpass signal to complex baseband.
//!
//! A DDC performs three operations in sequence:
//! 1. **Mix**: multiply the real input by a complex exponential (numerically controlled
//!    oscillator, NCO) to shift the center frequency to DC.
//! 2. **Filter**: apply a real-valued lowpass FIR to the I and Q channels to suppress
//!    out-of-band noise and alias products.
//! 3. **Decimate**: keep only every `decimation`-th sample to reduce the output rate.
//!
//! # Signal Model
//!
//! For a real input x\[n\] and center frequency ω_c = 2π·f_c/f_s:
//!
//!   I\[n\] = x\[n\] · cos(ω_c · n)
//!   Q\[n\] = x\[n\] · (-sin(ω_c · n))
//!
//! After lowpass filtering and decimation, the complex output y\[m\] = I_d\[m\] + j·Q_d\[m\]
//! represents the analytic baseband signal centered at f_c.
//!
//! # Reference
//!
//! R. G. Lyons, *Understanding Digital Signal Processing*, 3rd ed.,
//! Prentice Hall, 2011, Chapter 8 (Digital Downconverters).
//!
//! # Example
//!
//! ```rust
//! use sonido_analysis::ddc::Ddc;
//! use std::f32::consts::PI;
//!
//! // 1 kHz tone at 48 kHz; shift to baseband, decimate by 8
//! let mut ddc = Ddc::new(48000.0, 1000.0, 8, 0);
//! let signal: Vec<f32> = (0..4800)
//!     .map(|i| (2.0 * PI * 1000.0 * i as f32 / 48000.0).sin())
//!     .collect();
//! let baseband = ddc.process(&signal);
//! assert_eq!(baseband.len(), signal.len() / 8);
//! ```

use crate::resample::design_lowpass;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

/// Digital Down-Converter: shifts a real bandpass signal to complex baseband.
///
/// Mixes the input with a numerically controlled oscillator (NCO) at `center_freq`,
/// filters the I/Q channels with a shared windowed-sinc FIR, and decimates by
/// `decimation` to produce a narrowband complex baseband stream.
///
/// ## Parameters
///
/// - `center_freq`: NCO center frequency in Hz (0.0 to sample_rate / 2)
/// - `sample_rate`: Input sample rate in Hz
/// - `decimation`: Integer decimation factor (output rate = sample_rate / decimation)
/// - `filter_order`: FIR tap count (0 = auto: 4 × decimation × 10 + 1)
pub struct Ddc {
    center_freq: f32,
    sample_rate: f32,
    decimation: usize,
    /// Current NCO phase in radians, accumulated per sample.
    phase: f32,
    /// Phase increment per input sample: 2π · f_c / f_s.
    phase_inc: f32,
    /// Circular buffer for the I (in-phase) channel FIR delay line.
    filter_taps_i: Vec<f32>,
    /// Circular buffer for the Q (quadrature) channel FIR delay line.
    filter_taps_q: Vec<f32>,
    /// FIR lowpass coefficients, shared by both I and Q channels.
    filter_coeffs: Vec<f32>,
    /// Write pointer into the circular delay-line buffers.
    filter_pos: usize,
    /// Sample counter for decimation gating.
    decimate_counter: usize,
}

impl Ddc {
    /// Create a new Digital Down-Converter.
    ///
    /// Designs the anti-aliasing FIR lowpass with cutoff at `0.4 / decimation`
    /// (normalized, 40% of the decimated Nyquist) to leave headroom for the
    /// transition band and prevent aliasing of adjacent channels.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Input sample rate in Hz (e.g., 48000.0)
    /// * `center_freq` - NCO center frequency in Hz. Range: 0.0 to `sample_rate / 2`.
    /// * `decimation` - Integer decimation factor (≥ 1). Output rate = sample_rate / decimation.
    /// * `filter_order` - Number of FIR taps. Pass `0` for the automatic default
    ///   of `4 * decimation * 10 + 1` (approximately 60 dB stopband rejection).
    pub fn new(sample_rate: f32, center_freq: f32, decimation: usize, filter_order: usize) -> Self {
        assert!(decimation >= 1, "decimation must be >= 1");
        assert!(sample_rate > 0.0, "sample_rate must be positive");

        let num_taps = if filter_order == 0 {
            4 * decimation * 10 + 1
        } else {
            filter_order
        };

        // Cutoff is 0.4 / decimation (normalized): 40% of the decimated Nyquist.
        // This matches the convention used in resample::decimate but with a tighter
        // guard band to accommodate the complex baseband mixing products.
        let cutoff = 0.4 / decimation as f32;
        let filter_coeffs = design_lowpass(num_taps, cutoff);

        let phase_inc = 2.0 * PI * center_freq / sample_rate;

        Self {
            center_freq,
            sample_rate,
            decimation,
            phase: 0.0,
            phase_inc,
            filter_taps_i: vec![0.0; num_taps],
            filter_taps_q: vec![0.0; num_taps],
            filter_coeffs,
            filter_pos: 0,
            decimate_counter: 0,
        }
    }

    /// Process a block of real input samples and return complex baseband output.
    ///
    /// For each input sample:
    /// 1. Multiply by `cos(phase)` → I channel, `-sin(phase)` → Q channel.
    /// 2. Push I and Q into their respective circular delay-line buffers.
    /// 3. Apply the FIR filter via circular convolution.
    /// 4. Every `decimation` samples, emit one complex output sample.
    /// 5. Advance and wrap the NCO phase.
    ///
    /// # Arguments
    ///
    /// * `input` - Real input samples at the full sample rate.
    ///
    /// # Returns
    ///
    /// Complex baseband samples. Length = `input.len() / decimation`.
    /// The NCO phase and filter state are preserved across calls, enabling
    /// continuous streaming without phase discontinuities at block boundaries.
    pub fn process(&mut self, input: &[f32]) -> Vec<Complex<f32>> {
        let out_len = input.len() / self.decimation;
        let mut output = Vec::with_capacity(out_len);

        let num_taps = self.filter_coeffs.len();

        for &x in input {
            // Mix: down-convert to baseband
            let i_sample = x * self.phase.cos();
            let q_sample = x * (-self.phase.sin());

            // Push into circular delay-line buffers
            self.filter_taps_i[self.filter_pos] = i_sample;
            self.filter_taps_q[self.filter_pos] = q_sample;

            // Decimate gate: emit one output every `decimation` input samples
            self.decimate_counter += 1;
            if self.decimate_counter >= self.decimation {
                self.decimate_counter = 0;

                // Apply FIR filter via circular buffer convolution
                let mut i_out = 0.0f32;
                let mut q_out = 0.0f32;

                for (k, &coeff) in self.filter_coeffs.iter().enumerate() {
                    // Oldest sample is filter_pos + 1 (mod num_taps)
                    let tap_idx = (self.filter_pos + num_taps - k) % num_taps;
                    i_out += coeff * self.filter_taps_i[tap_idx];
                    q_out += coeff * self.filter_taps_q[tap_idx];
                }

                output.push(Complex::new(i_out, q_out));
            }

            // Advance NCO phase
            self.phase += self.phase_inc;

            // Wrap phase to [0, 2π) to prevent floating-point drift
            if self.phase >= 2.0 * PI {
                self.phase -= 2.0 * PI;
            }

            // Advance circular buffer write pointer
            self.filter_pos = (self.filter_pos + 1) % num_taps;
        }

        output
    }

    /// Set a new center frequency and reset the NCO phase to zero.
    ///
    /// The FIR filter state is preserved to avoid a transient, but the NCO
    /// phase is reset to prevent a phase discontinuity in the output.
    ///
    /// # Arguments
    ///
    /// * `freq` - New center frequency in Hz. Range: 0.0 to `sample_rate / 2`.
    pub fn set_center_freq(&mut self, freq: f32) {
        self.center_freq = freq;
        self.phase_inc = 2.0 * PI * freq / self.sample_rate;
        self.phase = 0.0;
    }

    /// Reset all internal state: NCO phase, filter delay lines, and decimation counter.
    ///
    /// After reset, the DDC behaves identically to a freshly constructed instance
    /// with the same parameters.
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.filter_pos = 0;
        self.decimate_counter = 0;
        for x in self.filter_taps_i.iter_mut() {
            *x = 0.0;
        }
        for x in self.filter_taps_q.iter_mut() {
            *x = 0.0;
        }
    }

    /// Return the configured center frequency in Hz.
    pub fn center_freq(&self) -> f32 {
        self.center_freq
    }

    /// Return the configured input sample rate in Hz.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Return the configured decimation factor.
    pub fn decimation(&self) -> usize {
        self.decimation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a real sine wave at `frequency` Hz sampled at `sample_rate` Hz.
    fn sine_wave(frequency: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
        (0..num_samples)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect()
    }

    /// Compute the mean power of the complex signal.
    fn mean_power(signal: &[Complex<f32>]) -> f32 {
        if signal.is_empty() {
            return 0.0;
        }
        signal.iter().map(|c| c.norm_sqr()).sum::<f32>() / signal.len() as f32
    }

    /// Estimate the power at a single baseband frequency via direct DFT.
    fn _power_at_freq(signal: &[Complex<f32>], freq_hz: f32, sample_rate: f32) -> f32 {
        let n = signal.len() as f32;
        let mut re = 0.0f32;
        let mut im = 0.0f32;
        for (i, c) in signal.iter().enumerate() {
            let phase = 2.0 * PI * freq_hz * i as f32 / sample_rate;
            re += c.re * phase.cos() + c.im * phase.sin();
            im += c.im * phase.cos() - c.re * phase.sin();
        }
        (re * re + im * im).sqrt() / n
    }

    #[test]
    fn test_ddc_decimation_length() {
        // Output length must equal input.len() / decimation.
        let sr = 48000.0;
        let n = 4800;
        let decimation = 8;
        let mut ddc = Ddc::new(sr, 1000.0, decimation, 0);
        let signal = sine_wave(1000.0, sr, n);
        let output = ddc.process(&signal);
        assert_eq!(
            output.len(),
            n / decimation,
            "Output length should be input / decimation"
        );
    }

    #[test]
    fn test_ddc_shifts_to_baseband() {
        // A 1 kHz tone shifted by DDC at 1 kHz should appear near DC (0 Hz offset).
        // In the baseband output (after shifting), a 1 kHz-at-1kHz-center tone becomes
        // a near-DC complex sinusoid. We verify the output has significant power.
        let sr = 48000.0;
        let fc = 1000.0;
        let n = 4800;
        let decimation = 8;
        let mut ddc = Ddc::new(sr, fc, decimation, 0);

        let signal = sine_wave(fc, sr, n);
        let output = ddc.process(&signal);

        // Skip initial transient (filter settling)
        let skip = output.len() / 4;
        let steady = &output[skip..];

        let power = mean_power(steady);
        assert!(
            power > 0.05,
            "Baseband output should have significant power for on-frequency tone, got {}",
            power
        );
    }

    #[test]
    fn test_ddc_rejects_out_of_band() {
        // Two tones: one at center frequency (fc), one far away (fc + 5000 Hz).
        // After DDC with center at fc, the out-of-band tone should be strongly rejected.
        let sr = 48000.0;
        let fc = 8000.0;
        let n = 48000; // 1 second for frequency resolution
        let decimation = 8;

        let mut ddc_on = Ddc::new(sr, fc, decimation, 0);
        let mut ddc_off = Ddc::new(sr, fc, decimation, 0);

        // On-frequency tone
        let on_freq = sine_wave(fc, sr, n);
        // Far off-frequency tone: 5 kHz away (well outside the lowpass passband)
        let off_freq = sine_wave(fc + 5000.0, sr, n);

        let out_on = ddc_on.process(&on_freq);
        let out_off = ddc_off.process(&off_freq);

        // Skip initial filter transient
        let skip = out_on.len() / 4;
        let power_on = mean_power(&out_on[skip..]);
        let power_off = mean_power(&out_off[skip..]);

        let rejection_db = 10.0 * (power_off / (power_on + 1e-12)).log10();
        assert!(
            rejection_db < -30.0,
            "Out-of-band rejection should be > 30 dB, got {:.1} dB",
            rejection_db
        );
    }

    #[test]
    fn test_ddc_streaming_continuity() {
        // Process a long signal in two halves. The output should match
        // processing it all at once (phase continuity across block boundary).
        let sr = 48000.0;
        let fc = 2000.0;
        let n = 4800;
        let decimation = 4;

        let signal = sine_wave(fc, sr, n);

        // All at once
        let mut ddc_whole = Ddc::new(sr, fc, decimation, 0);
        let out_whole = ddc_whole.process(&signal);

        // Two halves
        let mut ddc_split = Ddc::new(sr, fc, decimation, 0);
        let mut out_split = ddc_split.process(&signal[..n / 2]);
        out_split.extend(ddc_split.process(&signal[n / 2..]));

        assert_eq!(out_whole.len(), out_split.len());

        // Skip initial transient; check that outputs match within tolerance
        let skip = out_whole.len() / 4;
        let end = 3 * out_whole.len() / 4;
        for i in skip..end {
            let diff_re = (out_whole[i].re - out_split[i].re).abs();
            let diff_im = (out_whole[i].im - out_split[i].im).abs();
            assert!(
                diff_re < 1e-5 && diff_im < 1e-5,
                "Phase discontinuity at sample {}: whole={:?}, split={:?}",
                i,
                out_whole[i],
                out_split[i]
            );
        }
    }

    #[test]
    fn test_ddc_reset() {
        // After reset, processing the same block should give the same result as a fresh instance.
        let sr = 48000.0;
        let fc = 3000.0;
        let n = 960;
        let decimation = 4;

        let signal = sine_wave(fc, sr, n);

        // First run
        let mut ddc = Ddc::new(sr, fc, decimation, 0);
        let out1 = ddc.process(&signal);

        // Process some garbage to dirty the state
        let noise: Vec<f32> = (0..n).map(|_| 0.5f32).collect();
        let _ = ddc.process(&noise);

        // Reset and re-run
        ddc.reset();
        let out2 = ddc.process(&signal);

        assert_eq!(out1.len(), out2.len());
        for (i, (a, b)) in out1.iter().zip(out2.iter()).enumerate() {
            assert!(
                (a.re - b.re).abs() < 1e-5 && (a.im - b.im).abs() < 1e-5,
                "Post-reset mismatch at sample {}: {:?} vs {:?}",
                i,
                a,
                b
            );
        }
    }

    #[test]
    fn test_ddc_set_center_freq() {
        // set_center_freq should update the NCO and reset the phase.
        let sr = 48000.0;
        let n = 480;
        let decimation = 4;

        let mut ddc = Ddc::new(sr, 1000.0, decimation, 0);
        ddc.set_center_freq(2000.0);
        assert!((ddc.center_freq() - 2000.0).abs() < 1e-6);

        // A 2 kHz tone should now be on-frequency
        let signal = sine_wave(2000.0, sr, n);
        let output = ddc.process(&signal);
        assert_eq!(output.len(), n / decimation);

        let skip = output.len() / 4;
        let power = mean_power(&output[skip..]);
        assert!(
            power > 0.01,
            "Power after center freq change should be non-zero, got {}",
            power
        );
    }
}
