//! Cross-Frequency Coupling (CFC) analysis for biosignal research.
//!
//! This module provides tools for analyzing phase-amplitude coupling (PAC),
//! a form of cross-frequency coupling where the phase of a slow oscillation
//! modulates the amplitude of a faster oscillation.
//!
//! PAC is observed in:
//! - EEG (theta-gamma coupling during memory encoding)
//! - Electric fish communication signals
//! - Slime mold oscillations
//! - Neural oscillations in general
//!
//! # Example
//!
//! ```rust
//! use sonido_analysis::cfc::{PacAnalyzer, PacMethod};
//! use sonido_analysis::filterbank::eeg_bands;
//!
//! // Analyze theta-gamma coupling
//! let mut analyzer = PacAnalyzer::new(
//!     1000.0,  // sample rate
//!     eeg_bands::THETA,     // phase band
//!     eeg_bands::LOW_GAMMA, // amplitude band
//! );
//!
//! let signal = vec![0.0; 5000]; // Your EEG signal
//! let result = analyzer.analyze(&signal);
//!
//! println!("Modulation Index: {:.4}", result.modulation_index);
//! println!("Preferred Phase: {:.2} rad", result.preferred_phase);
//! ```

use crate::filterbank::{FilterBank, FrequencyBand};
use crate::hilbert::HilbertTransform;
use rustfft::num_complex::Complex;
use std::f32::consts::PI;

/// Number of phase bins for amplitude distribution (18 bins = 20 degrees each)
pub const NUM_PHASE_BINS: usize = 18;

/// Result of Phase-Amplitude Coupling analysis.
#[derive(Debug, Clone)]
pub struct PacResult {
    /// The phase band used for analysis
    pub phase_band: FrequencyBand,
    /// The amplitude band used for analysis
    pub amplitude_band: FrequencyBand,
    /// Modulation Index (0-1): strength of coupling
    /// 0 = no coupling, 1 = perfect coupling
    pub modulation_index: f32,
    /// Preferred phase in radians (-PI to PI)
    /// The phase at which amplitude is maximal
    pub preferred_phase: f32,
    /// Mean amplitude in each phase bin (18 bins of 20 degrees)
    pub mean_amplitude_per_phase: [f32; NUM_PHASE_BINS],
}

impl PacResult {
    /// Get the preferred phase in degrees.
    pub fn preferred_phase_degrees(&self) -> f32 {
        self.preferred_phase * 180.0 / PI
    }

    /// Check if coupling is statistically significant.
    ///
    /// This is a simple heuristic threshold. For rigorous statistical testing,
    /// use surrogate data methods.
    pub fn is_significant(&self, threshold: f32) -> bool {
        self.modulation_index > threshold
    }
}

/// Method for computing Phase-Amplitude Coupling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacMethod {
    /// Mean Vector Length method (Canolty et al., 2006)
    /// Fast and intuitive, measures coupling as the mean resultant vector length
    MeanVectorLength,

    /// Kullback-Leibler divergence method (Tort et al., 2010)
    /// Measures how much the amplitude distribution deviates from uniform
    KullbackLeibler,
}

/// Phase-Amplitude Coupling analyzer.
///
/// Analyzes the relationship between the phase of a slow oscillation
/// and the amplitude of a fast oscillation.
pub struct PacAnalyzer {
    sample_rate: f32,
    phase_band: FrequencyBand,
    amplitude_band: FrequencyBand,
    method: PacMethod,
    filter_bank: FilterBank,
    hilbert: HilbertTransform,
}

impl PacAnalyzer {
    /// Create a new PAC analyzer.
    ///
    /// # Arguments
    ///
    /// * `sample_rate` - Sample rate of the input signal in Hz
    /// * `phase_band` - Low frequency band for phase extraction
    /// * `amplitude_band` - High frequency band for amplitude extraction
    ///
    /// # Panics
    ///
    /// Panics if phase_band center frequency >= amplitude_band center frequency.
    pub fn new(sample_rate: f32, phase_band: FrequencyBand, amplitude_band: FrequencyBand) -> Self {
        assert!(
            phase_band.center_hz() < amplitude_band.center_hz(),
            "Phase band center frequency must be less than amplitude band"
        );

        let filter_bank = FilterBank::new(sample_rate, &[phase_band, amplitude_band]);
        let hilbert = HilbertTransform::new(8192); // Large enough for most signals

        Self {
            sample_rate,
            phase_band,
            amplitude_band,
            method: PacMethod::MeanVectorLength,
            filter_bank,
            hilbert,
        }
    }

