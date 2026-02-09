//! Dynamics analysis for audio signals
//!
//! This module provides tools for analyzing the dynamic characteristics of audio:
//! - RMS level measurement
//! - Peak detection
//! - Crest factor (peak-to-RMS ratio)
//! - Dynamic range
//! - Loudness envelope

/// Compute RMS (Root Mean Square) level of a signal
///
/// Returns RMS value in linear scale (not dB)
pub fn rms(signal: &[f32]) -> f32 {
    if signal.is_empty() {
        return 0.0;
    }

    let sum_sq: f32 = signal.iter().map(|&x| x * x).sum();
    (sum_sq / signal.len() as f32).sqrt()
}

/// Compute RMS level in dB
pub fn rms_db(signal: &[f32]) -> f32 {
    let rms_val = rms(signal);
    if rms_val > 1e-10 {
        20.0 * rms_val.log10()
    } else {
        -200.0 // Effectively silence
    }
}

/// Compute peak level (maximum absolute value)
pub fn peak(signal: &[f32]) -> f32 {
    signal
        .iter()
        .map(|x| x.abs())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0.0)
}

/// Compute peak level in dB
pub fn peak_db(signal: &[f32]) -> f32 {
    let peak_val = peak(signal);
    if peak_val > 1e-10 {
        20.0 * peak_val.log10()
    } else {
        -200.0
    }
}

/// Compute crest factor (peak-to-RMS ratio)
///
/// Higher values indicate more transient/percussive content.
/// Typical values:
/// - Sine wave: ~1.41 (3 dB)
/// - Music: 4-8 (12-18 dB)
/// - Highly compressed: 2-4 (6-12 dB)
pub fn crest_factor(signal: &[f32]) -> f32 {
    let rms_val = rms(signal);
    let peak_val = peak(signal);

    if rms_val > 1e-10 {
        peak_val / rms_val
    } else {
        0.0
    }
}

/// Compute crest factor in dB
pub fn crest_factor_db(signal: &[f32]) -> f32 {
    let cf = crest_factor(signal);
    if cf > 1e-10 { 20.0 * cf.log10() } else { 0.0 }
}

/// Compute dynamic range in dB
///
/// Measures the ratio between the loudest and quietest non-silent portions.
/// Uses windowed RMS to find min/max levels.
///
/// # Arguments
/// * `signal` - Input audio signal
/// * `window_size` - Window size for RMS calculation
/// * `silence_threshold_db` - Threshold below which audio is considered silent
pub fn dynamic_range_db(signal: &[f32], window_size: usize, silence_threshold_db: f32) -> f32 {
    if signal.len() < window_size {
        return 0.0;
    }

    let silence_threshold = 10.0_f32.powf(silence_threshold_db / 20.0);

    let mut max_rms = 0.0_f32;
    let mut min_rms = f32::MAX;

    for window in signal.windows(window_size) {
        let window_rms = rms(window);

        if window_rms > silence_threshold {
            max_rms = max_rms.max(window_rms);
            min_rms = min_rms.min(window_rms);
        }
    }

    if min_rms > 1e-10 && max_rms > min_rms {
        20.0 * (max_rms / min_rms).log10()
    } else {
        0.0
    }
}

/// Compute amplitude envelope using RMS windowing
///
/// # Arguments
/// * `signal` - Input signal
/// * `window_size` - Window size for envelope calculation
/// * `hop_size` - Hop size between windows
pub fn envelope(signal: &[f32], window_size: usize, hop_size: usize) -> Vec<f32> {
    if signal.len() < window_size || hop_size == 0 {
        return vec![rms(signal)];
    }

    let mut env = Vec::new();
    let mut offset = 0;

    while offset + window_size <= signal.len() {
        let window_rms = rms(&signal[offset..offset + window_size]);
        env.push(window_rms);
        offset += hop_size;
    }

    env
}

