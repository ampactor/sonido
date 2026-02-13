//! A/B comparison command for reverse engineering.

use clap::Args;
use sonido_analysis::compare::{mse, rmse, snr_db};
use sonido_analysis::dynamics;
use sonido_analysis::{Fft, Window, spectral_correlation, spectral_difference};
use sonido_core::linear_to_db;
use sonido_io::read_wav;
use std::path::PathBuf;

#[derive(Args)]
pub struct CompareArgs {
    /// Reference audio file (e.g., hardware recording)
    #[arg(value_name = "REFERENCE")]
    reference: PathBuf,

    /// Implementation audio file (e.g., software processing)
    #[arg(value_name = "IMPLEMENTATION")]
    implementation: PathBuf,

    /// FFT size for spectral analysis
    #[arg(long, default_value = "4096")]
    fft_size: usize,

    /// Output detailed JSON report
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Show detailed frequency band analysis
    #[arg(long)]
    detailed: bool,
}

pub fn run(args: CompareArgs) -> anyhow::Result<()> {
    println!("A/B Comparison");
    println!("==============");
    println!("  Reference:      {}", args.reference.display());
    println!("  Implementation: {}", args.implementation.display());
    println!();

    // Load files
    let (ref_samples, ref_spec) = read_wav(&args.reference)?;
    let (impl_samples, impl_spec) = read_wav(&args.implementation)?;

    if ref_spec.sample_rate != impl_spec.sample_rate {
        anyhow::bail!(
            "Sample rate mismatch: {} vs {}",
            ref_spec.sample_rate,
            impl_spec.sample_rate
        );
    }

    let sample_rate = ref_spec.sample_rate as f32;

    // Use the shorter length
    let len = ref_samples.len().min(impl_samples.len());
    let ref_samples = &ref_samples[..len];
    let impl_samples = &impl_samples[..len];

    println!(
        "Comparing {} samples ({:.2}s at {} Hz)",
        len,
        len as f32 / sample_rate,
        ref_spec.sample_rate
    );
    println!();

    // Time-domain metrics
    let mse_val = mse(ref_samples, impl_samples);
    let rmse_val = rmse(ref_samples, impl_samples);
    let snr_val = snr_db(ref_samples, impl_samples);

    // Level analysis
    let ref_rms = dynamics::rms(ref_samples);
    let impl_rms = dynamics::rms(impl_samples);
    let ref_peak = dynamics::peak(ref_samples);
    let impl_peak = dynamics::peak(impl_samples);

    println!("Time Domain Metrics");
    println!("-------------------");
    println!("  MSE:          {:.6}", mse_val);
    println!("  RMSE:         {:.6}", rmse_val);
    println!("  SNR:          {:.1} dB", snr_val);
    println!();
    println!(
        "  Reference  - RMS: {:.1} dB, Peak: {:.1} dB",
        linear_to_db(ref_rms),
        linear_to_db(ref_peak)
    );
    println!(
        "  Implementation - RMS: {:.1} dB, Peak: {:.1} dB",
        linear_to_db(impl_rms),
        linear_to_db(impl_peak)
    );
    println!(
        "  Level diff - RMS: {:.1} dB, Peak: {:.1} dB",
        linear_to_db(impl_rms) - linear_to_db(ref_rms),
        linear_to_db(impl_peak) - linear_to_db(ref_peak)
    );
    println!();

    // Spectral analysis
    let fft_size = args.fft_size;

    let correlation = spectral_correlation(ref_samples, impl_samples, fft_size);
    let avg_diff = spectral_difference(ref_samples, impl_samples, fft_size);

    println!("Spectral Metrics");
    println!("----------------");
    println!("  Correlation:        {:.4}", correlation);
    println!("  Avg magnitude diff: {:.2} dB", avg_diff);
    println!();

    // Frequency band analysis
    if args.detailed {
        println!("Frequency Band Analysis");
        println!("-----------------------");

        let bands = [
            ("Sub bass", 20.0, 60.0),
            ("Bass", 60.0, 250.0),
            ("Low mids", 250.0, 500.0),
            ("Mids", 500.0, 2000.0),
            ("High mids", 2000.0, 4000.0),
            ("Presence", 4000.0, 6000.0),
            ("Brilliance", 6000.0, 20000.0),
        ];

        let bin_width = sample_rate / fft_size as f32;

        // Compute spectra for band analysis
        let ref_spectrum = compute_average_spectrum(ref_samples, fft_size);
        let impl_spectrum = compute_average_spectrum(impl_samples, fft_size);

        println!(
            "  {:12}  {:>8}  {:>8}  {:>8}",
            "Band", "Ref (dB)", "Impl (dB)", "Diff"
        );
        println!(
            "  {:12}  {:>8}  {:>8}  {:>8}",
            "----", "--------", "---------", "----"
        );

        for (name, low, high) in bands {
            let start_bin = (low / bin_width).floor() as usize;
            let end_bin = (high / bin_width).ceil() as usize;

            if start_bin >= ref_spectrum.len() {
                continue;
            }
            let end_bin = end_bin.min(ref_spectrum.len());

            let ref_band: f32 =
                ref_spectrum[start_bin..end_bin].iter().sum::<f32>() / (end_bin - start_bin) as f32;
            let impl_band: f32 = impl_spectrum[start_bin..end_bin].iter().sum::<f32>()
                / (end_bin - start_bin) as f32;

            let ref_db = linear_to_db(ref_band);
            let impl_db = linear_to_db(impl_band);
            let diff = impl_db - ref_db;

            println!(
                "  {:12}  {:>8.1}  {:>8.1}  {:>+8.1}",
                name, ref_db, impl_db, diff
            );
        }
        println!();
    }

    // Summary
    println!("Summary");
    println!("-------");

    let match_quality = if correlation > 0.99 && snr_val > 40.0 {
        "Excellent"
    } else if correlation > 0.95 && snr_val > 30.0 {
        "Good"
    } else if correlation > 0.90 && snr_val > 20.0 {
        "Fair"
    } else {
        "Poor"
    };

    println!("  Match quality: {}", match_quality);
    println!(
        "  The implementation {} the reference.",
        if correlation > 0.95 {
            "closely matches"
        } else if correlation > 0.85 {
            "moderately matches"
        } else {
            "differs significantly from"
        }
    );

    // Write JSON report if requested
    if let Some(output_path) = args.output {
        let report = serde_json::json!({
            "reference": args.reference.to_string_lossy(),
            "implementation": args.implementation.to_string_lossy(),
            "sample_rate": sample_rate,
            "length_samples": len,
            "duration_seconds": len as f32 / sample_rate,
            "time_domain": {
                "mse": mse_val,
                "rmse": rmse_val,
                "snr_db": snr_val,
                "reference_rms_db": linear_to_db(ref_rms),
                "implementation_rms_db": linear_to_db(impl_rms),
                "reference_peak_db": linear_to_db(ref_peak),
                "implementation_peak_db": linear_to_db(impl_peak),
            },
            "spectral": {
                "correlation": correlation,
                "average_magnitude_diff_db": avg_diff,
                "fft_size": fft_size,
            },
            "summary": {
                "match_quality": match_quality,
            }
        });

        std::fs::write(&output_path, serde_json::to_string_pretty(&report)?)?;
        println!("\nWrote detailed report to {}", output_path.display());
    }

    Ok(())
}

