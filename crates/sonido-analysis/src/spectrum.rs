//! Spectral analysis utilities

use crate::fft::{Fft, Window};
use rustfft::num_complex::Complex;

/// Compute magnitude spectrum from time-domain signal
pub fn magnitude_spectrum(signal: &[f32], fft_size: usize, window: Window) -> Vec<f32> {
    let fft = Fft::new(fft_size);

    let mut windowed = signal.to_vec();
    windowed.resize(fft_size, 0.0);
    window.apply(&mut windowed);

    let spectrum = fft.forward(&windowed);
    spectrum.iter().map(|c| c.norm()).collect()
}

/// Compute phase spectrum from time-domain signal
pub fn phase_spectrum(signal: &[f32], fft_size: usize, window: Window) -> Vec<f32> {
    let fft = Fft::new(fft_size);

    let mut windowed = signal.to_vec();
    windowed.resize(fft_size, 0.0);
    window.apply(&mut windowed);

    let spectrum = fft.forward(&windowed);
    spectrum.iter().map(|c| c.arg()).collect()
}

/// Compute power spectrum (magnitude squared) in dB
pub fn power_spectrum_db(signal: &[f32], fft_size: usize, window: Window) -> Vec<f32> {
    let mag = magnitude_spectrum(signal, fft_size, window);
    mag.iter()
        .map(|&m| 10.0 * (m * m).max(1e-10).log10())
        .collect()
}

/// Compute spectral centroid (center of mass of spectrum)
///
/// Returns frequency in Hz
pub fn spectral_centroid(spectrum: &[f32], sample_rate: f32) -> f32 {
    let fft_size = (spectrum.len() - 1) * 2;
    let bin_width = sample_rate / fft_size as f32;

    let mut weighted_sum = 0.0;
    let mut magnitude_sum = 0.0;

    for (i, &mag) in spectrum.iter().enumerate() {
        let freq = i as f32 * bin_width;
        weighted_sum += freq * mag;
        magnitude_sum += mag;
    }

    if magnitude_sum > 1e-10 {
        weighted_sum / magnitude_sum
    } else {
        0.0
    }
}

/// Compute spectral flux (rate of change between frames)
pub fn spectral_flux(prev_spectrum: &[f32], curr_spectrum: &[f32]) -> f32 {
    prev_spectrum
        .iter()
        .zip(curr_spectrum.iter())
        .map(|(&p, &c)| {
            let diff = c - p;
            if diff > 0.0 { diff * diff } else { 0.0 }
        })
        .sum::<f32>()
        .sqrt()
}

/// Compute spectral flatness (how noise-like the spectrum is)
///
/// Returns value between 0 (tonal) and 1 (noise-like)
pub fn spectral_flatness(spectrum: &[f32]) -> f32 {
    let n = spectrum.len() as f32;

    // Geometric mean
    let log_sum: f32 = spectrum.iter().map(|&m| (m.max(1e-10)).ln()).sum();
    let geometric_mean = (log_sum / n).exp();

    // Arithmetic mean
    let arithmetic_mean: f32 = spectrum.iter().sum::<f32>() / n;

    if arithmetic_mean > 1e-10 {
        geometric_mean / arithmetic_mean
    } else {
        0.0
    }
}

/// Compute spectral rolloff (frequency below which X% of energy is contained)
///
/// Default rolloff_percent is 0.85 (85%)
pub fn spectral_rolloff(spectrum: &[f32], sample_rate: f32, rolloff_percent: f32) -> f32 {
    let fft_size = (spectrum.len() - 1) * 2;
    let bin_width = sample_rate / fft_size as f32;

    let total_energy: f32 = spectrum.iter().map(|&m| m * m).sum();
    let threshold = total_energy * rolloff_percent;

    let mut cumulative = 0.0;
    for (i, &mag) in spectrum.iter().enumerate() {
        cumulative += mag * mag;
        if cumulative >= threshold {
            return i as f32 * bin_width;
        }
    }

    sample_rate / 2.0 // Nyquist
}