/// Compute envelope in dB
pub fn envelope_db(signal: &[f32], window_size: usize, hop_size: usize) -> Vec<f32> {
    envelope(signal, window_size, hop_size)
        .iter()
        .map(|&x| if x > 1e-10 { 20.0 * x.log10() } else { -200.0 })
        .collect()
}

/// Detect transients in a signal
///
/// Returns indices where transients are detected based on envelope derivative.
///
/// # Arguments
/// * `signal` - Input signal
/// * `window_size` - Window size for envelope calculation
/// * `hop_size` - Hop size between windows
/// * `threshold_db` - Minimum level increase to detect a transient
pub fn detect_transients(
    signal: &[f32],
    window_size: usize,
    hop_size: usize,
    threshold_db: f32,
) -> Vec<usize> {
    let env = envelope_db(signal, window_size, hop_size);

    if env.len() < 2 {
        return vec![];
    }

    let mut transients = Vec::new();

    for i in 1..env.len() {
        let diff = env[i] - env[i - 1];
        if diff > threshold_db {
            // Convert envelope index back to sample index
            let sample_idx = i * hop_size;
            transients.push(sample_idx);
        }
    }

    transients
}

/// Compute loudness histogram
///
/// Returns histogram of RMS levels in dB, useful for analyzing
/// loudness distribution of audio content.
///
/// # Arguments
/// * `signal` - Input signal
/// * `window_size` - Window size for RMS calculation
/// * `hop_size` - Hop size between windows
/// * `min_db` - Minimum dB value for histogram
/// * `max_db` - Maximum dB value for histogram
/// * `num_bins` - Number of histogram bins
pub fn loudness_histogram(
    signal: &[f32],
    window_size: usize,
    hop_size: usize,
    min_db: f32,
    max_db: f32,
    num_bins: usize,
) -> (Vec<f32>, Vec<u32>) {
    let env = envelope_db(signal, window_size, hop_size);

    let bin_width = (max_db - min_db) / num_bins as f32;
    let mut histogram = vec![0_u32; num_bins];
    let bin_centers: Vec<f32> = (0..num_bins)
        .map(|i| min_db + (i as f32 + 0.5) * bin_width)
        .collect();

    for &level in &env {
        if level >= min_db && level < max_db {
            let bin = ((level - min_db) / bin_width) as usize;
            let bin = bin.min(num_bins - 1);
            histogram[bin] += 1;
        }
    }

    (bin_centers, histogram)
}

/// Analyze dynamics of a signal
///
/// Returns a comprehensive dynamics analysis
#[derive(Debug, Clone)]
pub struct DynamicsAnalysis {
    pub rms_db: f32,
    pub peak_db: f32,
    pub crest_factor_db: f32,
    pub dynamic_range_db: f32,
    pub min_rms_db: f32,
    pub max_rms_db: f32,
}

