//! STFT-based spectrogram generation
//!
//! Provides time-frequency analysis through Short-Time Fourier Transform,
//! useful for visualizing how spectral content changes over time.

use crate::fft::{Fft, Window};

/// Spectrogram data structure
#[derive(Debug, Clone)]
pub struct Spectrogram {
    /// 2D magnitude data `[time_frame][frequency_bin]`
    pub data: Vec<Vec<f32>>,
    /// FFT size used
    pub fft_size: usize,
    /// Hop size between frames
    pub hop_size: usize,
    /// Sample rate
    pub sample_rate: f32,
    /// Number of time frames
    pub num_frames: usize,
    /// Number of frequency bins (fft_size / 2 + 1)
    pub num_bins: usize,
}

impl Spectrogram {
    /// Get frequency in Hz for a given bin index
    pub fn bin_to_freq(&self, bin: usize) -> f32 {
        bin as f32 * self.sample_rate / self.fft_size as f32
    }

    /// Get time in seconds for a given frame index
    pub fn frame_to_time(&self, frame: usize) -> f32 {
        frame as f32 * self.hop_size as f32 / self.sample_rate
    }

    /// Get duration in seconds
    pub fn duration(&self) -> f32 {
        self.frame_to_time(self.num_frames)
    }

    /// Get maximum frequency (Nyquist)
    pub fn max_frequency(&self) -> f32 {
        self.sample_rate / 2.0
    }

    /// Get magnitude at specific time and frequency
    ///
    /// Returns None if out of bounds
    pub fn get(&self, frame: usize, bin: usize) -> Option<f32> {
        self.data.get(frame).and_then(|f| f.get(bin)).copied()
    }

    /// Get magnitude in dB at specific time and frequency
    pub fn get_db(&self, frame: usize, bin: usize) -> Option<f32> {
        self.get(frame, bin).map(|m| 20.0 * m.max(1e-10).log10())
    }

    /// Get the spectrum for a specific time frame
    pub fn get_frame(&self, frame: usize) -> Option<&[f32]> {
        self.data.get(frame).map(|v| v.as_slice())
    }

    /// Get magnitude values across time for a specific frequency bin
    pub fn get_bin_over_time(&self, bin: usize) -> Vec<f32> {
        self.data
            .iter()
            .filter_map(|frame| frame.get(bin).copied())
            .collect()
    }