/// Welch's method for power spectral density estimation
///
/// Computes PSD by averaging periodograms of overlapping windowed segments.
/// This reduces variance compared to a single periodogram.
///
/// # Arguments
/// * `signal` - Input time-domain signal
/// * `sample_rate` - Sample rate in Hz
/// * `segment_size` - Size of each segment (should be power of 2)
/// * `overlap` - Overlap fraction between segments (0.0 to 1.0, typically 0.5)
/// * `window` - Window function to apply
///
/// # Returns
/// Tuple of (frequencies, power_spectral_density) where PSD is in dB
pub fn welch_psd(
    signal: &[f32],
    sample_rate: f32,
    segment_size: usize,
    overlap: f32,
    window: Window,
) -> (Vec<f32>, Vec<f32>) {
    let overlap = overlap.clamp(0.0, 0.99);
    let hop_size = ((1.0 - overlap) * segment_size as f32) as usize;
    let hop_size = hop_size.max(1);

    let fft = Fft::new(segment_size);
    let window_coeffs = window.coefficients(segment_size);

    // Window normalization factor (for power preservation)
    let window_power: f32 = window_coeffs.iter().map(|w| w * w).sum::<f32>() / segment_size as f32;

    let num_bins = segment_size / 2 + 1;
    let mut psd_accum = vec![0.0_f64; num_bins];
    let mut segment_count = 0;

    // Process overlapping segments
    let mut offset = 0;
    while offset + segment_size <= signal.len() {
        // Extract and window segment
        let mut segment: Vec<f32> = signal[offset..offset + segment_size].to_vec();
        for (s, w) in segment.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }

        // Compute FFT
        let spectrum = fft.forward(&segment);

        // Accumulate power (magnitude squared)
        for (i, c) in spectrum.iter().enumerate() {
            let power = (c.re * c.re + c.im * c.im) as f64;
            psd_accum[i] += power;
        }

        segment_count += 1;
        offset += hop_size;
    }

    // Handle case with no complete segments
    if segment_count == 0 {
        // Process partial signal
        let mut segment = signal.to_vec();
        segment.resize(segment_size, 0.0);
        for (s, w) in segment.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }
        let spectrum = fft.forward(&segment);
        for (i, c) in spectrum.iter().enumerate() {
            let power = (c.re * c.re + c.im * c.im) as f64;
            psd_accum[i] += power;
        }
        segment_count = 1;
    }

    // Average and normalize
    let scale = 2.0 / (segment_count as f64 * segment_size as f64 * window_power as f64);
    let psd_db: Vec<f32> = psd_accum
        .iter()
        .map(|&p| {
            let psd_normalized = p * scale;
            10.0 * (psd_normalized.max(1e-20) as f32).log10()
        })
        .collect();

    // Compute frequency bins
    let bin_width = sample_rate / segment_size as f32;
    let frequencies: Vec<f32> = (0..num_bins).map(|i| i as f32 * bin_width).collect();

    (frequencies, psd_db)
}

/// Cross-spectral density estimation using Welch's method
///
/// Computes the cross-spectral density between two signals.
///
/// # Returns
/// Tuple of (frequencies, cross_spectral_density_magnitude, phase)
pub fn cross_spectral_density(
    signal_a: &[f32],
    signal_b: &[f32],
    sample_rate: f32,
    segment_size: usize,
    overlap: f32,
    window: Window,
) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let overlap = overlap.clamp(0.0, 0.99);
    let hop_size = ((1.0 - overlap) * segment_size as f32) as usize;
    let hop_size = hop_size.max(1);

    let len = signal_a.len().min(signal_b.len());
    let fft = Fft::new(segment_size);
    let window_coeffs = window.coefficients(segment_size);

    let num_bins = segment_size / 2 + 1;
    let mut csd_accum: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); num_bins];
    let mut segment_count = 0;

    let mut offset = 0;
    while offset + segment_size <= len {
        // Window both segments
        let mut seg_a: Vec<f32> = signal_a[offset..offset + segment_size].to_vec();
        let mut seg_b: Vec<f32> = signal_b[offset..offset + segment_size].to_vec();

        for (s, w) in seg_a.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }
        for (s, w) in seg_b.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }

        let spec_a = fft.forward(&seg_a);
        let spec_b = fft.forward(&seg_b);

        // Accumulate cross-spectrum: conj(A) * B
        for (i, (a, b)) in spec_a.iter().zip(spec_b.iter()).enumerate() {
            let conj_a = Complex::new(a.re, -a.im);
            let cross = Complex::new(
                (conj_a.re * b.re - conj_a.im * b.im) as f64,
                (conj_a.re * b.im + conj_a.im * b.re) as f64,
            );
            csd_accum[i] += cross;
        }

        segment_count += 1;
        offset += hop_size;
    }

    if segment_count == 0 {
        segment_count = 1;
    }

    // Extract magnitude and phase
    let scale = 1.0 / segment_count as f64;
    let magnitude: Vec<f32> = csd_accum
        .iter()
        .map(|c| {
            let scaled = c * scale;
            (scaled.re * scaled.re + scaled.im * scaled.im).sqrt() as f32
        })
        .collect();

    let phase: Vec<f32> = csd_accum
        .iter()
        .map(|c| (c.im.atan2(c.re)) as f32)
        .collect();

    let bin_width = sample_rate / segment_size as f32;
    let frequencies: Vec<f32> = (0..num_bins).map(|i| i as f32 * bin_width).collect();

    (frequencies, magnitude, phase)
}