fn compute_average_spectrum(samples: &[f32], fft_size: usize) -> Vec<f32> {
    let hop_size = fft_size / 2;
    let num_frames = (samples.len().saturating_sub(fft_size)) / hop_size + 1;

    let fft = Fft::new(fft_size);
    let window = Window::Blackman;

    if num_frames == 0 {
        // File too short, just zero-pad
        let mut padded = samples.to_vec();
        padded.resize(fft_size, 0.0);
        window.apply(&mut padded);
        let spectrum = fft.forward(&padded);
        return spectrum
            .iter()
            .take(fft_size / 2)
            .map(|c| (c.re * c.re + c.im * c.im).sqrt())
            .collect();
    }

    let mut avg_spectrum = vec![0.0f32; fft_size / 2];

    for i in 0..num_frames {
        let start = i * hop_size;
        let end = start + fft_size;
        if end > samples.len() {
            break;
        }

        let mut frame: Vec<f32> = samples[start..end].to_vec();
        window.apply(&mut frame);
        let spectrum = fft.forward(&frame);

        for (j, c) in spectrum.iter().take(fft_size / 2).enumerate() {
            avg_spectrum[j] += (c.re * c.re + c.im * c.im).sqrt();
        }
    }

    for val in &mut avg_spectrum {
        *val /= num_frames as f32;
    }

    avg_spectrum
}
