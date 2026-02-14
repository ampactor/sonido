//! Integration tests for sonido-analysis crate.
//!
//! Tests exercise the public API of FFT, filterbank, Hilbert transform, and CFC/PAC
//! analysis modules using synthetic signals with known properties.

use std::f32::consts::PI;

use sonido_analysis::cfc::{Comodulogram, PacAnalyzer, PacMethod};
use sonido_analysis::fft::{Fft, Window, magnitude_db};
use sonido_analysis::filterbank::{FilterBank, eeg_bands};
use sonido_analysis::hilbert::HilbertTransform;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Generate a sine wave at a given frequency and amplitude.
fn sine(freq_hz: f32, sample_rate: f32, num_samples: usize, amplitude: f32) -> Vec<f32> {
    (0..num_samples)
        .map(|i| amplitude * (2.0 * PI * freq_hz * i as f32 / sample_rate).sin())
        .collect()
}

/// Generate a cosine wave at a given frequency and amplitude.
fn cosine(freq_hz: f32, sample_rate: f32, num_samples: usize, amplitude: f32) -> Vec<f32> {
    (0..num_samples)
        .map(|i| amplitude * (2.0 * PI * freq_hz * i as f32 / sample_rate).cos())
        .collect()
}

/// RMS of a signal slice.
fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32).sqrt()
}

/// Find the bin index with the maximum magnitude in a complex spectrum.
fn peak_bin(spectrum: &[rustfft::num_complex::Complex<f32>]) -> usize {
    spectrum
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.norm().partial_cmp(&b.norm()).unwrap())
        .map(|(i, _)| i)
        .unwrap()
}

// ===========================================================================
// 1. FFT verification
// ===========================================================================

#[test]
fn fft_peak_matches_input_frequency() {
    let sample_rate = 48000.0;
    let fft_size = 8192;
    let freq_hz = 1000.0;

    let signal = sine(freq_hz, sample_rate, fft_size, 1.0);
    let fft = Fft::new(fft_size);
    let spectrum = fft.forward(&signal);

    // Expected bin: freq * fft_size / sample_rate
    let expected_bin = (freq_hz * fft_size as f32 / sample_rate).round() as usize;
    let actual_bin = peak_bin(&spectrum);

    assert!(
        (actual_bin as i32 - expected_bin as i32).unsigned_abs() <= 1,
        "Peak bin {actual_bin} should be within 1 of expected bin {expected_bin}"
    );
}

#[test]
fn fft_sidelobes_below_peak() {
    // With a rectangular window (no windowing) on a bin-centered tone,
    // all energy lands in a single bin. Other bins should be negligible.
    let sample_rate = 48000.0;
    let fft_size = 4096;
    // Pick a frequency that lands exactly on a bin: bin_k * sample_rate / fft_size
    let bin_k = 100;
    let freq_hz = bin_k as f32 * sample_rate / fft_size as f32;

    let signal = sine(freq_hz, sample_rate, fft_size, 1.0);
    let fft = Fft::new(fft_size);
    let spectrum = fft.forward(&signal);

    let db = magnitude_db(&spectrum);
    let peak_db = db[bin_k];

    // Every non-peak bin should be at least 40 dB below the peak.
    for (i, &val) in db.iter().enumerate() {
        if i == bin_k {
            continue;
        }
        assert!(
            val < peak_db - 40.0,
            "Bin {i} at {val:.1} dB should be >40 dB below peak at {peak_db:.1} dB"
        );
    }
}

