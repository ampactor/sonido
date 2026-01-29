//! Spectral analysis commands.

use clap::{Args, Subcommand};
use rustfft::num_complex::Complex;
use sonido_analysis::{Fft, TransferFunction, Window};
use sonido_io::read_wav;
use std::path::PathBuf;

#[derive(Args)]
pub struct AnalyzeArgs {
    #[command(subcommand)]
    command: AnalyzeCommand,
}

#[derive(Subcommand)]
enum AnalyzeCommand {
    /// Compute spectrum of an audio file
    Spectrum {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// FFT size
        #[arg(long, default_value = "4096")]
        fft_size: usize,

        /// Window function
        #[arg(long, default_value = "blackman")]
        window: String,

        /// Output CSV file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Show top N peaks
        #[arg(long, default_value = "10")]
        peaks: usize,
    },

    /// Compute transfer function between two files
    Transfer {
        /// Input (dry) WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output (wet) WAV file
        #[arg(value_name = "OUTPUT_FILE")]
        output_file: PathBuf,

        /// FFT size
        #[arg(long, default_value = "4096")]
        fft_size: usize,

        /// Output JSON file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Extract impulse response from sweep recording
    Ir {
        /// Sweep WAV file (the original sine sweep)
        #[arg(value_name = "SWEEP")]
        sweep: PathBuf,

        /// Response WAV file (recorded through system)
        #[arg(value_name = "RESPONSE")]
        response: PathBuf,

        /// Output impulse response WAV file
        #[arg(short, long)]
        output: PathBuf,
    },
}

pub fn run(args: AnalyzeArgs) -> anyhow::Result<()> {
    match args.command {
        AnalyzeCommand::Spectrum {
            input,
            fft_size,
            window,
            output,
            peaks,
        } => {
            println!("Analyzing spectrum of {}...", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            let window_fn = match window.to_lowercase().as_str() {
                "hamming" => Window::Hamming,
                "blackman" => Window::Blackman,
                "hann" => Window::Hann,
                "rectangular" | "rect" | "none" => Window::Rectangular,
                _ => {
                    eprintln!("Unknown window '{}', using Blackman", window);
                    Window::Blackman
                }
            };

            // Take a chunk from the middle of the file
            let start = samples.len().saturating_sub(fft_size) / 2;
            let mut chunk: Vec<f32> = samples
                .iter()
                .skip(start)
                .take(fft_size)
                .copied()
                .collect();

            // Pad if necessary
            chunk.resize(fft_size, 0.0);

            // Apply window
            window_fn.apply(&mut chunk);

            let fft = Fft::new(fft_size);
            let spectrum = fft.forward(&chunk);

            // Compute magnitude spectrum
            let magnitudes: Vec<f32> = spectrum
                .iter()
                .take(fft_size / 2)
                .map(|c| (c.re * c.re + c.im * c.im).sqrt())
                .collect();

            let db: Vec<f32> = magnitudes
                .iter()
                .map(|m| {
                    if *m > 0.0 {
                        20.0 * m.log10()
                    } else {
                        -120.0
                    }
                })
                .collect();

            // Find peaks
            let mut indexed: Vec<(usize, f32)> = db.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            println!("\nTop {} frequency peaks:", peaks);
            println!("  {:>10}  {:>8}", "Freq (Hz)", "Level (dB)");
            println!("  {:>10}  {:>8}", "--------", "----------");
            for (i, level) in indexed.iter().take(peaks) {
                let freq = *i as f32 * sample_rate / fft_size as f32;
                println!("  {:>10.1}  {:>8.1}", freq, level);
            }

            // Write CSV if requested
            if let Some(output_path) = output {
                let mut csv = String::new();
                csv.push_str("frequency_hz,magnitude_db\n");
                for (i, level) in db.iter().enumerate() {
                    let freq = i as f32 * sample_rate / fft_size as f32;
                    csv.push_str(&format!("{:.2},{:.2}\n", freq, level));
                }
                std::fs::write(&output_path, csv)?;
                println!("\nWrote spectrum to {}", output_path.display());
            }
        }

        AnalyzeCommand::Transfer {
            input,
            output_file,
            fft_size,
            output,
        } => {
            println!("Computing transfer function...");
            println!("  Input:  {}", input.display());
            println!("  Output: {}", output_file.display());

            let (input_samples, input_spec) = read_wav(&input)?;
            let (output_samples, output_spec) = read_wav(&output_file)?;

            if input_spec.sample_rate != output_spec.sample_rate {
                anyhow::bail!(
                    "Sample rate mismatch: {} vs {}",
                    input_spec.sample_rate,
                    output_spec.sample_rate
                );
            }

            let sample_rate = input_spec.sample_rate as f32;
            let len = input_samples.len().min(output_samples.len());

            let result = TransferFunction::measure(
                &input_samples[..len],
                &output_samples[..len],
                sample_rate,
                fft_size,
                0.5, // 50% overlap
            );

            println!("\nTransfer function summary:");
            println!("  Bins: {}", result.magnitude_db.len());

            // Find frequency response characteristics
            let mut low_db = 0.0f32;
            let mut mid_db = 0.0f32;
            let mut high_db = 0.0f32;
            let mut low_count = 0;
            let mut mid_count = 0;
            let mut high_count = 0;

            for (i, &db) in result.magnitude_db.iter().enumerate() {
                let freq = result.frequencies[i];
                if freq < 300.0 {
                    low_db += db;
                    low_count += 1;
                } else if freq < 3000.0 {
                    mid_db += db;
                    mid_count += 1;
                } else if freq < 10000.0 {
                    high_db += db;
                    high_count += 1;
                }
            }

            if low_count > 0 {
                low_db /= low_count as f32;
            }
            if mid_count > 0 {
                mid_db /= mid_count as f32;
            }
            if high_count > 0 {
                high_db /= high_count as f32;
            }

            println!("\n  Average gain by band:");
            println!("    Low  (<300 Hz):   {:>6.1} dB", low_db);
            println!("    Mid  (300-3k Hz): {:>6.1} dB", mid_db);
            println!("    High (3k-10k Hz): {:>6.1} dB", high_db);

            // Write JSON if requested
            if let Some(output_path) = output {
                let json = serde_json::json!({
                    "fft_size": fft_size,
                    "sample_rate": sample_rate,
                    "num_bins": result.magnitude_db.len(),
                    "frequencies": result.frequencies,
                    "magnitude_db": result.magnitude_db,
                    "phase_rad": result.phase_rad,
                    "coherence": result.coherence,
                });
                std::fs::write(&output_path, serde_json::to_string_pretty(&json)?)?;
                println!("\nWrote transfer function to {}", output_path.display());
            }
        }

        AnalyzeCommand::Ir {
            sweep,
            response,
            output,
        } => {
            println!("Extracting impulse response...");
            println!("  Sweep:    {}", sweep.display());
            println!("  Response: {}", response.display());

            let (sweep_samples, sweep_spec) = read_wav(&sweep)?;
            let (response_samples, response_spec) = read_wav(&response)?;

            if sweep_spec.sample_rate != response_spec.sample_rate {
                anyhow::bail!(
                    "Sample rate mismatch: {} vs {}",
                    sweep_spec.sample_rate,
                    response_spec.sample_rate
                );
            }

            let sample_rate = sweep_spec.sample_rate as f32;

            // Use deconvolution to extract IR
            // IR = IFFT(FFT(response) / FFT(sweep))
            let fft_size = sweep_samples.len().max(response_samples.len()).next_power_of_two();

            let mut sweep_padded = sweep_samples.clone();
            let mut response_padded = response_samples.clone();
            sweep_padded.resize(fft_size, 0.0);
            response_padded.resize(fft_size, 0.0);

            let fft = Fft::new(fft_size);
            let sweep_fft = fft.forward(&sweep_padded);
            let response_fft = fft.forward(&response_padded);

            // Divide in frequency domain (with regularization)
            let epsilon = 1e-6;
            let ir_fft: Vec<_> = sweep_fft
                .iter()
                .zip(response_fft.iter())
                .map(|(s, r)| {
                    let mag_sq = s.re * s.re + s.im * s.im + epsilon;
                    let conj = Complex::new(s.re, -s.im);
                    (r * conj) / mag_sq
                })
                .collect();

            // Inverse FFT
            let ir = fft.inverse(&ir_fft);

            // Normalize and trim
            let peak = ir.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            let normalized: Vec<f32> = ir.iter().map(|s| s / peak.max(epsilon)).collect();

            // Find IR length (where it decays to -60dB)
            let threshold = 0.001; // -60dB
            let ir_len = normalized
                .iter()
                .rposition(|s| s.abs() > threshold)
                .unwrap_or(normalized.len())
                + 1;

            let trimmed = &normalized[..ir_len.min(normalized.len())];

            println!(
                "\n  IR length: {} samples ({:.3}s)",
                trimmed.len(),
                trimmed.len() as f32 / sample_rate
            );

            let spec = sonido_io::WavSpec {
                channels: 1,
                sample_rate: sweep_spec.sample_rate,
                bits_per_sample: 32,
            };

            sonido_io::write_wav(&output, trimmed, spec)?;
            println!("  Wrote IR to {}", output.display());
        }
    }

    Ok(())
}
