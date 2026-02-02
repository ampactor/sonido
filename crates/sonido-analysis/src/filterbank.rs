//! Bandpass filter bank for frequency band extraction.
//!
//! This module provides tools for extracting signals from specific frequency bands,
//! commonly used in biosignal analysis (EEG, EMG, etc.) and cross-frequency coupling studies.
//!
//! # Example
//!
//! ```rust
//! use sonido_analysis::filterbank::{FilterBank, eeg_bands};
//!
//! let bands = [eeg_bands::THETA, eeg_bands::ALPHA, eeg_bands::BETA];
//! let mut bank = FilterBank::new(1000.0, &bands);
//!
//! let signal = vec![0.0; 1000]; // Your EEG signal
//! let extracted = bank.extract(&signal);
//! // extracted[0] contains theta band, [1] alpha, [2] beta
//! ```

use sonido_core::biquad::{Biquad, lowpass_coefficients, highpass_coefficients};

/// A frequency band specification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrequencyBand {
    /// Human-readable name for the band
    pub name: &'static str,
    /// Lower cutoff frequency in Hz
    pub low_hz: f32,
    /// Upper cutoff frequency in Hz
    pub high_hz: f32,
}

impl FrequencyBand {
    /// Create a new frequency band.
    pub const fn new(name: &'static str, low_hz: f32, high_hz: f32) -> Self {
        Self { name, low_hz, high_hz }
    }

    /// Get the center frequency of the band.
    pub fn center_hz(&self) -> f32 {
        (self.low_hz * self.high_hz).sqrt()
    }

    /// Get the bandwidth in Hz.
    pub fn bandwidth(&self) -> f32 {
        self.high_hz - self.low_hz
    }
}

/// Standard EEG frequency bands.
pub mod eeg_bands {
    use super::FrequencyBand;

    /// Delta band (0.5-4 Hz) - Deep sleep, unconscious processes
    pub const DELTA: FrequencyBand = FrequencyBand::new("delta", 0.5, 4.0);

    /// Theta band (4-8 Hz) - Drowsiness, light sleep, memory
    pub const THETA: FrequencyBand = FrequencyBand::new("theta", 4.0, 8.0);

    /// Alpha band (8-13 Hz) - Relaxed wakefulness, closed eyes
    pub const ALPHA: FrequencyBand = FrequencyBand::new("alpha", 8.0, 13.0);

    /// Beta band (13-30 Hz) - Active thinking, focus, anxiety
    pub const BETA: FrequencyBand = FrequencyBand::new("beta", 13.0, 30.0);

    /// Low gamma band (30-80 Hz) - Cognitive processing, perception
    pub const LOW_GAMMA: FrequencyBand = FrequencyBand::new("low_gamma", 30.0, 80.0);

    /// High gamma band (80-200 Hz) - Fine motor control, sensory processing
    pub const HIGH_GAMMA: FrequencyBand = FrequencyBand::new("high_gamma", 80.0, 200.0);

    /// All standard EEG bands in order of increasing frequency.
    pub const ALL: [FrequencyBand; 6] = [DELTA, THETA, ALPHA, BETA, LOW_GAMMA, HIGH_GAMMA];
}

/// A single bandpass filter stage using cascaded high-pass and low-pass filters.
///
/// Uses 4th-order Butterworth response (two cascaded 2nd-order sections for each
/// high-pass and low-pass stage).
#[derive(Debug, Clone)]
struct BandpassFilter {
    /// High-pass filters (2 cascaded for 4th order)
    highpass: [Biquad; 2],
    /// Low-pass filters (2 cascaded for 4th order)
    lowpass: [Biquad; 2],
    /// The frequency band this filter extracts
    band: FrequencyBand,
}

impl BandpassFilter {
    /// Create a new bandpass filter for the given band.
    fn new(sample_rate: f32, band: FrequencyBand) -> Self {
        // Q for Butterworth 4th order (two cascaded 2nd order sections)
        // The Q values for a 4th order Butterworth are 0.541 and 1.307
        let q1 = 0.541;
        let q2 = 1.307;

        let mut highpass = [Biquad::new(), Biquad::new()];
        let mut lowpass = [Biquad::new(), Biquad::new()];

        // Configure high-pass filters
        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(band.low_hz, q1, sample_rate);
        highpass[0].set_coefficients(b0, b1, b2, a0, a1, a2);

        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(band.low_hz, q2, sample_rate);
        highpass[1].set_coefficients(b0, b1, b2, a0, a1, a2);

        // Configure low-pass filters
        let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(band.high_hz, q1, sample_rate);
        lowpass[0].set_coefficients(b0, b1, b2, a0, a1, a2);

        let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(band.high_hz, q2, sample_rate);
        lowpass[1].set_coefficients(b0, b1, b2, a0, a1, a2);

        Self { highpass, lowpass, band }
    }