#[test]
fn fft_multiple_tones_distinct_peaks() {
    let sample_rate = 48000.0;
    let fft_size = 8192;
    let freq_a = 440.0;
    let freq_b = 2000.0;

    let sig_a = sine(freq_a, sample_rate, fft_size, 1.0);
    let sig_b = sine(freq_b, sample_rate, fft_size, 0.5);
    let signal: Vec<f32> = sig_a.iter().zip(&sig_b).map(|(a, b)| a + b).collect();

    let fft = Fft::new(fft_size);
    let spectrum = fft.forward(&signal);

    let expected_bin_a = (freq_a * fft_size as f32 / sample_rate).round() as usize;
    let expected_bin_b = (freq_b * fft_size as f32 / sample_rate).round() as usize;

    let mag_a = spectrum[expected_bin_a].norm();
    let mag_b = spectrum[expected_bin_b].norm();

    // Both should be prominent
    assert!(
        mag_a > 100.0,
        "Tone A magnitude {mag_a} should be significant"
    );
    assert!(
        mag_b > 50.0,
        "Tone B magnitude {mag_b} should be significant"
    );

    // Ratio should reflect amplitude ratio (within windowing tolerance)
    let ratio = mag_a / mag_b;
    assert!(
        (ratio - 2.0).abs() < 0.5,
        "Magnitude ratio {ratio:.2} should be close to 2.0 (amplitude ratio)"
    );
}

#[test]
fn fft_roundtrip_preserves_signal() {
    let sample_rate = 48000.0;
    let fft_size = 1024;
    let signal = sine(1000.0, sample_rate, fft_size, 0.8);

    let fft = Fft::new(fft_size);
    let spectrum = fft.forward(&signal);
    let reconstructed = fft.inverse(&spectrum);

    for (i, (a, b)) in signal.iter().zip(reconstructed.iter()).enumerate() {
        assert!(
            (a - b).abs() < 0.02,
            "Sample {i}: {a} vs {b} (diff {})",
            (a - b).abs()
        );
    }
}

#[test]
fn fft_windowed_reduces_sidelobes() {
    // Compare sidelobe levels between rectangular and Hann-windowed FFT
    // of a non-bin-centered frequency.
    let sample_rate = 48000.0;
    let fft_size = 4096;
    let freq_hz = 1234.5; // deliberately not bin-centered

    let signal_rect = sine(freq_hz, sample_rate, fft_size, 1.0);
    let mut signal_hann = signal_rect.clone();
    Window::Hann.apply(&mut signal_hann);

    let fft = Fft::new(fft_size);
    let spec_rect = fft.forward(&signal_rect);
    let spec_hann = fft.forward(&signal_hann);

    let db_rect = magnitude_db(&spec_rect);
    let db_hann = magnitude_db(&spec_hann);

    // Find peak bins
    let peak_rect = peak_bin(&spec_rect);
    let peak_hann = peak_bin(&spec_hann);

    // Measure average sidelobe level far from peak (>50 bins away)
    let far_sidelobes_rect: f32 = db_rect
        .iter()
        .enumerate()
        .filter(|(i, _)| (*i as i32 - peak_rect as i32).unsigned_abs() > 50)
        .map(|(_, &v)| v)
        .sum::<f32>()
        / db_rect.len() as f32;

    let far_sidelobes_hann: f32 = db_hann
        .iter()
        .enumerate()
        .filter(|(i, _)| (*i as i32 - peak_hann as i32).unsigned_abs() > 50)
        .map(|(_, &v)| v)
        .sum::<f32>()
        / db_hann.len() as f32;

    // Hann window should yield lower far sidelobes
    assert!(
        far_sidelobes_hann < far_sidelobes_rect,
        "Hann sidelobes ({far_sidelobes_hann:.1} dB) should be lower than rectangular ({far_sidelobes_rect:.1} dB)"
    );
}

// ===========================================================================
// 2. Filterbank band separation
// ===========================================================================

#[test]
fn filterbank_passes_in_band_signal() {
    let sample_rate = 1000.0;
    let duration = 3.0;
    let num_samples = (sample_rate * duration) as usize;
    let in_band_freq = 10.0; // center of alpha (8-13 Hz)

    let signal = sine(in_band_freq, sample_rate, num_samples, 1.0);
    let mut bank = FilterBank::new(sample_rate, &[eeg_bands::ALPHA]);
    let extracted = bank.extract(&signal);

    // Skip first second for filter settling
    let settle = sample_rate as usize;
    let input_rms = rms(&signal[settle..]);
    let output_rms = rms(&extracted[0][settle..]);

    let ratio = output_rms / input_rms;
    assert!(
        ratio > 0.5,
        "In-band signal should pass (ratio {ratio:.3}, expected >0.5)"
    );
}