/// Perform comprehensive dynamics analysis
///
/// # Arguments
/// * `signal` - Input signal
/// * `window_size` - Window size for windowed measurements
/// * `silence_threshold_db` - Threshold for silence detection
pub fn analyze_dynamics(
    signal: &[f32],
    window_size: usize,
    silence_threshold_db: f32,
) -> DynamicsAnalysis {
    let env = envelope_db(signal, window_size, window_size / 2);
    let silence_threshold = silence_threshold_db;

    let active_levels: Vec<f32> = env
        .iter()
        .copied()
        .filter(|&x| x > silence_threshold)
        .collect();

    let (min_rms, max_rms) = if active_levels.is_empty() {
        (-200.0, -200.0)
    } else {
        (
            active_levels
                .iter()
                .copied()
                .min_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap(),
            active_levels
                .iter()
                .copied()
                .max_by(|a, b| a.partial_cmp(b).unwrap())
                .unwrap(),
        )
    };

    DynamicsAnalysis {
        rms_db: rms_db(signal),
        peak_db: peak_db(signal),
        crest_factor_db: crest_factor_db(signal),
        dynamic_range_db: max_rms - min_rms,
        min_rms_db: min_rms,
        max_rms_db: max_rms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_rms_sine_wave() {
        // RMS of unit sine wave should be 1/sqrt(2) ≈ 0.707
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let rms_val = rms(&sine);
        let expected = 1.0 / 2.0_f32.sqrt();

        assert!(
            (rms_val - expected).abs() < 0.01,
            "RMS {} should be near {}",
            rms_val,
            expected
        );
    }

    #[test]
    fn test_rms_db_sine_wave() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let rms_db_val = rms_db(&sine);
        // Unit sine RMS in dB: 20*log10(1/sqrt(2)) ≈ -3.01 dB
        let expected = -3.01;

        assert!(
            (rms_db_val - expected).abs() < 0.1,
            "RMS dB {} should be near {}",
            rms_db_val,
            expected
        );
    }

    #[test]
    fn test_peak_sine_wave() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let peak_val = peak(&sine);
        assert!(
            (peak_val - 1.0).abs() < 0.001,
            "Peak {} should be near 1.0",
            peak_val
        );
    }

    #[test]
    fn test_crest_factor_sine() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let cf = crest_factor(&sine);
        let expected = 2.0_f32.sqrt(); // sqrt(2) for sine wave

        assert!(
            (cf - expected).abs() < 0.01,
            "Crest factor {} should be near {}",
            cf,
            expected
        );
    }

    #[test]
    fn test_crest_factor_db_sine() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let cf_db = crest_factor_db(&sine);
        // Crest factor in dB for sine: 20*log10(sqrt(2)) ≈ 3.01 dB
        let expected = 3.01;

        assert!(
            (cf_db - expected).abs() < 0.1,
            "Crest factor dB {} should be near {}",
            cf_db,
            expected
        );
    }

    #[test]
    fn test_envelope_length() {
        let signal = vec![0.5; 10000];
        let env = envelope(&signal, 1024, 512);

        // (10000 - 1024) / 512 + 1 = 17 + 1 = 18 windows fit
        let expected_len = (10000 - 1024) / 512 + 1;
        assert_eq!(env.len(), expected_len);
    }

    #[test]
    fn test_detect_transients() {
        // Create a signal with a transient
        let mut signal = vec![0.1; 4000];
        // Add loud burst
        for sample in signal.iter_mut().take(2500).skip(2000) {
            *sample = 0.9;
        }

        let transients = detect_transients(&signal, 256, 128, 6.0);

        // Should detect transient near sample 2000
        assert!(
            !transients.is_empty(),
            "Should detect at least one transient"
        );

        let first_transient = transients[0];
        assert!(
            (first_transient as i32 - 2000).abs() < 500,
            "Transient at {} should be near 2000",
            first_transient
        );
    }

    #[test]
    fn test_analyze_dynamics() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let analysis = analyze_dynamics(&sine, 1024, -60.0);

        assert!(analysis.rms_db > -4.0 && analysis.rms_db < -2.0);
        assert!(analysis.peak_db > -1.0 && analysis.peak_db < 1.0);
        assert!(analysis.crest_factor_db > 2.0 && analysis.crest_factor_db < 4.0);
    }

    #[test]
    fn test_loudness_histogram() {
        let sine: Vec<f32> = (0..44100)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let (bin_centers, histogram) = loudness_histogram(&sine, 1024, 512, -60.0, 0.0, 60);

        // Should have 60 bins
        assert_eq!(bin_centers.len(), 60);
        assert_eq!(histogram.len(), 60);

        // Most values should be around -3 dB for sine wave
        let total: u32 = histogram.iter().sum();
        assert!(total > 0, "Histogram should have counts");
    }

    #[test]
    fn test_empty_signal() {
        let empty: Vec<f32> = vec![];

        assert_eq!(rms(&empty), 0.0);
        assert_eq!(peak(&empty), 0.0);
        assert_eq!(crest_factor(&empty), 0.0);
    }
}
