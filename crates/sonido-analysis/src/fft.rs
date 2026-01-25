//! FFT wrapper with windowing functions

use rustfft::{FftPlanner, num_complex::Complex};
use std::f32::consts::PI;
use std::sync::Arc;

/// Window function types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// Rectangular (no windowing)
    Rectangular,
    /// Hann window (raised cosine)
    Hann,
    /// Hamming window
    Hamming,
    /// Blackman window
    Blackman,
    /// Blackman-Harris window (better sidelobe suppression)
    BlackmanHarris,
}

impl Window {
    /// Apply window to a buffer
    pub fn apply(&self, buffer: &mut [f32]) {
        let n = buffer.len();
        match self {
            Window::Rectangular => {}
            Window::Hann => {
                for (i, sample) in buffer.iter_mut().enumerate() {
                    let w = 0.5 * (1.0 - (2.0 * PI * i as f32 / n as f32).cos());
                    *sample *= w;
                }
            }
            Window::Hamming => {
                for (i, sample) in buffer.iter_mut().enumerate() {
                    let w = 0.54 - 0.46 * (2.0 * PI * i as f32 / n as f32).cos();
                    *sample *= w;
                }
            }
            Window::Blackman => {
                for (i, sample) in buffer.iter_mut().enumerate() {
                    let x = 2.0 * PI * i as f32 / n as f32;
                    let w = 0.42 - 0.5 * x.cos() + 0.08 * (2.0 * x).cos();
                    *sample *= w;
                }
            }
            Window::BlackmanHarris => {
                for (i, sample) in buffer.iter_mut().enumerate() {
                    let x = 2.0 * PI * i as f32 / n as f32;
                    let w = 0.35875 - 0.48829 * x.cos() + 0.14128 * (2.0 * x).cos()
                        - 0.01168 * (3.0 * x).cos();
                    *sample *= w;
                }
            }
        }
    }

    /// Get window coefficients
    pub fn coefficients(&self, size: usize) -> Vec<f32> {
        let mut coeffs = vec![1.0; size];
        self.apply(&mut coeffs);
        coeffs
    }
}

/// FFT processor with caching
pub struct Fft {
    planner: FftPlanner<f32>,
    fft: Arc<dyn rustfft::Fft<f32>>,
    ifft: Arc<dyn rustfft::Fft<f32>>,
    size: usize,
}

impl Fft {
    /// Create a new FFT processor for the given size
    pub fn new(size: usize) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(size);
        let ifft = planner.plan_fft_inverse(size);

        Self {
            planner,
            fft,
            ifft,
            size,
        }
    }

    /// Get FFT size
    pub fn size(&self) -> usize {
        self.size
    }

    /// Resize the FFT (creates new plan if needed)
    pub fn resize(&mut self, size: usize) {
        if size != self.size {
            self.fft = self.planner.plan_fft_forward(size);
            self.ifft = self.planner.plan_fft_inverse(size);
            self.size = size;
        }
    }

    /// Perform forward FFT on real input
    ///
    /// Returns complex spectrum (size/2 + 1 bins for positive frequencies)
    pub fn forward(&self, input: &[f32]) -> Vec<Complex<f32>> {
        let mut buffer: Vec<Complex<f32>> = input
            .iter()
            .map(|&x| Complex::new(x, 0.0))
            .collect();

        // Pad or truncate to FFT size
        buffer.resize(self.size, Complex::new(0.0, 0.0));

        self.fft.process(&mut buffer);

        // Return only positive frequencies (DC to Nyquist)
        buffer.truncate(self.size / 2 + 1);
        buffer
    }

    /// Perform forward FFT on complex input (in-place)
    pub fn forward_complex(&self, buffer: &mut [Complex<f32>]) {
        self.fft.process(buffer);
    }

    /// Perform inverse FFT
    ///
    /// Takes full spectrum and returns real signal
    pub fn inverse(&self, spectrum: &[Complex<f32>]) -> Vec<f32> {
        // Reconstruct full spectrum (mirror conjugate)
        let mut buffer = Vec::with_capacity(self.size);
        buffer.extend_from_slice(spectrum);

        // Mirror for negative frequencies (conjugate symmetry)
        for i in 1..self.size - spectrum.len() + 1 {
            let idx = spectrum.len() - 1 - i;
            if idx > 0 && idx < spectrum.len() {
                buffer.push(spectrum[idx].conj());
            }
        }

        buffer.resize(self.size, Complex::new(0.0, 0.0));

        self.ifft.process(&mut buffer);

        // Normalize and extract real part
        let scale = 1.0 / self.size as f32;
        buffer.iter().map(|c| c.re * scale).collect()
    }

    /// Perform inverse FFT on complex buffer (in-place)
    pub fn inverse_complex(&self, buffer: &mut [Complex<f32>]) {
        self.ifft.process(buffer);

        // Normalize
        let scale = 1.0 / self.size as f32;
        for c in buffer.iter_mut() {
            *c *= scale;
        }
    }
}

/// Compute magnitude spectrum in dB
pub fn magnitude_db(spectrum: &[Complex<f32>]) -> Vec<f32> {
    spectrum
        .iter()
        .map(|c| {
            let mag = c.norm();
            20.0 * (mag.max(1e-10)).log10()
        })
        .collect()
}

/// Compute phase spectrum in radians
pub fn phase_rad(spectrum: &[Complex<f32>]) -> Vec<f32> {
    spectrum.iter().map(|c| c.arg()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_roundtrip() {
        let fft = Fft::new(256);

        // Create test signal
        let input: Vec<f32> = (0..256)
            .map(|i| (2.0 * PI * 10.0 * i as f32 / 256.0).sin())
            .collect();

        let spectrum = fft.forward(&input);
        let reconstructed = fft.inverse(&spectrum);

        // Check reconstruction
        for (a, b) in input.iter().zip(reconstructed.iter()) {
            assert!((a - b).abs() < 0.01, "Mismatch: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_window_hann() {
        let mut buffer = vec![1.0; 100];
        Window::Hann.apply(&mut buffer);

        // Hann window should be 0 at edges, 1 at center
        assert!(buffer[0] < 0.01);
        assert!(buffer[99] < 0.01);
        assert!((buffer[50] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_dc_detection() {
        let fft = Fft::new(256);

        // DC signal
        let input = vec![1.0; 256];
        let spectrum = fft.forward(&input);

        // DC bin should be large, others small
        let dc_mag = spectrum[0].norm();
        let other_mag: f32 = spectrum[1..].iter().map(|c| c.norm()).sum();

        assert!(dc_mag > other_mag * 10.0);
    }
}