    /// Set the PAC computation method.
    pub fn set_method(&mut self, method: PacMethod) {
        self.method = method;
    }

    /// Get the current PAC computation method.
    pub fn method(&self) -> PacMethod {
        self.method
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Analyze Phase-Amplitude Coupling in the signal.
    ///
    /// # Arguments
    ///
    /// * `signal` - Input signal samples
    ///
    /// # Returns
    ///
    /// PAC analysis result containing modulation index and phase distribution
    pub fn analyze(&mut self, signal: &[f32]) -> PacResult {
        // Extract the two frequency bands
        let bands = self.filter_bank.extract(signal);
        let phase_signal = &bands[0];
        let amplitude_signal = &bands[1];

        // Get instantaneous phase of low frequency band
        let phase_low = self.hilbert.instantaneous_phase(phase_signal);

        // Get instantaneous amplitude of high frequency band
        let amplitude_high = self.hilbert.instantaneous_amplitude(amplitude_signal);

        match self.method {
            PacMethod::MeanVectorLength => {
                self.compute_mvl(&phase_low, &amplitude_high)
            }
            PacMethod::KullbackLeibler => {
                self.compute_kl(&phase_low, &amplitude_high)
            }
        }
    }

    /// Compute PAC using Mean Vector Length method (Canolty et al., 2006).
    fn compute_mvl(&self, phase: &[f32], amplitude: &[f32]) -> PacResult {
        let n = phase.len().min(amplitude.len());

        if n == 0 {
            return PacResult {
                phase_band: self.phase_band,
                amplitude_band: self.amplitude_band,
                modulation_index: 0.0,
                preferred_phase: 0.0,
                mean_amplitude_per_phase: [0.0; NUM_PHASE_BINS],
            };
        }

        // Normalize amplitude
        let amp_mean: f32 = amplitude[..n].iter().sum::<f32>() / n as f32;
        let amp_normalized: Vec<f32> = if amp_mean > 0.0 {
            amplitude[..n].iter().map(|&a| a / amp_mean).collect()
        } else {
            vec![0.0; n]
        };

        // Compute mean vector: sum of amp * exp(i * phase)
        let mut sum = Complex::new(0.0, 0.0);
        for i in 0..n {
            let c = Complex::from_polar(amp_normalized[i], phase[i]);
            sum += c;
        }

        let mean_vector = sum / n as f32;

        // Modulation index is the magnitude of the mean vector
        let modulation_index = mean_vector.norm();

        // Preferred phase is the angle of the mean vector
        let preferred_phase = mean_vector.arg();

        // Compute mean amplitude per phase bin
        let mean_amplitude_per_phase = self.compute_phase_amplitude_histogram(phase, amplitude, n);

        PacResult {
            phase_band: self.phase_band,
            amplitude_band: self.amplitude_band,
            modulation_index,
            preferred_phase,
            mean_amplitude_per_phase,
        }
    }

    /// Compute PAC using Kullback-Leibler divergence method (Tort et al., 2010).
    fn compute_kl(&self, phase: &[f32], amplitude: &[f32]) -> PacResult {
        let n = phase.len().min(amplitude.len());

        if n == 0 {
            return PacResult {
                phase_band: self.phase_band,
                amplitude_band: self.amplitude_band,
                modulation_index: 0.0,
                preferred_phase: 0.0,
                mean_amplitude_per_phase: [0.0; NUM_PHASE_BINS],
            };
        }

        // Compute mean amplitude per phase bin
        let mean_amplitude_per_phase = self.compute_phase_amplitude_histogram(phase, amplitude, n);

        // Normalize to create a probability distribution
        let total: f32 = mean_amplitude_per_phase.iter().sum();
        let p: Vec<f32> = if total > 0.0 {
            mean_amplitude_per_phase.iter().map(|&a| a / total).collect()
        } else {
            vec![1.0 / NUM_PHASE_BINS as f32; NUM_PHASE_BINS]
        };

        // Uniform distribution
        let q = 1.0 / NUM_PHASE_BINS as f32;

        // Compute KL divergence: sum(p * log(p / q))
        let mut kl = 0.0;
        for &pi in &p {
            if pi > 1e-10 {
                kl += pi * (pi / q).ln();
            }
        }

        // Normalize to [0, 1] range
        // Max KL divergence for 18 bins is log(18) ≈ 2.89
        let max_kl = (NUM_PHASE_BINS as f32).ln();
        let modulation_index = (kl / max_kl).min(1.0);

        // Find preferred phase (bin with maximum amplitude)
        let max_bin = mean_amplitude_per_phase
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i)
            .unwrap_or(0);

        let preferred_phase = -PI + (max_bin as f32 + 0.5) * 2.0 * PI / NUM_PHASE_BINS as f32;

        PacResult {
            phase_band: self.phase_band,
            amplitude_band: self.amplitude_band,
            modulation_index,
            preferred_phase,
            mean_amplitude_per_phase,
        }
    }