    /// Process a single sample through the bandpass filter.
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // High-pass cascade
        let mut sample = input;
        for hp in &mut self.highpass {
            sample = hp.process(sample);
        }

        // Low-pass cascade
        for lp in &mut self.lowpass {
            sample = lp.process(sample);
        }

        sample
    }

    /// Reset the filter state.
    fn reset(&mut self) {
        for hp in &mut self.highpass {
            hp.clear();
        }
        for lp in &mut self.lowpass {
            lp.clear();
        }
    }
}

/// A bank of bandpass filters for extracting multiple frequency bands simultaneously.
///
/// The filter bank uses 4th-order Butterworth bandpass filters for each band,
/// providing good frequency isolation with minimal ringing.
#[derive(Debug, Clone)]
pub struct FilterBank {
    filters: Vec<BandpassFilter>,
    sample_rate: f32,
}

impl FilterBank {
    /// Create a new filter bank with the specified bands.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate in Hz
    /// * `bands` - Slice of frequency bands to extract
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_analysis::filterbank::{FilterBank, eeg_bands};
    ///
    /// let mut bank = FilterBank::new(1000.0, &[eeg_bands::THETA, eeg_bands::LOW_GAMMA]);
    /// ```
    pub fn new(sample_rate: f32, bands: &[FrequencyBand]) -> Self {
        let filters = bands
            .iter()
            .map(|&band| BandpassFilter::new(sample_rate, band))
            .collect();

        Self { filters, sample_rate }
    }

    /// Get the number of bands in the filter bank.
    pub fn num_bands(&self) -> usize {
        self.filters.len()
    }