#[test]
fn filterbank_rejects_out_of_band_signal() {
    let sample_rate = 1000.0;
    let duration = 3.0;
    let num_samples = (sample_rate * duration) as usize;
    let out_of_band_freq = 100.0; // well above alpha (8-13 Hz)

    let signal = sine(out_of_band_freq, sample_rate, num_samples, 1.0);
    let mut bank = FilterBank::new(sample_rate, &[eeg_bands::ALPHA]);
    let extracted = bank.extract(&signal);

    let settle = sample_rate as usize;
    let input_rms = rms(&signal[settle..]);
    let output_rms = rms(&extracted[0][settle..]);

    let ratio = output_rms / input_rms;
    assert!(
        ratio < 0.1,
        "Out-of-band signal should be attenuated (ratio {ratio:.3}, expected <0.1)"
    );
}

#[test]
fn filterbank_separates_composite_signal() {
    // Composite: 6 Hz (theta) + 10 Hz (alpha) + 20 Hz (beta)
    let sample_rate = 1000.0;
    let duration = 4.0;
    let num_samples = (sample_rate * duration) as usize;

    let theta_sig = sine(6.0, sample_rate, num_samples, 1.0);
    let alpha_sig = sine(10.0, sample_rate, num_samples, 1.0);
    let beta_sig = sine(20.0, sample_rate, num_samples, 1.0);

    let composite: Vec<f32> = (0..num_samples)
        .map(|i| theta_sig[i] + alpha_sig[i] + beta_sig[i])
        .collect();

    let mut bank = FilterBank::new(
        sample_rate,
        &[eeg_bands::THETA, eeg_bands::ALPHA, eeg_bands::BETA],
    );
    let extracted = bank.extract(&composite);

    let settle = (sample_rate * 1.5) as usize;

    let theta_out_rms = rms(&extracted[0][settle..]);
    let alpha_out_rms = rms(&extracted[1][settle..]);
    let beta_out_rms = rms(&extracted[2][settle..]);

    // Each band should have substantial energy (from its target tone)
    assert!(theta_out_rms > 0.3, "Theta band RMS {theta_out_rms:.3}");
    assert!(alpha_out_rms > 0.3, "Alpha band RMS {alpha_out_rms:.3}");
    assert!(beta_out_rms > 0.3, "Beta band RMS {beta_out_rms:.3}");
}

#[test]
fn filterbank_cross_band_rejection() {
    // Feed a pure 6 Hz tone and check that alpha and beta bands reject it.
    let sample_rate = 1000.0;
    let duration = 4.0;
    let num_samples = (sample_rate * duration) as usize;

    let signal = sine(6.0, sample_rate, num_samples, 1.0);

    let mut bank = FilterBank::new(
        sample_rate,
        &[eeg_bands::THETA, eeg_bands::ALPHA, eeg_bands::BETA],
    );
    let extracted = bank.extract(&signal);

    let settle = (sample_rate * 1.5) as usize;

    let theta_rms = rms(&extracted[0][settle..]);
    let alpha_rms = rms(&extracted[1][settle..]);
    let beta_rms = rms(&extracted[2][settle..]);

    // Theta should capture most energy
    assert!(
        theta_rms > alpha_rms * 2.0,
        "Theta ({theta_rms:.3}) should dominate over alpha ({alpha_rms:.3})"
    );
    assert!(
        theta_rms > beta_rms * 5.0,
        "Theta ({theta_rms:.3}) should dominate over beta ({beta_rms:.3})"
    );
}

#[test]
fn filterbank_extract_single_band_matches_full() {
    let sample_rate = 1000.0;
    let signal = sine(10.0, sample_rate, 2000, 1.0);

    let mut bank = FilterBank::new(sample_rate, &[eeg_bands::THETA, eeg_bands::ALPHA]);

    let all = bank.extract(&signal);
    let single = bank.extract_band(&signal, 1).unwrap();

    // Single band extraction should match the corresponding band from full extraction.
    // Note: FilterBank::extract resets filters, so both start from the same state.
    for (a, b) in all[1].iter().zip(single.iter()) {
        assert!(
            (a - b).abs() < 1e-6,
            "extract_band should match full extract"
        );
    }
}

