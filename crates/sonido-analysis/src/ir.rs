//! Impulse response capture via exponential sine sweep

use std::f32::consts::PI;

/// Exponential sine sweep generator for IR capture
///
/// Uses the Farina method for deconvolution-based impulse response measurement.
pub struct SineSweep {
    sample_rate: f32,
    start_freq: f32,
    end_freq: f32,
    duration_secs: f32,
}

impl SineSweep {
    /// Create a new sine sweep generator
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz
    /// * `start_freq` - Start frequency in Hz
    /// * `end_freq` - End frequency in Hz
    /// * `duration_secs` - Sweep duration in seconds
    pub fn new(sample_rate: f32, start_freq: f32, end_freq: f32, duration_secs: f32) -> Self {
        Self {
            sample_rate,
            start_freq,
            end_freq,
            duration_secs,
        }
    }

    /// Generate the exponential sine sweep
    pub fn generate(&self) -> Vec<f32> {
        let num_samples = (self.duration_secs * self.sample_rate) as usize;
        let k = (self.end_freq / self.start_freq).ln();

        (0..num_samples)
            .map(|i| {
                let t = i as f32 / self.sample_rate;
                let phase = 2.0 * PI * self.start_freq * self.duration_secs / k
                    * ((k * t / self.duration_secs).exp() - 1.0);
                phase.sin()
            })
            .collect()
    }

    /// Generate the inverse filter for deconvolution
    pub fn inverse_filter(&self) -> Vec<f32> {
        let sweep = self.generate();
        let k = (self.end_freq / self.start_freq).ln();

        // Time-reverse and apply amplitude envelope
        sweep
            .into_iter()
            .rev()
            .enumerate()
            .map(|(i, sample)| {
                let t = i as f32 / self.sample_rate;
                // Amplitude envelope compensates for exponential frequency increase
                let amplitude = (-k * t / self.duration_secs).exp();
                sample * amplitude
            })
            .collect()
    }

    /// Compute impulse response from recorded sweep response
    ///
    /// # Arguments
    /// * `response` - Recorded sweep through the system under test
    ///
    /// # Returns
    /// Impulse response of the system
    pub fn compute_ir(&self, response: &[f32]) -> Vec<f32> {
        use crate::fft::Fft;
        use rustfft::num_complex::Complex;

        let inverse = self.inverse_filter();

        // Pad to power of 2 for FFT
        let fft_size = (response.len() + inverse.len() - 1).next_power_of_two();
        let fft = Fft::new(fft_size);

        // Convert to complex and pad
        let mut response_complex: Vec<Complex<f32>> = response
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        response_complex.resize(fft_size, Complex::new(0.0, 0.0));

        let mut inverse_complex: Vec<Complex<f32>> = inverse
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();
        inverse_complex.resize(fft_size, Complex::new(0.0, 0.0));

        // FFT both
        fft.forward_complex(&mut response_complex);
        fft.forward_complex(&mut inverse_complex);

        // Multiply in frequency domain
        for (r, i) in response_complex.iter_mut().zip(inverse_complex.iter()) {
            *r *= *i;
        }

        // IFFT
        fft.inverse_complex(&mut response_complex);

        // Extract real part
        response_complex.iter().map(|c| c.re).collect()
    }

    /// Get sweep duration in seconds
    pub fn duration(&self) -> f32 {
        self.duration_secs
    }

    /// Get number of samples
    pub fn num_samples(&self) -> usize {
        (self.duration_secs * self.sample_rate) as usize
    }
}

/// Generate a simple impulse signal
pub fn impulse(length: usize) -> Vec<f32> {
    let mut signal = vec![0.0; length];
    if !signal.is_empty() {
        signal[0] = 1.0;
    }
    signal
}

/// Generate white noise for testing
pub fn white_noise(length: usize, amplitude: f32) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    (0..length)
        .map(|i| {
            // Simple PRNG using hash
            let mut hasher = DefaultHasher::new();
            i.hash(&mut hasher);
            let hash = hasher.finish();
            let random = (hash as f32 / u64::MAX as f32) * 2.0 - 1.0;
            random * amplitude
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_sweep_generation() {
        let sweep = SineSweep::new(48000.0, 20.0, 20000.0, 1.0);
        let signal = sweep.generate();

        assert_eq!(signal.len(), 48000);

        // Should be bounded
        assert!(signal.iter().all(|&x| x.abs() <= 1.0));
    }

    #[test]
    fn test_inverse_filter_length() {
        let sweep = SineSweep::new(48000.0, 20.0, 20000.0, 1.0);
        let inverse = sweep.inverse_filter();

        assert_eq!(inverse.len(), sweep.num_samples());
    }

    #[test]
    fn test_impulse() {
        let imp = impulse(100);
        assert_eq!(imp[0], 1.0);
        assert!(imp[1..].iter().all(|&x| x == 0.0));
    }
}