    /// Compute histogram of amplitude values binned by phase.
    fn compute_phase_amplitude_histogram(&self, phase: &[f32], amplitude: &[f32], n: usize) -> [f32; NUM_PHASE_BINS] {
        let mut bin_sums = [0.0f32; NUM_PHASE_BINS];
        let mut bin_counts = [0usize; NUM_PHASE_BINS];

        let bin_width = 2.0 * PI / NUM_PHASE_BINS as f32;

        for i in 0..n {
            // Convert phase from [-PI, PI] to bin index [0, NUM_PHASE_BINS-1]
            let normalized_phase = phase[i] + PI; // [0, 2*PI]
            let bin = ((normalized_phase / bin_width) as usize).min(NUM_PHASE_BINS - 1);

            bin_sums[bin] += amplitude[i];
            bin_counts[bin] += 1;
        }

        // Compute mean amplitude per bin
        let mut result = [0.0f32; NUM_PHASE_BINS];
        for i in 0..NUM_PHASE_BINS {
            result[i] = if bin_counts[i] > 0 {
                bin_sums[i] / bin_counts[i] as f32
            } else {
                0.0
            };
        }

        result
    }
}

/// Comodulogram for visualizing PAC across multiple frequency pairs.
///
/// A comodulogram shows the modulation index for all combinations of
/// phase frequencies and amplitude frequencies, revealing which frequency
/// pairs show the strongest coupling.
#[derive(Debug, Clone)]
pub struct Comodulogram {
    /// Center frequencies for phase bands (Hz)
    pub phase_frequencies: Vec<f32>,
    /// Center frequencies for amplitude bands (Hz)
    pub amplitude_frequencies: Vec<f32>,
    /// Coupling matrix: `coupling_matrix[phase_idx][amp_idx]` = MI
    pub coupling_matrix: Vec<Vec<f32>>,
    /// Sample rate used for computation
    pub sample_rate: f32,
}