// ===========================================================================
// 3. Hilbert transform phase accuracy
// ===========================================================================

#[test]
fn hilbert_cosine_constant_amplitude() {
    let sample_rate = 4096.0;
    let fft_size = 4096;
    let freq = 100.0;

    let signal = cosine(freq, sample_rate, fft_size, 1.0);
    let hilbert = HilbertTransform::new(fft_size);
    let amplitude = hilbert.instantaneous_amplitude(&signal);

    // In the interior (avoid edge effects) the envelope should be ~1.0
    let start = fft_size / 4;
    let end = 3 * fft_size / 4;

    for i in start..end {
        assert!(
            (amplitude[i] - 1.0).abs() < 0.05,
            "Amplitude at sample {i}: {:.4} (expected ~1.0)",
            amplitude[i]
        );
    }
}

#[test]
fn hilbert_phase_linear_ramp() {
    let sample_rate = 4096.0;
    let fft_size = 4096;
    let freq = 50.0;

    // Use sine so phase starts near 0 (sin has phase -pi/2 relative to cos,
    // but the unwrapped phase should still be a linear ramp).
    let signal = sine(freq, sample_rate, fft_size, 1.0);
    let hilbert = HilbertTransform::new(fft_size);
    let phase = hilbert.instantaneous_phase(&signal);
    let unwrapped = HilbertTransform::unwrap_phase(&phase);

    let expected_delta = 2.0 * PI * freq / sample_rate;

    let start = fft_size / 4;
    let end = 3 * fft_size / 4;

    for i in (start + 1)..end {
        let delta = unwrapped[i] - unwrapped[i - 1];
        assert!(
            (delta - expected_delta).abs() < 0.02,
            "Phase delta at {i}: {delta:.6} (expected {expected_delta:.6})"
        );
    }
}

#[test]
fn hilbert_instantaneous_frequency_matches() {
    let sample_rate = 4096.0;
    let fft_size = 4096;
    let freq = 200.0;

    let signal = sine(freq, sample_rate, fft_size, 1.0);
    let hilbert = HilbertTransform::new(fft_size);
    let inst_freq = hilbert.instantaneous_frequency(&signal, sample_rate);

    let start = fft_size / 4;
    let end = 3 * fft_size / 4;

    for i in start..end.min(inst_freq.len()) {
        assert!(
            (inst_freq[i] - freq).abs() < 1.0,
            "Instantaneous freq at {i}: {:.2} Hz (expected {freq} Hz)",
            inst_freq[i]
        );
    }
}

#[test]
fn hilbert_am_envelope_recovery() {
    // Amplitude-modulated signal: carrier 200 Hz, modulator 5 Hz, depth 0.5
    let sample_rate = 4096.0;
    let fft_size = 4096;
    let carrier = 200.0;
    let modulator = 5.0;
    let depth = 0.5;

    let signal: Vec<f32> = (0..fft_size)
        .map(|i| {
            let t = i as f32 / sample_rate;
            let env = 1.0 + depth * (2.0 * PI * modulator * t).cos();
            env * (2.0 * PI * carrier * t).sin()
        })
        .collect();

    let hilbert = HilbertTransform::new(fft_size);
    let amplitude = hilbert.instantaneous_amplitude(&signal);

    // In the interior, envelope should swing between ~0.5 and ~1.5
    let start = fft_size / 4;
    let end = 3 * fft_size / 4;

    let min_amp = amplitude[start..end]
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max_amp = amplitude[start..end]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);

    assert!(
        min_amp < 0.7,
        "Minimum envelope {min_amp:.3} should be near 0.5"
    );
    assert!(
        max_amp > 1.3,
        "Maximum envelope {max_amp:.3} should be near 1.5"
    );
}