/// Coherence estimation using Welch's method
///
/// Computes magnitude-squared coherence between two signals.
/// Values range from 0 (no correlation) to 1 (perfectly correlated).
///
/// # Returns
/// Tuple of (frequencies, coherence)
pub fn coherence(
    signal_a: &[f32],
    signal_b: &[f32],
    sample_rate: f32,
    segment_size: usize,
    overlap: f32,
    window: Window,
) -> (Vec<f32>, Vec<f32>) {
    let overlap = overlap.clamp(0.0, 0.99);
    let hop_size = ((1.0 - overlap) * segment_size as f32) as usize;
    let hop_size = hop_size.max(1);

    let len = signal_a.len().min(signal_b.len());
    let fft = Fft::new(segment_size);
    let window_coeffs = window.coefficients(segment_size);

    let num_bins = segment_size / 2 + 1;
    let mut psd_a_accum = vec![0.0_f64; num_bins];
    let mut psd_b_accum = vec![0.0_f64; num_bins];
    let mut csd_accum: Vec<Complex<f64>> = vec![Complex::new(0.0, 0.0); num_bins];
    let mut segment_count = 0;

    let mut offset = 0;
    while offset + segment_size <= len {
        let mut seg_a: Vec<f32> = signal_a[offset..offset + segment_size].to_vec();
        let mut seg_b: Vec<f32> = signal_b[offset..offset + segment_size].to_vec();

        for (s, w) in seg_a.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }
        for (s, w) in seg_b.iter_mut().zip(window_coeffs.iter()) {
            *s *= w;
        }

        let spec_a = fft.forward(&seg_a);
        let spec_b = fft.forward(&seg_b);

        for (i, (a, b)) in spec_a.iter().zip(spec_b.iter()).enumerate() {
            psd_a_accum[i] += (a.re * a.re + a.im * a.im) as f64;
            psd_b_accum[i] += (b.re * b.re + b.im * b.im) as f64;

            let conj_a = Complex::new(a.re, -a.im);
            let cross = Complex::new(
                (conj_a.re * b.re - conj_a.im * b.im) as f64,
                (conj_a.re * b.im + conj_a.im * b.re) as f64,
            );
            csd_accum[i] += cross;
        }

        segment_count += 1;
        offset += hop_size;
    }

    if segment_count == 0 {
        let frequencies: Vec<f32> = (0..num_bins)
            .map(|i| i as f32 * sample_rate / segment_size as f32)
            .collect();
        return (frequencies, vec![0.0; num_bins]);
    }

    // Coherence = |Cxy|^2 / (Pxx * Pyy)
    let coh: Vec<f32> = csd_accum
        .iter()
        .zip(psd_a_accum.iter().zip(psd_b_accum.iter()))
        .map(|(cxy, (&pxx, &pyy))| {
            let cxy_mag_sq = cxy.re * cxy.re + cxy.im * cxy.im;
            let denom = pxx * pyy;
            if denom > 1e-20 {
                (cxy_mag_sq / denom) as f32
            } else {
                0.0
            }
        })
        .collect();

    let bin_width = sample_rate / segment_size as f32;
    let frequencies: Vec<f32> = (0..num_bins).map(|i| i as f32 * bin_width).collect();

    (frequencies, coh)
}

