//! Transfer function measurement

use crate::fft::{Fft, Window};
use rustfft::num_complex::Complex;

/// Transfer function measurement result
pub struct TransferFunction {
    /// Frequency bins (Hz)
    pub frequencies: Vec<f32>,
    /// Magnitude response (dB)
    pub magnitude_db: Vec<f32>,
    /// Phase response (radians)
    pub phase_rad: Vec<f32>,
    /// Coherence (0-1, measure of linearity)
    pub coherence: Vec<f32>,
}

impl TransferFunction {
    /// Measure transfer function using cross-spectral method
    ///
    /// # Arguments
    /// * `input` - Input signal (reference)
    /// * `output` - Output signal (system response)
    /// * `sample_rate` - Sample rate in Hz
    /// * `fft_size` - FFT size (determines frequency resolution)
    /// * `overlap` - Overlap ratio (0.0 to 0.9, typical 0.5)
    pub fn measure(
        input: &[f32],
        output: &[f32],
        sample_rate: f32,
        fft_size: usize,
        overlap: f32,
    ) -> Self {
        let hop_size = ((1.0 - overlap) * fft_size as f32) as usize;
        let num_frames = (input.len().min(output.len()) - fft_size) / hop_size + 1;

        let fft = Fft::new(fft_size);
        let window = Window::Hann.coefficients(fft_size);

        // Accumulate cross-spectral density and power spectral densities
        let spectrum_size = fft_size / 2 + 1;
        let mut pxx = vec![0.0f32; spectrum_size]; // Input PSD
        let mut pyy = vec![0.0f32; spectrum_size]; // Output PSD
        let mut pxy = vec![Complex::new(0.0, 0.0); spectrum_size]; // Cross PSD

        for frame_idx in 0..num_frames {
            let start = frame_idx * hop_size;

            // Window and FFT input
            let mut x_windowed: Vec<Complex<f32>> = input[start..start + fft_size]
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();
            fft.forward_complex(&mut x_windowed);

            // Window and FFT output
            let mut y_windowed: Vec<Complex<f32>> = output[start..start + fft_size]
                .iter()
                .zip(window.iter())
                .map(|(&s, &w)| Complex::new(s * w, 0.0))
                .collect();
            fft.forward_complex(&mut y_windowed);

            // Accumulate spectra
            for i in 0..spectrum_size {
                let x = x_windowed[i];
                let y = y_windowed[i];

                pxx[i] += x.norm_sqr();
                pyy[i] += y.norm_sqr();
                pxy[i] += y * x.conj();
            }
        }

        // Compute transfer function H = Pxy / Pxx
        let mut magnitude_db = Vec::with_capacity(spectrum_size);
        let mut phase_rad = Vec::with_capacity(spectrum_size);
        let mut coherence = Vec::with_capacity(spectrum_size);
        let mut frequencies = Vec::with_capacity(spectrum_size);

        let freq_resolution = sample_rate / fft_size as f32;

        for i in 0..spectrum_size {
            frequencies.push(i as f32 * freq_resolution);

            if pxx[i] > 1e-10 {
                let h = pxy[i] / pxx[i];
                magnitude_db.push(20.0 * h.norm().log10());
                phase_rad.push(h.arg());

                // Coherence = |Pxy|^2 / (Pxx * Pyy)
                let coh = pxy[i].norm_sqr() / (pxx[i] * pyy[i]).max(1e-10);
                coherence.push(coh.min(1.0));
            } else {
                magnitude_db.push(-120.0);
                phase_rad.push(0.0);
                coherence.push(0.0);
            }
        }

        Self {
            frequencies,
            magnitude_db,
            phase_rad,
            coherence,
        }
    }

    /// Get magnitude at a specific frequency (interpolated)
    pub fn magnitude_at(&self, freq_hz: f32) -> f32 {
        interpolate(&self.frequencies, &self.magnitude_db, freq_hz)
    }

    /// Get phase at a specific frequency (interpolated)
    pub fn phase_at(&self, freq_hz: f32) -> f32 {
        interpolate(&self.frequencies, &self.phase_rad, freq_hz)
    }

    /// Find -3dB cutoff frequency
    pub fn cutoff_frequency(&self, reference_db: f32) -> Option<f32> {
        let target = reference_db - 3.0;

        for i in 1..self.magnitude_db.len() {
            if self.magnitude_db[i] < target && self.magnitude_db[i - 1] >= target {
                // Linear interpolation
                let t = (target - self.magnitude_db[i - 1])
                    / (self.magnitude_db[i] - self.magnitude_db[i - 1]);
                return Some(self.frequencies[i - 1] + t * (self.frequencies[i] - self.frequencies[i - 1]));
            }
        }
        None
    }
}

/// Linear interpolation helper
fn interpolate(x: &[f32], y: &[f32], target_x: f32) -> f32 {
    if x.is_empty() {
        return 0.0;
    }

    if target_x <= x[0] {
        return y[0];
    }

    for i in 1..x.len() {
        if target_x <= x[i] {
            let t = (target_x - x[i - 1]) / (x[i] - x[i - 1]);
            return y[i - 1] + t * (y[i] - y[i - 1]);
        }
    }

    *y.last().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_transfer_function_passthrough() {
        let sample_rate = 48000.0;
        let duration = 1.0;
        let num_samples = (sample_rate * duration) as usize;

        // Generate test signal
        let input: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * PI * 1000.0 * t).sin() + (2.0 * PI * 5000.0 * t).sin() * 0.5
            })
            .collect();

        // Passthrough system
        let output = input.clone();

        let tf = TransferFunction::measure(&input, &output, sample_rate, 4096, 0.5);

        // Magnitude should be ~0 dB across frequencies
        let avg_mag: f32 = tf.magnitude_db.iter().sum::<f32>() / tf.magnitude_db.len() as f32;
        assert!(
            avg_mag.abs() < 6.0,
            "Passthrough should have ~0dB magnitude, got {}",
            avg_mag
        );
    }

    #[test]
    fn test_interpolate() {
        let x = vec![0.0, 1.0, 2.0];
        let y = vec![0.0, 10.0, 20.0];

        assert_eq!(interpolate(&x, &y, 0.5), 5.0);
        assert_eq!(interpolate(&x, &y, 1.5), 15.0);
    }
}
