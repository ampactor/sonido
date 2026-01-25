//! Spectral analysis utilities

use crate::fft::{Fft, Window};

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
}