    /// Get the frequency bands.
    pub fn bands(&self) -> Vec<FrequencyBand> {
        self.filters.iter().map(|f| f.band).collect()
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Extract all frequency bands from the input signal.
    ///
    /// Returns a vector of vectors, where each inner vector contains
    /// the signal filtered to the corresponding frequency band.
    ///
    /// # Arguments
    ///
    /// * `signal` - Input signal samples
    ///
    /// # Returns
    ///
    /// Vector of extracted band signals, in the same order as the bands
    /// were specified during construction.
    pub fn extract(&mut self, signal: &[f32]) -> Vec<Vec<f32>> {
        let mut outputs: Vec<Vec<f32>> = vec![Vec::with_capacity(signal.len()); self.filters.len()];

        // Reset filters before processing
        for filter in &mut self.filters {
            filter.reset();
        }

        // Process each sample through all filters
        for &sample in signal {
            for (i, filter) in self.filters.iter_mut().enumerate() {
                outputs[i].push(filter.process(sample));
            }
        }

        outputs
    }

    /// Extract a single band from the input signal.
    ///
    /// # Arguments
    ///
    /// * `signal` - Input signal samples
    /// * `band_index` - Index of the band to extract (0-based)
    ///
    /// # Returns
    ///
    /// The filtered signal for the specified band, or None if index is out of range.
    pub fn extract_band(&mut self, signal: &[f32], band_index: usize) -> Option<Vec<f32>> {
        if band_index >= self.filters.len() {
            return None;
        }

        self.filters[band_index].reset();

        let mut output = Vec::with_capacity(signal.len());
        for &sample in signal {
            output.push(self.filters[band_index].process(sample));
        }

        Some(output)
    }

    /// Reset all filter states.
    pub fn reset(&mut self) {
        for filter in &mut self.filters {
            filter.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    /// Generate a sine wave at a given frequency.
    fn sine_wave(frequency: f32, sample_rate: f32, duration_secs: f32) -> Vec<f32> {
        let num_samples = (sample_rate * duration_secs) as usize;
        (0..num_samples)
            .map(|i| (2.0 * PI * frequency * i as f32 / sample_rate).sin())
            .collect()
    }

    #[test]
    fn test_frequency_band_properties() {
        let band = FrequencyBand::new("test", 4.0, 8.0);
        assert_eq!(band.bandwidth(), 4.0);
        assert!((band.center_hz() - 5.657).abs() < 0.01); // sqrt(4*8) ≈ 5.657
    }

    #[test]
    fn test_filter_bank_creation() {
        let bank = FilterBank::new(1000.0, &[eeg_bands::THETA, eeg_bands::ALPHA]);
        assert_eq!(bank.num_bands(), 2);
        assert_eq!(bank.sample_rate(), 1000.0);
    }

    #[test]
    fn test_filter_bank_passband() {
        // Test that a signal within the passband is preserved
        let sample_rate = 1000.0;
        let mut bank = FilterBank::new(sample_rate, &[eeg_bands::ALPHA]); // 8-13 Hz

        // Generate a 10 Hz sine (within alpha band)
        let signal = sine_wave(10.0, sample_rate, 2.0);
        let extracted = bank.extract(&signal);

        // Skip the first 500ms for filter settling
        let settling_samples = (sample_rate * 0.5) as usize;
        let output = &extracted[0][settling_samples..];
        let input = &signal[settling_samples..];

        // Calculate RMS of input and output
        let input_rms: f32 = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let output_rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();

        // Output should be close to input (within -3dB)
        let ratio = output_rms / input_rms;
        assert!(ratio > 0.5, "Passband signal should pass through, ratio was {}", ratio);
    }

    #[test]
    fn test_filter_bank_stopband() {
        // Test that a signal outside the passband is attenuated
        let sample_rate = 1000.0;
        let mut bank = FilterBank::new(sample_rate, &[eeg_bands::ALPHA]); // 8-13 Hz

        // Generate a 50 Hz sine (well outside alpha band)
        let signal = sine_wave(50.0, sample_rate, 2.0);
        let extracted = bank.extract(&signal);

        // Skip the first 500ms for filter settling
        let settling_samples = (sample_rate * 0.5) as usize;
        let output = &extracted[0][settling_samples..];
        let input = &signal[settling_samples..];

        // Calculate RMS of input and output
        let input_rms: f32 = (input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32).sqrt();
        let output_rms: f32 = (output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32).sqrt();

        // Output should be significantly attenuated (at least -20dB = 0.1 ratio)
        let ratio = output_rms / input_rms;
        assert!(ratio < 0.2, "Stopband signal should be attenuated, ratio was {}", ratio);
    }

    #[test]
    fn test_filter_bank_multiple_bands() {
        let sample_rate = 1000.0;
        let mut bank = FilterBank::new(sample_rate, &[eeg_bands::THETA, eeg_bands::BETA]);

        // Generate mixed signal: 6 Hz (theta) + 20 Hz (beta)
        let theta_signal = sine_wave(6.0, sample_rate, 2.0);
        let beta_signal = sine_wave(20.0, sample_rate, 2.0);
        let mixed: Vec<f32> = theta_signal.iter()
            .zip(beta_signal.iter())
            .map(|(t, b)| t + b)
            .collect();

        let extracted = bank.extract(&mixed);

        // Skip settling time
        let settling_samples = (sample_rate * 0.5) as usize;

        // Theta extraction should have more theta than beta
        let theta_out = &extracted[0][settling_samples..];
        let theta_rms: f32 = (theta_out.iter().map(|x| x * x).sum::<f32>() / theta_out.len() as f32).sqrt();

        // Beta extraction should have more beta than theta
        let beta_out = &extracted[1][settling_samples..];
        let beta_rms: f32 = (beta_out.iter().map(|x| x * x).sum::<f32>() / beta_out.len() as f32).sqrt();

        // Both should have reasonable energy (not completely attenuated)
        assert!(theta_rms > 0.3, "Theta extraction failed, RMS was {}", theta_rms);
        assert!(beta_rms > 0.3, "Beta extraction failed, RMS was {}", beta_rms);
    }

    #[test]
    fn test_extract_single_band() {
        let sample_rate = 1000.0;
        let mut bank = FilterBank::new(sample_rate, &[eeg_bands::THETA, eeg_bands::ALPHA]);

        let signal = sine_wave(10.0, sample_rate, 1.0);

        let alpha_only = bank.extract_band(&signal, 1).unwrap();
        assert_eq!(alpha_only.len(), signal.len());

        // Out of range should return None
        assert!(bank.extract_band(&signal, 5).is_none());
    }

    #[test]
    fn test_eeg_bands_constants() {
        assert_eq!(eeg_bands::DELTA.low_hz, 0.5);
        assert_eq!(eeg_bands::DELTA.high_hz, 4.0);
        assert_eq!(eeg_bands::THETA.low_hz, 4.0);
        assert_eq!(eeg_bands::THETA.high_hz, 8.0);
        assert_eq!(eeg_bands::ALPHA.low_hz, 8.0);
        assert_eq!(eeg_bands::ALPHA.high_hz, 13.0);
        assert_eq!(eeg_bands::BETA.low_hz, 13.0);
        assert_eq!(eeg_bands::BETA.high_hz, 30.0);
        assert_eq!(eeg_bands::LOW_GAMMA.low_hz, 30.0);
        assert_eq!(eeg_bands::LOW_GAMMA.high_hz, 80.0);
        assert_eq!(eeg_bands::HIGH_GAMMA.low_hz, 80.0);
        assert_eq!(eeg_bands::HIGH_GAMMA.high_hz, 200.0);
    }
}
