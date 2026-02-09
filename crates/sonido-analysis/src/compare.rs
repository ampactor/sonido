//! A/B comparison tools for audio signals

use crate::fft::{Fft, Window};

/// Compute spectral correlation between two signals
///
/// Returns a value between -1 and 1, where 1 means identical spectra
pub fn spectral_correlation(signal_a: &[f32], signal_b: &[f32], fft_size: usize) -> f32 {
    let fft = Fft::new(fft_size);
    let window = Window::Hann;

    // Compute magnitude spectra
    let mut a_windowed = signal_a.to_vec();
    a_windowed.resize(fft_size, 0.0);
    window.apply(&mut a_windowed);

    let mut b_windowed = signal_b.to_vec();
    b_windowed.resize(fft_size, 0.0);
    window.apply(&mut b_windowed);

    let spec_a = fft.forward(&a_windowed);
    let spec_b = fft.forward(&b_windowed);

    let mag_a: Vec<f32> = spec_a.iter().map(|c| c.norm()).collect();
    let mag_b: Vec<f32> = spec_b.iter().map(|c| c.norm()).collect();

    // Pearson correlation
    let n = mag_a.len() as f32;
    let mean_a = mag_a.iter().sum::<f32>() / n;
    let mean_b = mag_b.iter().sum::<f32>() / n;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for (a, b) in mag_a.iter().zip(mag_b.iter()) {
        let da = a - mean_a;
        let db = b - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    if var_a > 1e-10 && var_b > 1e-10 {
        cov / (var_a.sqrt() * var_b.sqrt())
    } else {
        0.0
    }
}

/// Compute spectral difference in dB
///
/// Returns the average magnitude difference across frequency bins
pub fn spectral_difference(signal_a: &[f32], signal_b: &[f32], fft_size: usize) -> f32 {
    let fft = Fft::new(fft_size);
    let window = Window::Hann;

    let mut a_windowed = signal_a.to_vec();
    a_windowed.resize(fft_size, 0.0);
    window.apply(&mut a_windowed);

    let mut b_windowed = signal_b.to_vec();
    b_windowed.resize(fft_size, 0.0);
    window.apply(&mut b_windowed);

    let spec_a = fft.forward(&a_windowed);
    let spec_b = fft.forward(&b_windowed);

    let mut total_diff = 0.0;

    for (a, b) in spec_a.iter().zip(spec_b.iter()) {
        let mag_a_db = 20.0 * a.norm().max(1e-10).log10();
        let mag_b_db = 20.0 * b.norm().max(1e-10).log10();
        total_diff += (mag_a_db - mag_b_db).abs();
    }

    total_diff / spec_a.len() as f32
}

/// Compute Mean Squared Error between two signals
pub fn mse(signal_a: &[f32], signal_b: &[f32]) -> f32 {
    let len = signal_a.len().min(signal_b.len());
    if len == 0 {
        return 0.0;
    }

    let sum: f32 = signal_a[..len]
        .iter()
        .zip(signal_b[..len].iter())
        .map(|(a, b)| (a - b).powi(2))
        .sum();

    sum / len as f32
}

/// Compute Root Mean Squared Error
pub fn rmse(signal_a: &[f32], signal_b: &[f32]) -> f32 {
    mse(signal_a, signal_b).sqrt()
}

/// Compute Signal-to-Noise Ratio in dB
///
/// Treats signal_b as the "noisy" version of signal_a
pub fn snr_db(reference: &[f32], test: &[f32]) -> f32 {
    let len = reference.len().min(test.len());
    if len == 0 {
        return 0.0;
    }

    let signal_power: f32 = reference[..len].iter().map(|x| x.powi(2)).sum();
    let noise_power: f32 = reference[..len]
        .iter()
        .zip(test[..len].iter())
        .map(|(r, t)| (r - t).powi(2))
        .sum();

    if noise_power > 1e-10 {
        10.0 * (signal_power / noise_power).log10()
    } else {
        f32::INFINITY
    }
}

/// Compare envelopes (amplitude over time)
///
/// Returns correlation between amplitude envelopes
pub fn envelope_correlation(signal_a: &[f32], signal_b: &[f32], window_size: usize) -> f32 {
    let env_a = compute_envelope(signal_a, window_size);
    let env_b = compute_envelope(signal_b, window_size);

    let len = env_a.len().min(env_b.len());
    if len == 0 {
        return 0.0;
    }

    let mean_a: f32 = env_a[..len].iter().sum::<f32>() / len as f32;
    let mean_b: f32 = env_b[..len].iter().sum::<f32>() / len as f32;

    let mut cov = 0.0;
    let mut var_a = 0.0;
    let mut var_b = 0.0;

    for i in 0..len {
        let da = env_a[i] - mean_a;
        let db = env_b[i] - mean_b;
        cov += da * db;
        var_a += da * da;
        var_b += db * db;
    }

    if var_a > 1e-10 && var_b > 1e-10 {
        cov / (var_a.sqrt() * var_b.sqrt())
    } else {
        0.0
    }
}

/// Compute amplitude envelope
fn compute_envelope(signal: &[f32], window_size: usize) -> Vec<f32> {
    if signal.len() < window_size {
        return vec![signal.iter().map(|x| x.abs()).sum::<f32>() / signal.len().max(1) as f32];
    }

    signal
        .windows(window_size)
        .map(|w| w.iter().map(|x| x.abs()).sum::<f32>() / window_size as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_spectral_correlation_identical() {
        let signal: Vec<f32> = (0..1024)
            .map(|i| (2.0 * PI * 440.0 * i as f32 / 44100.0).sin())
            .collect();

        let corr = spectral_correlation(&signal, &signal, 1024);
        assert!(
            corr > 0.99,
            "Identical signals should have correlation ~1, got {}",
            corr
        );
    }

    #[test]
    fn test_mse_identical() {
        let signal = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(mse(&signal, &signal), 0.0);
    }

    #[test]
    fn test_mse_difference() {
        let a = vec![1.0, 2.0, 3.0, 4.0];
        let b = vec![2.0, 3.0, 4.0, 5.0];
        assert_eq!(mse(&a, &b), 1.0); // Each differs by 1, squared = 1, mean = 1
    }

    #[test]
    fn test_snr_db() {
        let reference = vec![1.0; 100];
        let test = vec![1.0; 100];
        let snr = snr_db(&reference, &test);
        assert!(snr > 100.0, "Identical signals should have very high SNR");
    }
}