#[test]
fn hilbert_phase_and_amplitude_consistency() {
    let fft_size = 2048;
    let signal = sine(50.0, 2048.0, fft_size, 1.0);
    let hilbert = HilbertTransform::new(fft_size);

    let (phase_combined, amp_combined) = hilbert.phase_and_amplitude(&signal);
    let phase_separate = hilbert.instantaneous_phase(&signal);
    let amp_separate = hilbert.instantaneous_amplitude(&signal);

    for i in 0..fft_size {
        assert!(
            (phase_combined[i] - phase_separate[i]).abs() < 1e-6,
            "Phase mismatch at {i}"
        );
        assert!(
            (amp_combined[i] - amp_separate[i]).abs() < 1e-6,
            "Amplitude mismatch at {i}"
        );
    }
}

// ===========================================================================
// 4. CFC / PAC analysis
// ===========================================================================

/// Generate a synthetic PAC signal.
fn generate_pac_signal(
    sample_rate: f32,
    duration_secs: f32,
    theta_freq: f32,
    gamma_freq: f32,
    coupling_strength: f32,
) -> Vec<f32> {
    let n = (sample_rate * duration_secs) as usize;
    (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate;
            let theta_phase = 2.0 * PI * theta_freq * t;
            let theta = theta_phase.sin();
            let modulation = 1.0 + coupling_strength * theta_phase.cos();
            let gamma = modulation * (2.0 * PI * gamma_freq * t).sin();
            theta + 0.5 * gamma
        })
        .collect()
}

#[test]
fn pac_detects_coupling_in_synthetic_signal() {
    let sample_rate = 1000.0;
    let signal = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.8);

    let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
    let result = analyzer.analyze(&signal);

    assert!(
        result.modulation_index > 0.1,
        "Should detect coupling: MI = {:.4}",
        result.modulation_index
    );
}

#[test]
fn pac_uncoupled_signal_low_mi() {
    let sample_rate = 1000.0;
    let n = (sample_rate * 10.0) as usize;

    // Independent theta + gamma (no amplitude modulation)
    let signal: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (2.0 * PI * 6.0 * t).sin() + 0.5 * (2.0 * PI * 50.0 * t).sin()
        })
        .collect();

    let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
    let result = analyzer.analyze(&signal);

    assert!(
        result.modulation_index < 0.3,
        "Uncoupled signal should have low MI: {:.4}",
        result.modulation_index
    );
}

#[test]
fn pac_kl_method_also_detects_coupling() {
    let sample_rate = 1000.0;
    let signal = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.8);

    let mut analyzer = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
    analyzer.set_method(PacMethod::KullbackLeibler);

    let result = analyzer.analyze(&signal);
    assert!(
        result.modulation_index > 0.05,
        "KL method should detect coupling: MI = {:.4}",
        result.modulation_index
    );
}

#[test]
fn pac_stronger_coupling_higher_mi() {
    let sample_rate = 1000.0;

    let weak = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.2);
    let strong = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.9);

    let mut analyzer_w = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);
    let mut analyzer_s = PacAnalyzer::new(sample_rate, eeg_bands::THETA, eeg_bands::LOW_GAMMA);

    let mi_weak = analyzer_w.analyze(&weak).modulation_index;
    let mi_strong = analyzer_s.analyze(&strong).modulation_index;

    assert!(
        mi_strong > mi_weak,
        "Stronger coupling ({mi_strong:.4}) should yield higher MI than weaker ({mi_weak:.4})"
    );
}

#[test]
fn comodulogram_dimensions_and_peak() {
    let sample_rate = 1000.0;
    let signal = generate_pac_signal(sample_rate, 10.0, 6.0, 50.0, 0.8);

    let como = Comodulogram::compute(
        &signal,
        sample_rate,
        (4.0, 10.0, 2.0),   // phase: 4, 6, 8, 10 Hz
        (30.0, 70.0, 10.0), // amplitude: 30, 40, 50, 60, 70 Hz
        0.5,
    );

    assert_eq!(como.phase_frequencies.len(), 4);
    assert_eq!(como.amplitude_frequencies.len(), 5);
    assert_eq!(como.coupling_matrix.len(), 4);
    assert_eq!(como.coupling_matrix[0].len(), 5);

    let (peak_phase, peak_amp, peak_mi) = como.peak_coupling();
    assert!(peak_mi > 0.0, "Peak MI should be positive");
    assert!(
        (4.0..=10.0).contains(&peak_phase),
        "Peak phase freq {peak_phase} should be in [4, 10]"
    );
    assert!(
        (30.0..=70.0).contains(&peak_amp),
        "Peak amp freq {peak_amp} should be in [30, 70]"
    );
}