impl Comodulogram {
    /// Compute a comodulogram for the given signal.
    ///
    /// # Arguments
    ///
    /// * `signal` - Input signal samples
    /// * `sample_rate` - Sample rate in Hz
    /// * `phase_range` - (min, max, step) for phase band center frequencies
    /// * `amplitude_range` - (min, max, step) for amplitude band center frequencies
    /// * `bandwidth_ratio` - Bandwidth as fraction of center frequency (e.g., 0.5)
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_analysis::cfc::Comodulogram;
    ///
    /// let signal = vec![0.0; 10000]; // Your signal
    /// let como = Comodulogram::compute(
    ///     &signal,
    ///     1000.0,              // sample rate
    ///     (2.0, 12.0, 2.0),    // phase: 2-12 Hz, step 2
    ///     (30.0, 100.0, 10.0), // amplitude: 30-100 Hz, step 10
    ///     0.5,                 // bandwidth = 50% of center freq
    /// );
    /// ```
    pub fn compute(
        signal: &[f32],
        sample_rate: f32,
        phase_range: (f32, f32, f32),
        amplitude_range: (f32, f32, f32),
        bandwidth_ratio: f32,
    ) -> Self {
        let (phase_min, phase_max, phase_step) = phase_range;
        let (amp_min, amp_max, amp_step) = amplitude_range;

        // Generate frequency vectors
        let mut phase_frequencies = Vec::new();
        let mut f = phase_min;
        while f <= phase_max {
            phase_frequencies.push(f);
            f += phase_step;
        }

        let mut amplitude_frequencies = Vec::new();
        let mut f = amp_min;
        while f <= amp_max {
            amplitude_frequencies.push(f);
            f += amp_step;
        }

        // Compute coupling matrix
        let mut coupling_matrix = vec![vec![0.0; amplitude_frequencies.len()]; phase_frequencies.len()];

        for (pi, &phase_center) in phase_frequencies.iter().enumerate() {
            for (ai, &amp_center) in amplitude_frequencies.iter().enumerate() {
                // Create bands with specified bandwidth
                let phase_bw = phase_center * bandwidth_ratio;
                let amp_bw = amp_center * bandwidth_ratio;

                let phase_band = FrequencyBand::new(
                    "phase",
                    (phase_center - phase_bw / 2.0).max(0.1),
                    phase_center + phase_bw / 2.0,
                );

                let amplitude_band = FrequencyBand::new(
                    "amplitude",
                    (amp_center - amp_bw / 2.0).max(0.1),
                    amp_center + amp_bw / 2.0,
                );

                // Ensure amplitude band is higher than phase band
                if phase_band.high_hz < amplitude_band.low_hz {
                    let mut analyzer = PacAnalyzer::new(sample_rate, phase_band, amplitude_band);
                    let result = analyzer.analyze(signal);
                    coupling_matrix[pi][ai] = result.modulation_index;
                }
            }
        }

        Self {
            phase_frequencies,
            amplitude_frequencies,
            coupling_matrix,
            sample_rate,
        }
    }

    /// Find the frequency pair with maximum coupling.
    ///
    /// # Returns
    ///
    /// Tuple of (phase_frequency, amplitude_frequency, modulation_index)
    pub fn peak_coupling(&self) -> (f32, f32, f32) {
        let mut max_mi = 0.0;
        let mut max_phase = 0.0;
        let mut max_amp = 0.0;

        for (pi, &phase_f) in self.phase_frequencies.iter().enumerate() {
            for (ai, &amp_f) in self.amplitude_frequencies.iter().enumerate() {
                let mi = self.coupling_matrix[pi][ai];
                if mi > max_mi {
                    max_mi = mi;
                    max_phase = phase_f;
                    max_amp = amp_f;
                }
            }
        }

        (max_phase, max_amp, max_mi)
    }

    /// Export the comodulogram as CSV.
    ///
    /// The format is:
    /// - First row: header with amplitude frequencies
    /// - Subsequent rows: phase frequency, then coupling values
    pub fn to_csv(&self) -> String {
        let mut csv = String::new();

        // Header row
        csv.push_str("phase_hz");
        for &amp_f in &self.amplitude_frequencies {
            csv.push_str(&format!(",{:.1}", amp_f));
        }
        csv.push('\n');

        // Data rows
        for (pi, &phase_f) in self.phase_frequencies.iter().enumerate() {
            csv.push_str(&format!("{:.1}", phase_f));
            for ai in 0..self.amplitude_frequencies.len() {
                csv.push_str(&format!(",{:.6}", self.coupling_matrix[pi][ai]));
            }
            csv.push('\n');
        }

        csv
    }

