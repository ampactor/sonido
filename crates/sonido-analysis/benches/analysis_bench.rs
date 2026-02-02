//! Criterion benchmarks for sonido-analysis components
//!
//! Run with: cargo bench -p sonido-analysis

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sonido_analysis::{
    compare::{mse, rmse, snr_db, spectral_correlation, spectral_difference},
    dynamics::{analyze_dynamics, crest_factor, peak, rms},
    fft::{Fft, Window},
    spectrum::{magnitude_spectrum, spectral_centroid, welch_psd},
    spectrogram::StftAnalyzer,
    ThdAnalyzer,
};
use std::f32::consts::PI;

const SAMPLE_RATE: f32 = 48000.0;

/// Generate a test sine wave
fn generate_sine(size: usize, frequency: f32) -> Vec<f32> {
    (0..size)
        .map(|i| (2.0 * PI * frequency * i as f32 / SAMPLE_RATE).sin())
        .collect()
}

/// Generate a complex test signal with multiple harmonics
fn generate_complex_signal(size: usize) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            let f1 = (2.0 * PI * 440.0 * t).sin();
            let f2 = 0.5 * (2.0 * PI * 880.0 * t).sin();
            let f3 = 0.25 * (2.0 * PI * 1320.0 * t).sin();
            let f4 = 0.125 * (2.0 * PI * 1760.0 * t).sin();
            (f1 + f2 + f3 + f4) * 0.5
        })
        .collect()
}

/// Generate white noise
fn generate_noise(size: usize) -> Vec<f32> {
    let mut state = 0x12345678u32;
    (0..size)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state as i32 as f32) / (i32::MAX as f32)
        })
        .collect()
}

// ============================================================================
// FFT benchmarks
// ============================================================================