#[test]
fn comodulogram_csv_roundtrip_structure() {
    let como = Comodulogram {
        phase_frequencies: vec![4.0, 6.0, 8.0],
        amplitude_frequencies: vec![30.0, 50.0],
        coupling_matrix: vec![vec![0.1, 0.2], vec![0.3, 0.4], vec![0.05, 0.15]],
        sample_rate: 1000.0,
    };

    let csv = como.to_csv();
    let lines: Vec<&str> = csv.trim().lines().collect();

    // Header + 3 data rows
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("phase_hz"));

    // Verify lookup
    assert_eq!(como.get_coupling(6.0, 50.0), Some(0.4));
    assert_eq!(como.get_coupling(99.0, 50.0), None);
}

// ===========================================================================
// 5. Cross-module: FFT -> Hilbert pipeline
// ===========================================================================

#[test]
fn fft_hilbert_pipeline_chirp_signal() {
    // Generate a linear chirp from 20 Hz to 200 Hz over 1 second.
    // Verify the Hilbert transform recovers a roughly constant envelope
    // and instantaneous frequency that increases over time.
    let sample_rate = 4096.0;
    let fft_size = 4096;
    let f0 = 20.0;
    let f1 = 200.0;

    let signal: Vec<f32> = (0..fft_size)
        .map(|i| {
            let t = i as f32 / sample_rate;
            let duration = fft_size as f32 / sample_rate;
            let phase = 2.0 * PI * (f0 * t + (f1 - f0) * t * t / (2.0 * duration));
            phase.sin()
        })
        .collect();

    // FFT should show energy spread across frequencies
    let fft = Fft::new(fft_size);
    let spectrum = fft.forward(&signal);
    let db = magnitude_db(&spectrum);

    let bin_f0 = (f0 * fft_size as f32 / sample_rate) as usize;
    let bin_f1 = (f1 * fft_size as f32 / sample_rate) as usize;

    // Energy should be present in the sweep range
    let sweep_energy: f32 = db[bin_f0..=bin_f1]
        .iter()
        .map(|&v| 10.0_f32.powf(v / 10.0))
        .sum();
    let total_energy: f32 = db.iter().map(|&v| 10.0_f32.powf(v / 10.0)).sum();

    assert!(
        sweep_energy / total_energy > 0.5,
        "Most energy should be in sweep range"
    );

    // Hilbert: envelope should be roughly constant ~1.0
    let hilbert = HilbertTransform::new(fft_size);
    let amplitude = hilbert.instantaneous_amplitude(&signal);
    let start = fft_size / 4;
    let end = 3 * fft_size / 4;
    let mean_amp: f32 = amplitude[start..end].iter().sum::<f32>() / (end - start) as f32;

    assert!(
        (mean_amp - 1.0).abs() < 0.15,
        "Mean envelope {mean_amp:.3} should be near 1.0 for constant-amplitude chirp"
    );

    // Instantaneous frequency should increase monotonically (broadly)
    let inst_freq = hilbert.instantaneous_frequency(&signal, sample_rate);
    let quarter = inst_freq.len() / 4;
    let three_quarter = 3 * inst_freq.len() / 4;

    let early_mean: f32 = inst_freq[quarter..quarter + 100].iter().sum::<f32>() / 100.0;
    let late_mean: f32 = inst_freq[three_quarter - 100..three_quarter]
        .iter()
        .sum::<f32>()
        / 100.0;

    assert!(
        late_mean > early_mean,
        "Instantaneous frequency should increase: early {early_mean:.1} Hz, late {late_mean:.1} Hz"
    );
}