    /// Find peak frequency at a given time frame
    pub fn peak_frequency(&self, frame: usize) -> Option<f32> {
        let spectrum = self.get_frame(frame)?;
        let (peak_bin, _) = spectrum
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())?;
        Some(self.bin_to_freq(peak_bin))
    }

    /// Compute spectral centroid for each frame
    pub fn spectral_centroid(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|frame| {
                let mut weighted_sum = 0.0;
                let mut magnitude_sum = 0.0;

                for (i, &mag) in frame.iter().enumerate() {
                    let freq = self.bin_to_freq(i);
                    weighted_sum += freq * mag;
                    magnitude_sum += mag;
                }

                if magnitude_sum > 1e-10 {
                    weighted_sum / magnitude_sum
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Convert to dB scale (in-place modification returns new Spectrogram)
    pub fn to_db(&self) -> Spectrogram {
        let data = self
            .data
            .iter()
            .map(|frame| frame.iter().map(|&m| 20.0 * m.max(1e-10).log10()).collect())
            .collect();

        Spectrogram {
            data,
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            sample_rate: self.sample_rate,
            num_frames: self.num_frames,
            num_bins: self.num_bins,
        }
    }

    /// Normalize to 0-1 range
    pub fn normalize(&self) -> Spectrogram {
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;

        for frame in &self.data {
            for &val in frame {
                min_val = min_val.min(val);
                max_val = max_val.max(val);
            }
        }

        let range = max_val - min_val;
        if range < 1e-10 {
            return self.clone();
        }

        let data = self
            .data
            .iter()
            .map(|frame| frame.iter().map(|&v| (v - min_val) / range).collect())
            .collect();

        Spectrogram {
            data,
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            sample_rate: self.sample_rate,
            num_frames: self.num_frames,
            num_bins: self.num_bins,
        }
    }
}

/// STFT (Short-Time Fourier Transform) analyzer
pub struct StftAnalyzer {
    fft_size: usize,
    hop_size: usize,
    window: Window,
    sample_rate: f32,
    fft: Fft,
    window_coeffs: Vec<f32>,
}

impl StftAnalyzer {
    /// Create a new STFT analyzer
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz
    /// * `fft_size` - FFT size (power of 2 recommended)
    /// * `hop_size` - Hop size between frames (typically fft_size / 4)
    /// * `window` - Window function to use
    pub fn new(sample_rate: f32, fft_size: usize, hop_size: usize, window: Window) -> Self {
        let fft = Fft::new(fft_size);
        let window_coeffs = window.coefficients(fft_size);

        Self {
            fft_size,
            hop_size,
            window,
            sample_rate,
            fft,
            window_coeffs,
        }
    }

    /// Create analyzer with default settings (50% overlap, Hann window)
    pub fn default_for_sample_rate(sample_rate: f32, fft_size: usize) -> Self {
        Self::new(sample_rate, fft_size, fft_size / 2, Window::Hann)
    }

    /// Compute spectrogram from audio signal
    pub fn analyze(&self, signal: &[f32]) -> Spectrogram {
        let num_frames = if signal.len() >= self.fft_size {
            (signal.len() - self.fft_size) / self.hop_size + 1
        } else {
            0
        };

        let num_bins = self.fft_size / 2 + 1;
        let mut data = Vec::with_capacity(num_frames);

        for frame_idx in 0..num_frames {
            let start = frame_idx * self.hop_size;
            let end = start + self.fft_size;

            // Extract and window frame
            let mut frame: Vec<f32> = signal[start..end.min(signal.len())].to_vec();
            frame.resize(self.fft_size, 0.0);

            // Apply window
            for (sample, &coeff) in frame.iter_mut().zip(self.window_coeffs.iter()) {
                *sample *= coeff;
            }

            // FFT
            let spectrum = self.fft.forward(&frame);

            // Extract magnitudes
            let magnitudes: Vec<f32> = spectrum.iter().map(|c| c.norm()).collect();
            data.push(magnitudes);
        }

        Spectrogram {
            data,
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            sample_rate: self.sample_rate,
            num_frames,
            num_bins,
        }
    }

    /// Compute spectrogram with phase information
    pub fn analyze_complex(&self, signal: &[f32]) -> (Spectrogram, Vec<Vec<f32>>) {
        let num_frames = if signal.len() >= self.fft_size {
            (signal.len() - self.fft_size) / self.hop_size + 1
        } else {
            0
        };

        let num_bins = self.fft_size / 2 + 1;
        let mut magnitude_data = Vec::with_capacity(num_frames);
        let mut phase_data = Vec::with_capacity(num_frames);

        for frame_idx in 0..num_frames {
            let start = frame_idx * self.hop_size;
            let end = start + self.fft_size;

            let mut frame: Vec<f32> = signal[start..end.min(signal.len())].to_vec();
            frame.resize(self.fft_size, 0.0);

            for (sample, &coeff) in frame.iter_mut().zip(self.window_coeffs.iter()) {
                *sample *= coeff;
            }

            let spectrum = self.fft.forward(&frame);

            let magnitudes: Vec<f32> = spectrum.iter().map(|c| c.norm()).collect();
            let phases: Vec<f32> = spectrum.iter().map(|c| c.arg()).collect();

            magnitude_data.push(magnitudes);
            phase_data.push(phases);
        }

        let spectrogram = Spectrogram {
            data: magnitude_data,
            fft_size: self.fft_size,
            hop_size: self.hop_size,
            sample_rate: self.sample_rate,
            num_frames,
            num_bins,
        };

        (spectrogram, phase_data)
    }

    /// Get FFT size
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    /// Get hop size
    pub fn hop_size(&self) -> usize {
        self.hop_size
    }

    /// Get frequency resolution (Hz per bin)
    pub fn frequency_resolution(&self) -> f32 {
        self.sample_rate / self.fft_size as f32
    }

    /// Get time resolution (seconds per frame)
    pub fn time_resolution(&self) -> f32 {
        self.hop_size as f32 / self.sample_rate
    }

    /// Get the window function used
    pub fn window(&self) -> Window {
        self.window
    }
}

/// Mel-scaled spectrogram
pub struct MelSpectrogram {
    /// Mel-scaled magnitude data `[time_frame][mel_bin]`
    pub data: Vec<Vec<f32>>,
    /// Number of mel bins
    pub num_mel_bins: usize,
    /// Minimum frequency (Hz)
    pub min_freq: f32,
    /// Maximum frequency (Hz)
    pub max_freq: f32,
    /// Sample rate
    pub sample_rate: f32,
    /// Hop size
    pub hop_size: usize,
    /// Number of frames
    pub num_frames: usize,
}

/// Mel filterbank for converting linear spectrogram to mel scale
pub struct MelFilterbank {
    filters: Vec<Vec<f32>>,
    num_mel_bins: usize,
    num_fft_bins: usize,
}

impl MelFilterbank {
    /// Create a mel filterbank
    ///
    /// # Arguments
    /// * `num_fft_bins` - Number of FFT bins (fft_size / 2 + 1)
    /// * `num_mel_bins` - Number of mel bins
    /// * `sample_rate` - Sample rate in Hz
    /// * `min_freq` - Minimum frequency (Hz)
    /// * `max_freq` - Maximum frequency (Hz)
    pub fn new(
        num_fft_bins: usize,
        num_mel_bins: usize,
        sample_rate: f32,
        min_freq: f32,
        max_freq: f32,
    ) -> Self {
        let fft_size = (num_fft_bins - 1) * 2;

        // Convert Hz to Mel
        let mel_min = Self::hz_to_mel(min_freq);
        let mel_max = Self::hz_to_mel(max_freq);

        // Create equally spaced mel points
        let mel_points: Vec<f32> = (0..=num_mel_bins + 1)
            .map(|i| mel_min + (mel_max - mel_min) * i as f32 / (num_mel_bins + 1) as f32)
            .collect();

        // Convert back to Hz
        let hz_points: Vec<f32> = mel_points.iter().map(|&m| Self::mel_to_hz(m)).collect();

        // Convert to FFT bin indices
        let bin_points: Vec<usize> = hz_points
            .iter()
            .map(|&f| ((fft_size as f32 + 1.0) * f / sample_rate).floor() as usize)
            .collect();

        // Create triangular filters
        let mut filters = vec![vec![0.0; num_fft_bins]; num_mel_bins];

        for m in 0..num_mel_bins {
            let left = bin_points[m];
            let center = bin_points[m + 1];
            let right = bin_points[m + 2];

            // Rising edge
            if center > left {
                for (k, val) in filters[m]
                    .iter_mut()
                    .enumerate()
                    .take(center.min(num_fft_bins))
                    .skip(left)
                {
                    *val = (k - left) as f32 / (center - left) as f32;
                }
            }

            // Falling edge
            if right > center {
                for (k, val) in filters[m]
                    .iter_mut()
                    .enumerate()
                    .take(right.min(num_fft_bins))
                    .skip(center)
                {
                    *val = (right - k) as f32 / (right - center) as f32;
                }
            }
        }

        Self {
            filters,
            num_mel_bins,
            num_fft_bins,
        }
    }

    /// Convert Hz to Mel scale
    fn hz_to_mel(hz: f32) -> f32 {
        2595.0 * (1.0 + hz / 700.0).log10()
    }

    /// Convert Mel to Hz
    fn mel_to_hz(mel: f32) -> f32 {
        700.0 * (10.0_f32.powf(mel / 2595.0) - 1.0)
    }

    /// Apply filterbank to linear spectrum
    pub fn apply(&self, spectrum: &[f32]) -> Vec<f32> {
        self.filters
            .iter()
            .map(|filter| {
                filter
                    .iter()
                    .zip(spectrum.iter())
                    .map(|(&f, &s)| f * s)
                    .sum()
            })
            .collect()
    }

    /// Convert entire spectrogram to mel scale
    pub fn apply_to_spectrogram(&self, spectrogram: &Spectrogram) -> MelSpectrogram {
        let data: Vec<Vec<f32>> = spectrogram
            .data
            .iter()
            .map(|frame| self.apply(frame))
            .collect();

        MelSpectrogram {
            data,
            num_mel_bins: self.num_mel_bins,
            min_freq: Self::mel_to_hz(Self::hz_to_mel(0.0)),
            max_freq: spectrogram.sample_rate / 2.0,
            sample_rate: spectrogram.sample_rate,
            hop_size: spectrogram.hop_size,
            num_frames: spectrogram.num_frames,
        }
    }

    /// Get number of mel bins
    pub fn num_mel_bins(&self) -> usize {
        self.num_mel_bins
    }

    /// Get number of FFT bins
    pub fn num_fft_bins(&self) -> usize {
        self.num_fft_bins
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn generate_sine(sample_rate: f32, freq: f32, duration_secs: f32) -> Vec<f32> {
        let num_samples = (duration_secs * sample_rate) as usize;
        (0..num_samples)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn test_spectrogram_dimensions() {
        let sample_rate = 44100.0;
        let signal = generate_sine(sample_rate, 440.0, 1.0);

        let analyzer = StftAnalyzer::new(sample_rate, 1024, 512, Window::Hann);
        let spectrogram = analyzer.analyze(&signal);

        // Check dimensions
        assert_eq!(spectrogram.num_bins, 513); // 1024/2 + 1
        assert!(spectrogram.num_frames > 0);
        assert_eq!(spectrogram.data.len(), spectrogram.num_frames);
        assert_eq!(spectrogram.data[0].len(), spectrogram.num_bins);
    }

    #[test]
    fn test_spectrogram_peak_detection() {
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let signal = generate_sine(sample_rate, freq, 0.5);

        let analyzer = StftAnalyzer::new(sample_rate, 2048, 1024, Window::Hann);
        let spectrogram = analyzer.analyze(&signal);

        // Peak should be near 1000 Hz for all frames
        for frame in 0..spectrogram.num_frames {
            let peak_freq = spectrogram.peak_frequency(frame).unwrap();
            assert!(
                (peak_freq - freq).abs() < 50.0,
                "Peak {} Hz should be near {} Hz",
                peak_freq,
                freq
            );
        }
    }

    #[test]
    fn test_spectrogram_time_frequency_conversion() {
        let sample_rate = 48000.0;
        let _analyzer = StftAnalyzer::new(sample_rate, 1024, 256, Window::Hann);

        let spectrogram = Spectrogram {
            data: vec![],
            fft_size: 1024,
            hop_size: 256,
            sample_rate,
            num_frames: 100,
            num_bins: 513,
        };

        // Test frequency conversion
        assert!((spectrogram.bin_to_freq(0) - 0.0).abs() < 0.01);
        assert!((spectrogram.bin_to_freq(512) - 24000.0).abs() < 1.0);

        // Test time conversion
        assert!((spectrogram.frame_to_time(0) - 0.0).abs() < 0.001);
        let expected_time = 256.0 / 48000.0;
        assert!((spectrogram.frame_to_time(1) - expected_time).abs() < 0.001);
    }

    #[test]
    fn test_mel_filterbank() {
        let filterbank = MelFilterbank::new(513, 40, 44100.0, 20.0, 8000.0);

        // Test with flat spectrum
        let spectrum = vec![1.0; 513];
        let mel = filterbank.apply(&spectrum);

        assert_eq!(mel.len(), 40);
        // All bins should have non-zero output
        assert!(mel.iter().all(|&v| v > 0.0));
    }

    #[test]
    fn test_spectrogram_to_db() {
        let sample_rate = 44100.0;
        let signal = generate_sine(sample_rate, 440.0, 0.5);

        let analyzer = StftAnalyzer::new(sample_rate, 1024, 512, Window::Hann);
        let spectrogram = analyzer.analyze(&signal);
        let spectrogram_db = spectrogram.to_db();

        // dB values should be negative for magnitudes < 1
        // and the peak should be around 0 dB for amplitude 1 signal
        assert!(spectrogram_db.data[0].iter().any(|&v| v < 0.0));
    }

    #[test]
    fn test_spectral_centroid() {
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let signal = generate_sine(sample_rate, freq, 0.5);

        let analyzer = StftAnalyzer::new(sample_rate, 2048, 1024, Window::Hann);
        let spectrogram = analyzer.analyze(&signal);
        let centroids = spectrogram.spectral_centroid();

        // Centroid should be near the fundamental
        for centroid in centroids {
            assert!(
                (centroid - freq).abs() < 100.0,
                "Centroid {} should be near {} Hz",
                centroid,
                freq
            );
        }
    }

    #[test]
    fn test_frequency_resolution() {
        let sample_rate = 48000.0;
        let analyzer = StftAnalyzer::new(sample_rate, 2048, 512, Window::Hann);

        let expected_freq_res = 48000.0 / 2048.0;
        assert!((analyzer.frequency_resolution() - expected_freq_res).abs() < 0.01);

        let expected_time_res = 512.0 / 48000.0;
        assert!((analyzer.time_resolution() - expected_time_res).abs() < 0.0001);
    }
}
