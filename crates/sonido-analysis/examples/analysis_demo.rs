//! Analysis demo: FFT spectrum, filter bank, and dynamics analysis.
//!
//! Run with: cargo run -p sonido-analysis --example analysis_demo

use sonido_analysis::filterbank::{FilterBank, FrequencyBand, eeg_bands};
use sonido_analysis::{
    Fft, Window, analyze_dynamics, crest_factor, crest_factor_db, magnitude_spectrum, peak,
    peak_db, rms, rms_db, spectral_centroid,
};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000.0;

    // --- Generate a test sine wave ---
    println!("=== FFT Spectrum of a 1 kHz Sine Wave ===\n");

    let freq = 1000.0;
    let duration_samples = 4096;
    let signal: Vec<f32> = (0..duration_samples)
        .map(|i| (2.0 * PI * freq * i as f32 / sample_rate).sin())
        .collect();

    // Compute FFT
    let fft_size = 4096;
    let fft = Fft::new(fft_size);

    let mut windowed = signal.clone();
    Window::Hann.apply(&mut windowed);
    let spectrum = fft.forward(&windowed);

    // Find peak bin
    let magnitudes: Vec<f32> = spectrum.iter().map(|c| c.norm()).collect();
    let peak_bin = magnitudes
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i)
        .unwrap();
    let peak_freq = peak_bin as f32 * sample_rate / fft_size as f32;

    println!("Input: {} Hz sine wave, {} samples", freq, duration_samples);
    println!("FFT size: {}, Window: Hann", fft_size);
    println!("Peak bin: {} (frequency: {:.1} Hz)", peak_bin, peak_freq);
    println!("Peak magnitude: {:.2}", magnitudes[peak_bin]);

    // Show spectrum around the peak
    println!("\nSpectrum around peak:");
    println!("{:>8} {:>10} {:>10}", "Bin", "Freq (Hz)", "Magnitude");
    println!("{:->8} {:->10} {:->10}", "", "", "");

    let start = peak_bin.saturating_sub(5);
    let end = (peak_bin + 6).min(magnitudes.len());
    for i in start..end {
        let f = i as f32 * sample_rate / fft_size as f32;
        let marker = if i == peak_bin { " <--" } else { "" };
        println!("{:>8} {:>10.1} {:>10.2}{}", i, f, magnitudes[i], marker);
    }

    // --- Magnitude spectrum convenience function ---
    println!("\n=== Magnitude Spectrum (convenience function) ===\n");

    let mag = magnitude_spectrum(&signal, fft_size, Window::Hann);
    let centroid = spectral_centroid(&mag, sample_rate);
    println!(
        "Spectral centroid: {:.1} Hz (expected ~{} Hz for pure sine)",
        centroid, freq
    );

    // --- Multi-tone signal ---
    println!("\n=== Multi-tone Signal (440 + 880 + 1320 Hz) ===\n");

    let multi_signal: Vec<f32> = (0..fft_size)
        .map(|i| {
            let t = i as f32 / sample_rate;
            0.5 * (2.0 * PI * 440.0 * t).sin()
                + 0.3 * (2.0 * PI * 880.0 * t).sin()
                + 0.2 * (2.0 * PI * 1320.0 * t).sin()
        })
        .collect();

    let multi_mag = magnitude_spectrum(&multi_signal, fft_size, Window::Hann);

    // Find top 5 peaks
    let mut indexed: Vec<(usize, f32)> = multi_mag.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    println!("Top 5 spectral peaks:");
    println!("{:>8} {:>10} {:>10}", "Bin", "Freq (Hz)", "Magnitude");
    println!("{:->8} {:->10} {:->10}", "", "", "");

    for &(bin, mag) in indexed.iter().take(5) {
        let f = bin as f32 * sample_rate / fft_size as f32;
        println!("{:>8} {:>10.1} {:>10.4}", bin, f, mag);
    }

    // --- Dynamics analysis ---
    println!("\n=== Dynamics Analysis ===\n");

    let rms_val = rms(&signal);
    let rms_val_db = rms_db(&signal);
    let peak_val = peak(&signal);
    let peak_val_db = peak_db(&signal);
    let crest = crest_factor(&signal);
    let crest_db = crest_factor_db(&signal);

    println!("1 kHz sine wave dynamics:");
    println!("  RMS:          {:.4} ({:.1} dB)", rms_val, rms_val_db);
    println!("  Peak:         {:.4} ({:.1} dB)", peak_val, peak_val_db);
    println!("  Crest factor: {:.4} ({:.1} dB)", crest, crest_db);
    println!("  (Sine wave theoretical crest factor: 1.414 / 3.01 dB)");

    // Full dynamics analysis (windowed)
    let window_size = 1024;
    let silence_threshold_db = -60.0;
    let dynamics = analyze_dynamics(&signal, window_size, silence_threshold_db);
    println!(
        "\nFull dynamics analysis (windowed, {} sample windows):",
        window_size
    );
    println!("  RMS:            {:.1} dB", dynamics.rms_db);
    println!("  Peak:           {:.1} dB", dynamics.peak_db);
    println!("  Crest factor:   {:.1} dB", dynamics.crest_factor_db);
    println!("  Dynamic range:  {:.1} dB", dynamics.dynamic_range_db);
    println!("  Min RMS:        {:.1} dB", dynamics.min_rms_db);
    println!("  Max RMS:        {:.1} dB", dynamics.max_rms_db);

    // --- Filter Bank ---
    println!("\n=== Filter Bank (EEG Bands) ===\n");

    // Generate a signal with content in multiple EEG bands
    // Using a lower sample rate typical for EEG (1000 Hz)
    let eeg_sample_rate = 1000.0;
    let eeg_duration = 2000; // 2 seconds
    let eeg_signal: Vec<f32> = (0..eeg_duration)
        .map(|i| {
            let t = i as f32 / eeg_sample_rate;
            // Theta (6 Hz) + Alpha (10 Hz) + Beta (20 Hz)
            0.5 * (2.0 * PI * 6.0 * t).sin()
                + 0.8 * (2.0 * PI * 10.0 * t).sin()
                + 0.3 * (2.0 * PI * 20.0 * t).sin()
        })
        .collect();

    let bands = [eeg_bands::THETA, eeg_bands::ALPHA, eeg_bands::BETA];
    let mut bank = FilterBank::new(eeg_sample_rate, &bands);

    let extracted = bank.extract(&eeg_signal);

    println!("Signal: theta(6Hz, amp=0.5) + alpha(10Hz, amp=0.8) + beta(20Hz, amp=0.3)");
    println!(
        "Sample rate: {} Hz, Duration: {} samples\n",
        eeg_sample_rate, eeg_duration
    );

    println!(
        "{:<12} {:>10} {:>10} {:>12}",
        "Band", "Range (Hz)", "RMS", "Rel. Power"
    );
    println!("{:-<12} {:->10} {:->10} {:->12}", "", "", "", "");

    let band_rms_vals: Vec<f32> = extracted.iter().map(|b| rms(b)).collect();
    let total_power: f32 = band_rms_vals.iter().map(|r| r * r).sum();

    for (i, band) in bands.iter().enumerate() {
        let band_rms = band_rms_vals[i];
        let rel_power = if total_power > 0.0 {
            (band_rms * band_rms) / total_power * 100.0
        } else {
            0.0
        };
        println!(
            "{:<12} {:>4.0}-{:<5.0} {:>10.4} {:>11.1}%",
            band.name, band.low_hz, band.high_hz, band_rms, rel_power
        );
    }

    // --- Custom frequency bands ---
    println!("\n=== Custom Audio Frequency Bands ===\n");

    let audio_bands = [
        FrequencyBand::new("sub_bass", 20.0, 60.0),
        FrequencyBand::new("bass", 60.0, 250.0),
        FrequencyBand::new("low_mid", 250.0, 2000.0),
        FrequencyBand::new("high_mid", 2000.0, 6000.0),
        FrequencyBand::new("presence", 6000.0, 20000.0),
    ];

    println!("Audio frequency bands:");
    for band in &audio_bands {
        println!(
            "  {:<12} {:.0}-{:.0} Hz (center: {:.0} Hz, BW: {:.0} Hz)",
            band.name,
            band.low_hz,
            band.high_hz,
            band.center_hz(),
            band.bandwidth()
        );
    }

    // --- Window functions comparison ---
    println!("\n=== Window Functions ===\n");

    let windows = [
        ("Rectangular", Window::Rectangular),
        ("Hann", Window::Hann),
        ("Hamming", Window::Hamming),
        ("Blackman", Window::Blackman),
        ("Blackman-Harris", Window::BlackmanHarris),
    ];

    println!(
        "{:<18} {:>10} {:>10} {:>12}",
        "Window", "Peak Mag", "Peak Freq", "Centroid"
    );
    println!("{:-<18} {:->10} {:->10} {:->12}", "", "", "", "");

    for (name, window) in &windows {
        let mag = magnitude_spectrum(&signal, fft_size, *window);
        let peak_bin = mag
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
            .map(|(i, _)| i)
            .unwrap();
        let peak_f = peak_bin as f32 * sample_rate / fft_size as f32;
        let cent = spectral_centroid(&mag, sample_rate);

        println!(
            "{:<18} {:>10.2} {:>10.1} {:>12.1}",
            name, mag[peak_bin], peak_f, cent
        );
    }

    println!("\nAnalysis demo complete.");
}