fn bench_fft_forward(c: &mut Criterion) {
    let mut group = c.benchmark_group("FFT_Forward");

    let sizes = [256, 512, 1024, 2048, 4096, 8192];

    for &size in &sizes {
        let fft = Fft::new(size);
        let input = generate_sine(size, 440.0);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = fft.forward(black_box(&input));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_fft_inverse(c: &mut Criterion) {
    let mut group = c.benchmark_group("FFT_Inverse");

    let sizes = [256, 512, 1024, 2048, 4096, 8192];

    for &size in &sizes {
        let fft = Fft::new(size);
        let input = generate_sine(size, 440.0);
        let spectrum = fft.forward(&input);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = fft.inverse(black_box(&spectrum));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_fft_roundtrip(c: &mut Criterion) {
    let mut group = c.benchmark_group("FFT_Roundtrip");

    let sizes = [256, 512, 1024, 2048, 4096];

    for &size in &sizes {
        let fft = Fft::new(size);
        let input = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let spectrum = fft.forward(black_box(&input));
                let result = fft.inverse(&spectrum);
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Window function benchmarks
// ============================================================================

fn bench_window_functions(c: &mut Criterion) {
    let mut group = c.benchmark_group("Window");

    let windows = [
        ("Rectangular", Window::Rectangular),
        ("Hann", Window::Hann),
        ("Hamming", Window::Hamming),
        ("Blackman", Window::Blackman),
        ("BlackmanHarris", Window::BlackmanHarris),
    ];

    let size = 2048;

    for (name, window) in &windows {
        let buffer = generate_sine(size, 440.0);

        group.bench_function(*name, |b| {
            b.iter(|| {
                let mut buf = buffer.clone();
                window.apply(black_box(&mut buf));
                black_box(buf)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Spectrum analysis benchmarks
// ============================================================================

fn bench_magnitude_spectrum(c: &mut Criterion) {
    let mut group = c.benchmark_group("MagnitudeSpectrum");

    let sizes = [1024, 2048, 4096];

    for &size in &sizes {
        let signal = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = magnitude_spectrum(black_box(&signal), size, Window::Hann);
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_spectral_centroid(c: &mut Criterion) {
    let mut group = c.benchmark_group("SpectralCentroid");

    let sizes = [1024, 2048, 4096];

    for &size in &sizes {
        let signal = generate_complex_signal(size);
        // First compute magnitude spectrum, then pass to spectral_centroid
        let spectrum = magnitude_spectrum(&signal, size, Window::Hann);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = spectral_centroid(black_box(&spectrum), SAMPLE_RATE);
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_welch_psd(c: &mut Criterion) {
    let mut group = c.benchmark_group("WelchPSD");

    // Test with different signal lengths
    let lengths = [4096, 8192, 16384];
    let fft_size = 1024;

    for &length in &lengths {
        let signal = generate_complex_signal(length);

        group.bench_with_input(BenchmarkId::from_parameter(length), &length, |b, _| {
            b.iter(|| {
                let result = welch_psd(black_box(&signal), SAMPLE_RATE, fft_size, 0.5, Window::Hann);
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Compare module benchmarks
// ============================================================================

fn bench_mse(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare_MSE");

    let sizes = [1024, 4096, 16384, 65536];

    for &size in &sizes {
        let signal_a = generate_sine(size, 440.0);
        let signal_b = generate_sine(size, 441.0); // Slightly different frequency

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = mse(black_box(&signal_a), black_box(&signal_b));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_rmse(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare_RMSE");

    let sizes = [1024, 4096, 16384];

    for &size in &sizes {
        let signal_a = generate_complex_signal(size);
        let signal_b = generate_noise(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = rmse(black_box(&signal_a), black_box(&signal_b));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_snr_db(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare_SNR");

    let sizes = [1024, 4096, 16384];

    for &size in &sizes {
        let reference = generate_sine(size, 440.0);
        // Create noisy version
        let noise = generate_noise(size);
        let test: Vec<f32> = reference
            .iter()
            .zip(noise.iter())
            .map(|(r, n)| r + n * 0.01)
            .collect();

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = snr_db(black_box(&reference), black_box(&test));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_spectral_correlation(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare_SpectralCorrelation");

    let fft_sizes = [1024, 2048, 4096];

    for &fft_size in &fft_sizes {
        let signal_a = generate_complex_signal(fft_size);
        let signal_b = generate_complex_signal(fft_size);

        group.bench_with_input(BenchmarkId::from_parameter(fft_size), &fft_size, |b, _| {
            b.iter(|| {
                let result = spectral_correlation(
                    black_box(&signal_a),
                    black_box(&signal_b),
                    fft_size,
                );
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_spectral_difference(c: &mut Criterion) {
    let mut group = c.benchmark_group("Compare_SpectralDifference");

    let fft_sizes = [1024, 2048, 4096];

    for &fft_size in &fft_sizes {
        let signal_a = generate_complex_signal(fft_size);
        let signal_b = generate_noise(fft_size);

        group.bench_with_input(BenchmarkId::from_parameter(fft_size), &fft_size, |b, _| {
            b.iter(|| {
                let result = spectral_difference(
                    black_box(&signal_a),
                    black_box(&signal_b),
                    fft_size,
                );
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Dynamics analysis benchmarks
// ============================================================================

fn bench_dynamics_rms(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dynamics_RMS");

    let sizes = [1024, 4096, 16384, 65536];

    for &size in &sizes {
        let signal = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = rms(black_box(&signal));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_dynamics_peak(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dynamics_Peak");

    let sizes = [1024, 4096, 16384, 65536];

    for &size in &sizes {
        let signal = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = peak(black_box(&signal));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_dynamics_crest_factor(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dynamics_CrestFactor");

    let sizes = [1024, 4096, 16384];

    for &size in &sizes {
        let signal = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = crest_factor(black_box(&signal));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_dynamics_full_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("Dynamics_FullAnalysis");

    let sizes = [4096, 16384, 65536];
    let window_size = 1024;
    let silence_threshold_db = -60.0;

    for &size in &sizes {
        let signal = generate_complex_signal(size);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let result = analyze_dynamics(black_box(&signal), window_size, silence_threshold_db);
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Spectrogram benchmarks
// ============================================================================

fn bench_stft_analyzer(c: &mut Criterion) {
    let mut group = c.benchmark_group("STFT_Analyzer");

    // Test different FFT sizes
    let fft_sizes = [512, 1024, 2048];
    let signal_length = 48000; // 1 second

    for &fft_size in &fft_sizes {
        let hop_size = fft_size / 4;
        let analyzer = StftAnalyzer::new(SAMPLE_RATE, fft_size, hop_size, Window::Hann);
        let signal = generate_complex_signal(signal_length);

        group.bench_with_input(BenchmarkId::from_parameter(fft_size), &fft_size, |b, _| {
            b.iter(|| {
                let result = analyzer.analyze(black_box(&signal));
                black_box(result)
            })
        });
    }

    group.finish();
}

fn bench_stft_hop_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("STFT_HopSizes");

    let fft_size = 1024;
    let signal_length = 48000;
    let signal = generate_complex_signal(signal_length);

    let hop_ratios = [("25%", fft_size / 4), ("50%", fft_size / 2), ("75%", fft_size * 3 / 4)];

    for (name, hop_size) in &hop_ratios {
        let analyzer = StftAnalyzer::new(SAMPLE_RATE, fft_size, *hop_size, Window::Hann);

        group.bench_function(*name, |b| {
            b.iter(|| {
                let result = analyzer.analyze(black_box(&signal));
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// THD analysis benchmarks
// ============================================================================

fn bench_thd_analyzer(c: &mut Criterion) {
    let mut group = c.benchmark_group("THD_Analyzer");

    let fft_sizes = [2048, 4096, 8192];

    for &fft_size in &fft_sizes {
        let analyzer = ThdAnalyzer::new(SAMPLE_RATE, fft_size);
        // Generate 100ms of 1kHz tone
        let signal = generate_sine(4800, 1000.0);

        group.bench_with_input(BenchmarkId::from_parameter(fft_size), &fft_size, |b, _| {
            b.iter(|| {
                let result = analyzer.analyze(black_box(&signal), 1000.0);
                black_box(result)
            })
        });
    }

    group.finish();
}

// ============================================================================
// Composite analysis benchmark
// ============================================================================

fn bench_full_analysis_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("FullPipeline");

    // Simulate a typical analysis workflow
    let signal_length = 48000; // 1 second
    let fft_size = 2048;
    let window_size = 1024;
    let silence_threshold_db = -60.0;

    group.bench_function("typical_workflow", |b| {
        let signal = generate_complex_signal(signal_length);

        b.iter(|| {
            // 1. Basic dynamics
            let dynamics = analyze_dynamics(black_box(&signal), window_size, silence_threshold_db);

            // 2. Spectral analysis
            let spectrum = magnitude_spectrum(&signal, fft_size, Window::Hann);
            let centroid = spectral_centroid(&spectrum, SAMPLE_RATE);

            // 3. Compare to reference
            let reference = generate_sine(signal_length, 440.0);
            let mse_val = mse(&signal, &reference);
            let corr = spectral_correlation(&signal, &reference, fft_size);

            black_box((dynamics, spectrum, centroid, mse_val, corr))
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_fft_forward,
    bench_fft_inverse,
    bench_fft_roundtrip,
    bench_window_functions,
    bench_magnitude_spectrum,
    bench_spectral_centroid,
    bench_welch_psd,
    bench_mse,
    bench_rmse,
    bench_snr_db,
    bench_spectral_correlation,
    bench_spectral_difference,
    bench_dynamics_rms,
    bench_dynamics_peak,
    bench_dynamics_crest_factor,
    bench_dynamics_full_analysis,
    bench_stft_analyzer,
    bench_stft_hop_sizes,
    bench_thd_analyzer,
    bench_full_analysis_pipeline,
);

criterion_main!(benches);