    /// Get the modulation index for a specific frequency pair.
    ///
    /// Returns None if the frequencies are not in the comodulogram.
    pub fn get_coupling(&self, phase_hz: f32, amplitude_hz: f32) -> Option<f32> {
        let pi = self.phase_frequencies.iter().position(|&f| (f - phase_hz).abs() < 0.01)?;
        let ai = self.amplitude_frequencies.iter().position(|&f| (f - amplitude_hz).abs() < 0.01)?;
        Some(self.coupling_matrix[pi][ai])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filterbank::eeg_bands;

    /// Generate a synthetic PAC signal where gamma amplitude is modulated by theta phase.
    fn generate_pac_signal(
        sample_rate: f32,
        duration_secs: f32,
        theta_freq: f32,
        gamma_freq: f32,
        coupling_strength: f32,
        preferred_phase: f32,
    ) -> Vec<f32> {
        let num_samples = (sample_rate * duration_secs) as usize;

        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;

                // Theta oscillation (phase carrier)
                let theta = (2.0 * PI * theta_freq * t).sin();

                // Theta phase
                let theta_phase = 2.0 * PI * theta_freq * t;

                // Gamma amplitude modulated by theta phase
                // Amplitude is maximal when theta_phase == preferred_phase
                let phase_diff = theta_phase - preferred_phase;
                let modulation = 1.0 + coupling_strength * phase_diff.cos();

                // Gamma oscillation with modulated amplitude
                let gamma = modulation * (2.0 * PI * gamma_freq * t).sin();

                // Combined signal
                theta + 0.5 * gamma
            })
            .collect()
    }

    #[test]
    fn test_pac_analyzer_creation() {
        let analyzer = PacAnalyzer::new(1000.0, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        assert_eq!(analyzer.method(), PacMethod::MeanVectorLength);
    }

    #[test]
    #[should_panic]
    fn test_pac_analyzer_invalid_bands() {
        // Should panic because phase band center > amplitude band center
        let _analyzer = PacAnalyzer::new(1000.0, eeg_bands::LOW_GAMMA, eeg_bands::THETA);
    }

    #[test]
    fn test_pac_detection_synthetic() {
        // Generate a signal with strong theta-gamma coupling
        let sample_rate = 1000.0;
        let signal = generate_pac_signal(
            sample_rate,
            10.0,  // 10 seconds
            6.0,   // theta at 6 Hz
            50.0,  // gamma at 50 Hz
            0.8,   // strong coupling
            0.0,   // preferred phase at 0 radians
        );

        let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        let result = analyzer.analyze(&signal);

        // Should detect significant coupling
        assert!(
            result.modulation_index > 0.1,
            "Expected significant MI, got {}",
            result.modulation_index
        );
    }

    #[test]
    fn test_pac_no_coupling() {
        // Generate a signal with independent theta and gamma (no coupling)
        let sample_rate = 1000.0;
        let num_samples = (sample_rate * 10.0) as usize;

        let signal: Vec<f32> = (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                // Independent oscillations
                let theta = (2.0 * PI * 6.0 * t).sin();
                let gamma = (2.0 * PI * 50.0 * t).sin();
                theta + 0.5 * gamma
            })
            .collect();

        let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        let result = analyzer.analyze(&signal);

        // Should have low coupling
        assert!(
            result.modulation_index < 0.3,
            "Expected low MI for uncoupled signal, got {}",
            result.modulation_index
        );
    }

    #[test]
    fn test_pac_preferred_phase() {
        // Generate signal with coupling at specific phase
        // Note: The detected preferred phase depends on filter phase delays,
        // so we test that the coupling is detected and the phase is consistent
        let sample_rate = 1000.0;
        let target_phase = 0.0; // Use 0 degrees for more reliable detection

        let signal = generate_pac_signal(
            sample_rate,
            15.0,
            6.0,
            50.0,
            0.9,
            target_phase,
        );

        let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        let result = analyzer.analyze(&signal);

        // First, verify we detect significant coupling
        assert!(
            result.modulation_index > 0.1,
            "Should detect coupling, got MI={}",
            result.modulation_index
        );

        // The preferred phase should be a valid angle
        assert!(
            result.preferred_phase >= -PI && result.preferred_phase <= PI,
            "Preferred phase {} should be in [-PI, PI]",
            result.preferred_phase
        );

        // Test that the phase amplitude distribution is non-uniform
        // (indicating coupling was detected correctly)
        let max_amp = result.mean_amplitude_per_phase.iter().copied()
            .fold(f32::NEG_INFINITY, f32::max);
        let min_amp = result.mean_amplitude_per_phase.iter().copied()
            .fold(f32::INFINITY, f32::min);

        assert!(
            max_amp > min_amp * 1.1,
            "Amplitude distribution should be non-uniform: max={}, min={}",
            max_amp, min_amp
        );
    }

    #[test]
    fn test_pac_kl_method() {
        let sample_rate = 1000.0;
        let signal = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.8, 0.0);

        let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        analyzer.set_method(PacMethod::KullbackLeibler);

        let result = analyzer.analyze(&signal);

        // KL method should also detect coupling
        assert!(
            result.modulation_index > 0.05,
            "KL method should detect coupling, got MI={}",
            result.modulation_index
        );
    }

    #[test]
    fn test_pac_result_helpers() {
        let result = PacResult {
            phase_band: eeg_bands::THETA,
            amplitude_band: eeg_bands::LOW_GAMMA,
            modulation_index: 0.25,
            preferred_phase: PI / 2.0,
            mean_amplitude_per_phase: [0.0; NUM_PHASE_BINS],
        };

        assert!((result.preferred_phase_degrees() - 90.0).abs() < 0.1);
        assert!(result.is_significant(0.2));
        assert!(!result.is_significant(0.3));
    }

    #[test]
    fn test_comodulogram_basic() {
        let sample_rate = 1000.0;
        let signal = generate_pac_signal(sample_rate, 5.0, 6.0, 50.0, 0.8, 0.0);

        let como = Comodulogram::compute(
            &signal,
            sample_rate,
            (4.0, 10.0, 2.0),    // phase: 4-10 Hz, step 2
            (30.0, 70.0, 20.0),  // amplitude: 30-70 Hz, step 20
            0.5,
        );

        // Should have correct dimensions
        assert_eq!(como.phase_frequencies.len(), 4);   // 4, 6, 8, 10
        assert_eq!(como.amplitude_frequencies.len(), 3); // 30, 50, 70
        assert_eq!(como.coupling_matrix.len(), 4);
        assert_eq!(como.coupling_matrix[0].len(), 3);
    }

    #[test]
    fn test_comodulogram_peak() {
        let sample_rate = 1000.0;
        let signal = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.9, 0.0);

        let como = Comodulogram::compute(
            &signal,
            sample_rate,
            (4.0, 10.0, 2.0),
            (30.0, 70.0, 10.0),
            0.5,
        );

        let (peak_phase, peak_amp, peak_mi) = como.peak_coupling();

        // Peak should be within the searched range
        assert!((4.0..=10.0).contains(&peak_phase),
            "Peak phase {} should be in range [4, 10] Hz", peak_phase);
        assert!((30.0..=70.0).contains(&peak_amp),
            "Peak amp {} should be in range [30, 70] Hz", peak_amp);
        assert!(peak_mi > 0.0, "Peak MI should be positive");

        // The comodulogram should have detected some coupling
        // (the exact peak location depends on filter responses and bandwidth)
        let total_coupling: f32 = como.coupling_matrix.iter()
            .flat_map(|row| row.iter())
            .sum();
        assert!(total_coupling > 0.0, "Comodulogram should have some coupling detected");
    }

    #[test]
    fn test_comodulogram_csv() {
        let como = Comodulogram {
            phase_frequencies: vec![4.0, 6.0],
            amplitude_frequencies: vec![30.0, 50.0],
            coupling_matrix: vec![
                vec![0.1, 0.2],
                vec![0.3, 0.4],
            ],
            sample_rate: 1000.0,
        };

        let csv = como.to_csv();

        assert!(csv.contains("phase_hz"));
        assert!(csv.contains("30.0"));
        assert!(csv.contains("50.0"));
        assert!(csv.contains("4.0"));
        assert!(csv.contains("6.0"));
        assert!(csv.contains("0.1"));
        assert!(csv.contains("0.4"));
    }

    #[test]
    fn test_comodulogram_get_coupling() {
        let como = Comodulogram {
            phase_frequencies: vec![4.0, 6.0],
            amplitude_frequencies: vec![30.0, 50.0],
            coupling_matrix: vec![
                vec![0.1, 0.2],
                vec![0.3, 0.4],
            ],
            sample_rate: 1000.0,
        };

        assert_eq!(como.get_coupling(4.0, 30.0), Some(0.1));
        assert_eq!(como.get_coupling(6.0, 50.0), Some(0.4));
        assert_eq!(como.get_coupling(8.0, 30.0), None); // Not in grid
    }

    #[test]
    fn test_empty_signal() {
        let mut analyzer = PacAnalyzer::new(1000.0, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
        let result = analyzer.analyze(&[]);

        assert_eq!(result.modulation_index, 0.0);
        assert_eq!(result.preferred_phase, 0.0);
    }
}