/// Find peak frequencies in spectrum
///
/// Returns (frequency, magnitude) pairs for local maxima above threshold
pub fn find_peaks(
    spectrum: &[f32],
    sample_rate: f32,
    threshold_db: f32,
    min_distance_hz: f32,
) -> Vec<(f32, f32)> {
    let fft_size = (spectrum.len() - 1) * 2;
    let bin_width = sample_rate / fft_size as f32;
    let _min_distance_bins = (min_distance_hz / bin_width).ceil() as usize;

    let threshold_linear = 10.0_f32.powf(threshold_db / 20.0);

    let mut peaks = Vec::new();

    for i in 1..spectrum.len() - 1 {
        let mag = spectrum[i];
        if mag > threshold_linear
            && mag > spectrum[i - 1]
            && mag > spectrum[i + 1]
        {
            let freq = i as f32 * bin_width;

            // Check minimum distance from existing peaks
            let too_close = peaks
                .iter()
                .any(|(f, _): &(f32, f32)| (f - freq).abs() < min_distance_hz);

            if !too_close {
                peaks.push((freq, mag));
            }
        }
    }

    // Sort by magnitude (descending)
    peaks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_spectral_centroid_pure_tone() {
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let fft_size = 4096;

        // Generate pure tone
        let signal: Vec<f32> = (0..fft_size)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect();

        let spectrum = magnitude_spectrum(&signal, fft_size, Window::Hann);
        let centroid = spectral_centroid(&spectrum, sample_rate);

        // Centroid should be near the fundamental frequency
        assert!(
            (centroid - freq).abs() < 50.0,
            "Centroid {} should be near {}",
            centroid,
            freq
        );
    }

    #[test]
    fn test_spectral_flatness() {
        // Tonal signal (pure sine)
        let tonal: Vec<f32> = (0..1024)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let tonal_spectrum = magnitude_spectrum(&tonal, 1024, Window::Hann);
        let tonal_flatness = spectral_flatness(&tonal_spectrum);

        // Tonal should have low flatness
        assert!(tonal_flatness < 0.3, "Tonal flatness should be low: {}", tonal_flatness);
    }

    #[test]
    fn test_welch_psd_pure_tone() {
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let duration = 1.0;
        let num_samples = (sample_rate * duration) as usize;

        // Generate pure tone
        let signal: Vec<f32> = (0..num_samples)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect();

        let (frequencies, psd) = welch_psd(&signal, sample_rate, 4096, 0.5, Window::Hann);

        // Find peak bin
        let peak_idx = psd
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        let peak_freq = frequencies[peak_idx];

        // Peak should be near 1000 Hz
        assert!(
            (peak_freq - freq).abs() < 20.0,
            "Peak frequency {} should be near {} Hz",
            peak_freq,
            freq
        );
    }

    #[test]
    fn test_welch_psd_returns_correct_length() {
        let signal = vec![0.5; 8192];
        let (frequencies, psd) = welch_psd(&signal, 44100.0, 1024, 0.5, Window::Hann);

        // Should return segment_size/2 + 1 bins
        assert_eq!(frequencies.len(), 513);
        assert_eq!(psd.len(), 513);
    }

    #[test]
    fn test_coherence_identical_signals() {
        let sample_rate = 44100.0;
        let signal: Vec<f32> = (0..8192)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / sample_rate).sin())
            .collect();

        let (_, coh) = coherence(&signal, &signal, sample_rate, 1024, 0.5, Window::Hann);

        // Identical signals should have coherence = 1
        let avg_coh: f32 = coh.iter().sum::<f32>() / coh.len() as f32;
        assert!(
            avg_coh > 0.95,
            "Identical signals should have high coherence, got {}",
            avg_coh
        );
    }

    #[test]
    fn test_cross_spectral_density() {
        let sample_rate = 44100.0;
        let freq = 1000.0;
        let signal: Vec<f32> = (0..8192)
            .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
            .collect();

        let (frequencies, magnitude, _phase) =
            cross_spectral_density(&signal, &signal, sample_rate, 1024, 0.5, Window::Hann);

        // Find peak
        let peak_idx = magnitude
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;

        let peak_freq = frequencies[peak_idx];

        assert!(
            (peak_freq - freq).abs() < 50.0,
            "CSD peak {} should be near {} Hz",
            peak_freq,
            freq
        );
    }
}
