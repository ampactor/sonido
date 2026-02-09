//! Spectral analysis commands.

use clap::{Args, Subcommand};
use rustfft::num_complex::Complex;
use sonido_analysis::export::{export_distortion_json, export_frd, export_spectrogram_csv};
use sonido_analysis::{Chromagram, ConstantQTransform, CqtSpectrogram, ImdAnalyzer};
use sonido_analysis::{
    Comodulogram, FilterBank, FrequencyBand, HilbertTransform, PacAnalyzer, PacMethod,
};
use sonido_analysis::{Fft, StftAnalyzer, ThdAnalyzer, TransferFunction, Window, welch_psd};
use sonido_io::{WavSpec, read_wav, write_wav};
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

        /// Use Welch's method for noise reduction
        #[arg(long)]
        welch: bool,

        /// Overlap ratio for Welch's method (0.0-1.0)
        #[arg(long, default_value = "0.5")]
        overlap: f32,
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

        /// Output JSON or FRD file (optional, format detected from extension)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include group delay in output
        #[arg(long)]
        group_delay: bool,

        /// Apply 1/N octave smoothing (e.g., 3 for 1/3 octave)
        #[arg(long)]
        smooth: Option<f32>,
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

        /// Estimate and display RT60 reverberation time
        #[arg(long)]
        rt60: bool,
    },

    /// Analyze harmonic distortion (THD, THD+N)
    Distortion {
        /// Input WAV file (should contain a test tone)
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Fundamental frequency in Hz (auto-detected if not specified)
        #[arg(long)]
        fundamental: Option<f32>,

        /// FFT size
        #[arg(long, default_value = "8192")]
        fft_size: usize,

        /// Output JSON file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate time-frequency spectrogram
    Spectrogram {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// FFT size
        #[arg(long, default_value = "2048")]
        fft_size: usize,

        /// Hop size (defaults to fft_size / 4)
        #[arg(long)]
        hop: Option<usize>,

        /// Output CSV file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Analyze dynamics (RMS, crest factor, dynamic range)
    Dynamics {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,
    },

    /// Analyze Phase-Amplitude Coupling (PAC) between frequency bands
    Pac {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Phase band lower frequency (Hz)
        #[arg(long, default_value = "4.0")]
        phase_low: f32,

        /// Phase band upper frequency (Hz)
        #[arg(long, default_value = "8.0")]
        phase_high: f32,

        /// Amplitude band lower frequency (Hz)
        #[arg(long, default_value = "30.0")]
        amp_low: f32,

        /// Amplitude band upper frequency (Hz)
        #[arg(long, default_value = "100.0")]
        amp_high: f32,

        /// PAC computation method (mvl = Mean Vector Length, kl = Kullback-Leibler)
        #[arg(long, default_value = "mvl")]
        method: String,

        /// Number of surrogate iterations for significance testing (0 = disabled)
        #[arg(long, default_value = "0")]
        surrogates: usize,

        /// Output JSON file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Compute comodulogram (PAC across multiple frequency pairs)
    Comodulogram {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Phase frequency range as "low-high" (Hz)
        #[arg(long, default_value = "2-20")]
        phase_range: String,

        /// Amplitude frequency range as "low-high" (Hz)
        #[arg(long, default_value = "20-200")]
        amp_range: String,

        /// Phase frequency step (Hz)
        #[arg(long, default_value = "2.0")]
        phase_step: f32,

        /// Amplitude frequency step (Hz)
        #[arg(long, default_value = "10.0")]
        amp_step: f32,

        /// Bandwidth ratio (fraction of center frequency)
        #[arg(long, default_value = "0.5")]
        bandwidth: f32,

        /// Output CSV file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Extract a frequency band using bandpass filtering
    Bandpass {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Lower cutoff frequency (Hz)
        #[arg(long)]
        low: f32,

        /// Upper cutoff frequency (Hz)
        #[arg(long)]
        high: f32,

        /// Filter order (2, 4, or 6)
        #[arg(long, default_value = "4")]
        order: u32,

        /// Output WAV file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Extract instantaneous phase and amplitude using Hilbert transform
    Hilbert {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Output WAV file for phase (optional)
        #[arg(long)]
        phase_output: Option<PathBuf>,

        /// Output WAV file for amplitude/envelope (optional)
        #[arg(long)]
        amp_output: Option<PathBuf>,

        /// Optional bandpass filter before Hilbert transform (as "low-high" Hz)
        #[arg(long)]
        bandpass: Option<String>,
    },

    /// Analyze Intermodulation Distortion (IMD) using two-tone test
    Imd {
        /// Input WAV file (should contain a two-tone test signal)
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// First tone frequency in Hz
        #[arg(long)]
        freq1: f32,

        /// Second tone frequency in Hz
        #[arg(long)]
        freq2: f32,

        /// FFT size
        #[arg(long, default_value = "8192")]
        fft_size: usize,

        /// Output JSON file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Constant-Q Transform analysis (logarithmic frequency resolution)
    Cqt {
        /// Input WAV file
        #[arg(value_name = "INPUT")]
        input: PathBuf,

        /// Minimum frequency (Hz)
        #[arg(long, default_value = "32.7")]
        min_freq: f32,

        /// Maximum frequency (Hz, defaults to Nyquist/2)
        #[arg(long)]
        max_freq: Option<f32>,

        /// Bins per octave (12 for semitone resolution, 24 for quarter-tone)
        #[arg(long, default_value = "12")]
        bins_per_octave: usize,

        /// Output CSV file (optional)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Also compute chromagram (pitch class profile)
        #[arg(long)]
        chromagram: bool,

        /// Show top N peaks
        #[arg(long, default_value = "10")]
        peaks: usize,
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
            welch,
            overlap,
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

            let (frequencies, db) = if welch {
                println!("  Using Welch's method (overlap: {:.0}%)", overlap * 100.0);
                welch_psd(&samples, sample_rate, fft_size, overlap, window_fn)
            } else {
                // Take a chunk from the middle of the file
                let start = samples.len().saturating_sub(fft_size) / 2;
                let mut chunk: Vec<f32> =
                    samples.iter().skip(start).take(fft_size).copied().collect();

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
                    .map(|m| if *m > 0.0 { 20.0 * m.log10() } else { -120.0 })
                    .collect();

                let frequencies: Vec<f32> = (0..db.len())
                    .map(|i| i as f32 * sample_rate / fft_size as f32)
                    .collect();

                (frequencies, db)
            };

            // Find peaks
            let mut indexed: Vec<(usize, f32)> = db.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            println!("\nTop {} frequency peaks:", peaks);
            println!("  {:>10}  {:>8}", "Freq (Hz)", "Level (dB)");
            println!("  {:>10}  {:>8}", "--------", "----------");
            for (i, level) in indexed.iter().take(peaks) {
                let freq = frequencies.get(*i).copied().unwrap_or(0.0);
                println!("  {:>10.1}  {:>8.1}", freq, level);
            }

            // Write CSV if requested
            if let Some(output_path) = output {
                let mut csv = String::new();
                csv.push_str("frequency_hz,magnitude_db\n");
                for (freq, level) in frequencies.iter().zip(db.iter()) {
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
            group_delay,
            smooth,
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

            let mut result = TransferFunction::measure(
                &input_samples[..len],
                &output_samples[..len],
                sample_rate,
                fft_size,
                0.5, // 50% overlap
            );

            // Apply smoothing if requested
            if let Some(octave_fraction) = smooth {
                let window_size = (octave_fraction as usize).max(3);
                println!("  Applying smoothing (window size: {})", window_size);
                result = result.smooth(window_size);
            }

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

            // Show group delay if requested
            if group_delay {
                let gd = result.group_delay();
                let gd_ms: Vec<f32> = gd.iter().map(|&s| s * 1000.0 / sample_rate).collect();

                // Find average group delay in the mid-frequency range
                let mut mid_gd = 0.0f32;
                let mut mid_gd_count = 0;
                for (i, &gd_val) in gd_ms.iter().enumerate() {
                    let freq = result.frequencies[i];
                    if (300.0..=3000.0).contains(&freq) && gd_val.is_finite() {
                        mid_gd += gd_val;
                        mid_gd_count += 1;
                    }
                }
                if mid_gd_count > 0 {
                    mid_gd /= mid_gd_count as f32;
                }
                println!("\n  Group delay (300-3k Hz avg): {:.2} ms", mid_gd);
            }

            // Write output file if requested
            if let Some(output_path) = output {
                let ext = output_path
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or("");

                if ext.eq_ignore_ascii_case("frd") {
                    export_frd(&result, &output_path)?;
                    println!("\nWrote FRD to {}", output_path.display());
                } else {
                    // Default to JSON
                    let mut json = serde_json::json!({
                        "fft_size": fft_size,
                        "sample_rate": sample_rate,
                        "num_bins": result.magnitude_db.len(),
                        "frequencies": result.frequencies,
                        "magnitude_db": result.magnitude_db,
                        "phase_rad": result.phase_rad,
                        "coherence": result.coherence,
                    });

                    if group_delay {
                        let gd = result.group_delay();
                        json["group_delay_samples"] = serde_json::json!(gd);
                    }

                    std::fs::write(&output_path, serde_json::to_string_pretty(&json)?)?;
                    println!("\nWrote transfer function to {}", output_path.display());
                }
            }
        }

        AnalyzeCommand::Ir {
            sweep,
            response,
            output,
            rt60,
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
            let fft_size = sweep_samples
                .len()
                .max(response_samples.len())
                .next_power_of_two();

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

            // Estimate RT60 if requested
            if rt60 {
                use sonido_analysis::estimate_rt60;
                if let Some(rt60_result) = estimate_rt60(trimmed, sample_rate) {
                    println!("\n  Reverberation time estimates:");
                    println!("    RT60 (T30): {:.3}s", rt60_result.rt60_seconds);
                    println!("    T20:        {:.3}s", rt60_result.t20_seconds);
                    println!("    EDT:        {:.3}s", rt60_result.edt_seconds);
                    println!("    Correlation: {:.3}", rt60_result.correlation);
                } else {
                    println!("\n  Could not estimate RT60 (insufficient decay)");
                }
            }

            let spec = sonido_io::WavSpec {
                channels: 1,
                sample_rate: sweep_spec.sample_rate,
                bits_per_sample: 32,
            };

            sonido_io::write_wav(&output, trimmed, spec)?;
            println!("  Wrote IR to {}", output.display());
        }

        AnalyzeCommand::Distortion {
            input,
            fundamental,
            fft_size,
            output,
        } => {
            println!("Analyzing distortion of {}...", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            let analyzer = ThdAnalyzer::new(sample_rate, fft_size);

            let result = if let Some(fund_hz) = fundamental {
                println!("  Fundamental: {:.1} Hz (specified)", fund_hz);
                analyzer.analyze(&samples, fund_hz)
            } else {
                println!("  Fundamental: auto-detecting...");
                analyzer.analyze_auto(&samples)
            };

            let fundamental_db = if result.fundamental_amplitude > 0.0 {
                20.0 * result.fundamental_amplitude.log10()
            } else {
                -120.0
            };

            println!("\nDistortion Analysis:");
            println!(
                "  Fundamental: {:.1} Hz at {:.1} dB",
                result.fundamental_freq, fundamental_db
            );
            println!(
                "  THD:         {:.4}% ({:.1} dB)",
                result.thd_ratio * 100.0,
                result.thd_db
            );
            println!(
                "  THD+N:       {:.4}% ({:.1} dB)",
                result.thd_n_ratio * 100.0,
                result.thd_n_db
            );
            println!("  Noise floor: {:.6} (linear RMS)", result.noise_floor);

            println!("\n  Harmonics:");
            for (i, &amp) in result.harmonics.iter().enumerate().skip(1).take(5) {
                if amp > 0.0 {
                    let freq = result.fundamental_freq * (i + 1) as f32;
                    let db = 20.0 * amp.log10();
                    println!("    H{}: {:.1} Hz at {:.1} dB", i + 1, freq, db);
                }
            }

            if let Some(output_path) = output {
                export_distortion_json(&result, &output_path)?;
                println!("\nWrote distortion analysis to {}", output_path.display());
            }
        }

        AnalyzeCommand::Spectrogram {
            input,
            fft_size,
            hop,
            output,
        } => {
            println!("Computing spectrogram of {}...", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;
            let hop_size = hop.unwrap_or(fft_size / 4);

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );
            println!("  FFT size: {}, hop: {}", fft_size, hop_size);

            let analyzer = StftAnalyzer::new(sample_rate, fft_size, hop_size, Window::Hann);
            let spectrogram = analyzer.analyze(&samples);

            println!(
                "\n  Spectrogram: {} frames x {} bins",
                spectrogram.num_frames, spectrogram.num_bins
            );
            println!(
                "  Time resolution: {:.1} ms",
                hop_size as f32 / sample_rate * 1000.0
            );
            println!(
                "  Frequency resolution: {:.1} Hz",
                sample_rate / fft_size as f32
            );

            export_spectrogram_csv(&spectrogram, &output, true)?;
            println!("\nWrote spectrogram to {}", output.display());
        }

        AnalyzeCommand::Dynamics { input } => {
            println!("Analyzing dynamics of {}...", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            use sonido_analysis::{analyze_dynamics, crest_factor_db, peak_db, rms_db};

            let rms = rms_db(&samples);
            let peak = peak_db(&samples);
            let crest = crest_factor_db(&samples);

            // Use ~10ms window size for dynamics analysis
            let window_size = (sample_rate * 0.01) as usize;
            let dynamics = analyze_dynamics(&samples, window_size, -60.0);

            println!("\nDynamics Analysis:");
            println!("  Peak level:     {:.1} dBFS", peak);
            println!("  RMS level:      {:.1} dBFS", rms);
            println!("  Crest factor:   {:.1} dB", crest);
            println!("  Dynamic range:  {:.1} dB", dynamics.dynamic_range_db);
            println!("  Headroom:       {:.1} dB", -peak);
        }

        AnalyzeCommand::Pac {
            input,
            phase_low,
            phase_high,
            amp_low,
            amp_high,
            method,
            surrogates,
            output,
        } => {
            println!("Analyzing Phase-Amplitude Coupling...");
            println!("  Input: {}", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            // Validate frequency bands
            if phase_high <= phase_low {
                anyhow::bail!(
                    "Phase band high ({}) must be greater than low ({})",
                    phase_high,
                    phase_low
                );
            }
            if amp_high <= amp_low {
                anyhow::bail!(
                    "Amplitude band high ({}) must be greater than low ({})",
                    amp_high,
                    amp_low
                );
            }
            if phase_high >= amp_low {
                anyhow::bail!(
                    "Phase band ({}-{} Hz) must not overlap with amplitude band ({}-{} Hz)",
                    phase_low,
                    phase_high,
                    amp_low,
                    amp_high
                );
            }

            let phase_band = FrequencyBand::new("phase", phase_low, phase_high);
            let amplitude_band = FrequencyBand::new("amplitude", amp_low, amp_high);

            println!("\n  Phase band:     {:.1}-{:.1} Hz", phase_low, phase_high);
            println!("  Amplitude band: {:.1}-{:.1} Hz", amp_low, amp_high);

            let pac_method = match method.to_lowercase().as_str() {
                "mvl" | "mean_vector_length" => PacMethod::MeanVectorLength,
                "kl" | "kullback_leibler" => PacMethod::KullbackLeibler,
                _ => {
                    eprintln!("Unknown method '{}', using Mean Vector Length", method);
                    PacMethod::MeanVectorLength
                }
            };

            println!("  Method: {:?}", pac_method);

            let mut analyzer = PacAnalyzer::new(sample_rate, phase_band, amplitude_band);
            analyzer.set_method(pac_method);

            let result = analyzer.analyze(&samples);

            println!("\nPhase-Amplitude Coupling Results:");
            println!("  Modulation Index: {:.6}", result.modulation_index);
            println!(
                "  Preferred Phase:  {:.2} rad ({:.1} deg)",
                result.preferred_phase,
                result.preferred_phase_degrees()
            );

            // Surrogate significance testing
            let mut p_value = None;
            if surrogates > 0 {
                println!("\n  Running {} surrogate iterations...", surrogates);
                let mut surrogate_mis: Vec<f32> = Vec::with_capacity(surrogates);

                for _ in 0..surrogates {
                    // Create time-shifted surrogate (random circular shift)
                    let shift = (rand_simple() * samples.len() as f64) as usize;
                    let mut surrogate: Vec<f32> = samples[shift..].to_vec();
                    surrogate.extend_from_slice(&samples[..shift]);

                    let sur_result = analyzer.analyze(&surrogate);
                    surrogate_mis.push(sur_result.modulation_index);
                }

                // Calculate p-value: proportion of surrogates with MI >= observed MI
                let count_greater = surrogate_mis
                    .iter()
                    .filter(|&&mi| mi >= result.modulation_index)
                    .count();
                let p = count_greater as f32 / surrogates as f32;
                p_value = Some(p);

                let mean_sur: f32 = surrogate_mis.iter().sum::<f32>() / surrogates as f32;
                let std_sur: f32 = (surrogate_mis
                    .iter()
                    .map(|mi| (mi - mean_sur).powi(2))
                    .sum::<f32>()
                    / surrogates as f32)
                    .sqrt();

                println!("  Surrogate MI:     {:.6} +/- {:.6}", mean_sur, std_sur);
                println!("  p-value:          {:.4}", p);
                println!(
                    "  Significant:      {}",
                    if p < 0.05 { "Yes (p < 0.05)" } else { "No" }
                );
            }

            // Print phase-amplitude histogram
            println!("\n  Amplitude by phase bin (18 bins x 20 deg):");
            let bin_width = 360.0 / 18.0;
            for (i, &amp) in result.mean_amplitude_per_phase.iter().enumerate() {
                let phase_start = -180.0 + i as f32 * bin_width;
                let bar_len = ((amp
                    / result
                        .mean_amplitude_per_phase
                        .iter()
                        .copied()
                        .fold(f32::NEG_INFINITY, f32::max))
                    * 20.0) as usize;
                let bar = "#".repeat(bar_len);
                println!(
                    "    {:>4.0} - {:>4.0} deg: {:.4} {}",
                    phase_start,
                    phase_start + bin_width,
                    amp,
                    bar
                );
            }

            // Write output JSON if requested
            if let Some(output_path) = output {
                let json = serde_json::json!({
                    "phase_band": {
                        "low_hz": phase_low,
                        "high_hz": phase_high,
                    },
                    "amplitude_band": {
                        "low_hz": amp_low,
                        "high_hz": amp_high,
                    },
                    "method": format!("{:?}", pac_method),
                    "modulation_index": result.modulation_index,
                    "preferred_phase_rad": result.preferred_phase,
                    "preferred_phase_deg": result.preferred_phase_degrees(),
                    "mean_amplitude_per_phase": result.mean_amplitude_per_phase,
                    "p_value": p_value,
                    "surrogates": surrogates,
                });

                std::fs::write(&output_path, serde_json::to_string_pretty(&json)?)?;
                println!("\nWrote PAC analysis to {}", output_path.display());
            }
        }

        AnalyzeCommand::Comodulogram {
            input,
            phase_range,
            amp_range,
            phase_step,
            amp_step,
            bandwidth,
            output,
        } => {
            println!("Computing comodulogram...");
            println!("  Input: {}", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            // Parse frequency ranges
            let parse_range = |s: &str| -> anyhow::Result<(f32, f32)> {
                let parts: Vec<&str> = s.split('-').collect();
                if parts.len() != 2 {
                    anyhow::bail!("Invalid range format '{}', expected 'low-high'", s);
                }
                let low: f32 = parts[0].parse()?;
                let high: f32 = parts[1].parse()?;
                Ok((low, high))
            };

            let (phase_min, phase_max) = parse_range(&phase_range)?;
            let (amp_min, amp_max) = parse_range(&amp_range)?;

            println!(
                "\n  Phase range:     {:.1}-{:.1} Hz (step: {:.1})",
                phase_min, phase_max, phase_step
            );
            println!(
                "  Amplitude range: {:.1}-{:.1} Hz (step: {:.1})",
                amp_min, amp_max, amp_step
            );
            println!("  Bandwidth ratio: {:.2}", bandwidth);

            let como = Comodulogram::compute(
                &samples,
                sample_rate,
                (phase_min, phase_max, phase_step),
                (amp_min, amp_max, amp_step),
                bandwidth,
            );

            let (peak_phase, peak_amp, peak_mi) = como.peak_coupling();

            println!(
                "\n  Comodulogram size: {} x {} = {} cells",
                como.phase_frequencies.len(),
                como.amplitude_frequencies.len(),
                como.phase_frequencies.len() * como.amplitude_frequencies.len()
            );

            println!("\n  Peak coupling:");
            println!("    Phase frequency:     {:.1} Hz", peak_phase);
            println!("    Amplitude frequency: {:.1} Hz", peak_amp);
            println!("    Modulation index:    {:.6}", peak_mi);

            // Write CSV
            let csv = como.to_csv();
            std::fs::write(&output, csv)?;
            println!("\nWrote comodulogram to {}", output.display());
        }

        AnalyzeCommand::Bandpass {
            input,
            low,
            high,
            order,
            output,
        } => {
            println!("Extracting frequency band...");
            println!("  Input: {}", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            if high <= low {
                anyhow::bail!(
                    "High frequency ({}) must be greater than low ({})",
                    high,
                    low
                );
            }

            if order != 2 && order != 4 && order != 6 {
                anyhow::bail!("Filter order must be 2, 4, or 6 (got {})", order);
            }

            println!("\n  Bandpass: {:.1}-{:.1} Hz (order: {})", low, high, order);

            let band = FrequencyBand::new("bandpass", low, high);
            let mut filter_bank = FilterBank::new(sample_rate, &[band]);

            // Apply filter multiple times for higher orders (each FilterBank is 4th order)
            let mut filtered = samples.clone();
            let passes = order / 2;
            for _ in 0..passes {
                let result = filter_bank.extract(&filtered);
                filtered = result.into_iter().next().unwrap();
            }

            // Normalize to prevent clipping
            let peak = filtered.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            if peak > 1.0 {
                for s in &mut filtered {
                    *s /= peak;
                }
                println!("  Normalized (peak was {:.2})", peak);
            }

            let out_spec = WavSpec {
                channels: 1,
                sample_rate: spec.sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &filtered, out_spec)?;
            println!("\nWrote filtered signal to {}", output.display());
        }

        AnalyzeCommand::Hilbert {
            input,
            phase_output,
            amp_output,
            bandpass,
        } => {
            println!("Computing Hilbert transform...");
            println!("  Input: {}", input.display());

            let (mut samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            // Apply bandpass filter if specified
            if let Some(bp_range) = &bandpass {
                let parts: Vec<&str> = bp_range.split('-').collect();
                if parts.len() != 2 {
                    anyhow::bail!(
                        "Invalid bandpass format '{}', expected 'low-high'",
                        bp_range
                    );
                }
                let low: f32 = parts[0].parse()?;
                let high: f32 = parts[1].parse()?;

                println!("  Pre-filtering: {:.1}-{:.1} Hz", low, high);

                let band = FrequencyBand::new("bandpass", low, high);
                let mut filter_bank = FilterBank::new(sample_rate, &[band]);
                let result = filter_bank.extract(&samples);
                samples = result.into_iter().next().unwrap();
            }

            // Compute Hilbert transform
            let fft_size = samples.len().next_power_of_two();
            let hilbert = HilbertTransform::new(fft_size);
            let (phase, amplitude) = hilbert.phase_and_amplitude(&samples);

            // Print statistics
            let amp_mean: f32 = amplitude.iter().sum::<f32>() / amplitude.len() as f32;
            let amp_max = amplitude.iter().copied().fold(f32::NEG_INFINITY, f32::max);
            let amp_min = amplitude.iter().copied().fold(f32::INFINITY, f32::min);

            println!("\n  Amplitude envelope:");
            println!("    Mean: {:.4}", amp_mean);
            println!("    Min:  {:.4}", amp_min);
            println!("    Max:  {:.4}", amp_max);

            let out_spec = WavSpec {
                channels: 1,
                sample_rate: spec.sample_rate,
                bits_per_sample: 32,
            };

            // Write phase output
            if let Some(ref phase_path) = phase_output {
                // Normalize phase from [-PI, PI] to [-1, 1]
                let phase_normalized: Vec<f32> =
                    phase.iter().map(|&p| p / std::f32::consts::PI).collect();

                write_wav(phase_path, &phase_normalized, out_spec)?;
                println!("\nWrote phase to {}", phase_path.display());
            }

            // Write amplitude output
            if let Some(ref amp_path) = amp_output {
                // Normalize amplitude to [0, 1]
                let amp_normalized: Vec<f32> = if amp_max > 0.0 {
                    amplitude.iter().map(|&a| a / amp_max).collect()
                } else {
                    amplitude.clone()
                };

                write_wav(amp_path, &amp_normalized, out_spec)?;
                println!("Wrote amplitude envelope to {}", amp_path.display());
            }

            if phase_output.is_none() && amp_output.is_none() {
                println!("\n  Note: Use --phase-output and/or --amp-output to save results");
            }
        }

        AnalyzeCommand::Imd {
            input,
            freq1,
            freq2,
            fft_size,
            output,
        } => {
            println!("Analyzing Intermodulation Distortion...");
            println!("  Input: {}", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            // Validate frequencies
            let nyquist = sample_rate / 2.0;
            if freq1 >= nyquist || freq2 >= nyquist {
                anyhow::bail!("Frequencies must be below Nyquist ({:.0} Hz)", nyquist);
            }
            if freq1 == freq2 {
                anyhow::bail!("Two-tone test requires different frequencies");
            }

            let (f1, f2) = if freq1 < freq2 {
                (freq1, freq2)
            } else {
                (freq2, freq1)
            };

            println!("\n  Test tones: {:.1} Hz and {:.1} Hz", f1, f2);

            let analyzer = ImdAnalyzer::new(sample_rate, fft_size);
            let result = analyzer.analyze(&samples, f1, f2);

            // Convert to dB
            let amp1_db = if result.amp1 > 0.0 {
                20.0 * result.amp1.log10()
            } else {
                -120.0
            };
            let amp2_db = if result.amp2 > 0.0 {
                20.0 * result.amp2.log10()
            } else {
                -120.0
            };

            println!("\nIMD Analysis Results:");
            println!("  Fundamental tones:");
            println!("    f1 = {:.1} Hz at {:.1} dB", f1, amp1_db);
            println!("    f2 = {:.1} Hz at {:.1} dB", f2, amp2_db);

            println!("\n  Second-order products:");
            let imd2_diff_db = if result.imd2_diff > 0.0 {
                20.0 * result.imd2_diff.log10()
            } else {
                -120.0
            };
            let imd2_sum_db = if result.imd2_sum > 0.0 {
                20.0 * result.imd2_sum.log10()
            } else {
                -120.0
            };
            println!("    f2-f1 = {:.1} Hz at {:.1} dB", f2 - f1, imd2_diff_db);
            println!("    f1+f2 = {:.1} Hz at {:.1} dB", f1 + f2, imd2_sum_db);

            println!("\n  Third-order products:");
            let imd3_low_db = if result.imd3_low > 0.0 {
                20.0 * result.imd3_low.log10()
            } else {
                -120.0
            };
            let imd3_high_db = if result.imd3_high > 0.0 {
                20.0 * result.imd3_high.log10()
            } else {
                -120.0
            };
            println!(
                "    2f1-f2 = {:.1} Hz at {:.1} dB",
                2.0 * f1 - f2,
                imd3_low_db
            );
            println!(
                "    2f2-f1 = {:.1} Hz at {:.1} dB",
                2.0 * f2 - f1,
                imd3_high_db
            );

            println!(
                "\n  IMD ratio: {:.4}% ({:.1} dB)",
                result.imd_ratio * 100.0,
                result.imd_db
            );

            // Write output JSON if requested
            if let Some(output_path) = output {
                let json = serde_json::json!({
                    "freq1": f1,
                    "freq2": f2,
                    "amp1": result.amp1,
                    "amp2": result.amp2,
                    "imd2_diff": result.imd2_diff,
                    "imd2_sum": result.imd2_sum,
                    "imd3_low": result.imd3_low,
                    "imd3_high": result.imd3_high,
                    "imd_ratio": result.imd_ratio,
                    "imd_db": result.imd_db,
                    "products": {
                        "f2_minus_f1": f2 - f1,
                        "f1_plus_f2": f1 + f2,
                        "2f1_minus_f2": 2.0 * f1 - f2,
                        "2f2_minus_f1": 2.0 * f2 - f1,
                    }
                });

                std::fs::write(&output_path, serde_json::to_string_pretty(&json)?)?;
                println!("\nWrote IMD analysis to {}", output_path.display());
            }
        }

        AnalyzeCommand::Cqt {
            input,
            min_freq,
            max_freq,
            bins_per_octave,
            output,
            chromagram,
            peaks,
        } => {
            println!("Computing Constant-Q Transform...");
            println!("  Input: {}", input.display());

            let (samples, spec) = read_wav(&input)?;
            let sample_rate = spec.sample_rate as f32;

            println!(
                "  {} samples, {} Hz, {:.2}s",
                samples.len(),
                spec.sample_rate,
                samples.len() as f32 / sample_rate
            );

            // Use max_freq or default to Nyquist/2
            let max_f = max_freq.unwrap_or((sample_rate / 2.0).min(8000.0));

            println!("\n  Frequency range: {:.1} Hz to {:.1} Hz", min_freq, max_f);
            println!("  Bins per octave: {}", bins_per_octave);

            let cqt = ConstantQTransform::new(sample_rate, min_freq, max_f, bins_per_octave);
            let result = cqt.analyze(&samples);

            println!("\n  CQT bins: {}", result.magnitudes.len());

            // Find peaks
            let mut indexed: Vec<(usize, f32)> =
                result.magnitudes.iter().copied().enumerate().collect();
            indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

            let magnitude_db = result.magnitude_db();
            let midi_notes = result.midi_notes();

            println!("\nTop {} frequency peaks:", peaks);
            println!(
                "  {:>10}  {:>8}  {:>8}  {:>8}",
                "Freq (Hz)", "MIDI", "Note", "Level (dB)"
            );
            println!(
                "  {:>10}  {:>8}  {:>8}  {:>8}",
                "--------", "----", "----", "----------"
            );

            for (i, _) in indexed.iter().take(peaks) {
                let freq = result.frequencies.get(*i).copied().unwrap_or(0.0);
                let midi = midi_notes.get(*i).copied().unwrap_or(0.0);
                let db = magnitude_db.get(*i).copied().unwrap_or(-120.0);
                let note_name = midi_to_note_name(midi);
                println!(
                    "  {:>10.1}  {:>8.1}  {:>8}  {:>8.1}",
                    freq, midi, note_name, db
                );
            }

            // Peak frequency
            if let Some(peak_freq) = result.peak_frequency() {
                let peak_midi = 69.0 + 12.0 * (peak_freq / 440.0).log2();
                println!(
                    "\n  Peak frequency: {:.1} Hz (MIDI {:.1}, {})",
                    peak_freq,
                    peak_midi,
                    midi_to_note_name(peak_midi)
                );
            }

            // Chromagram if requested
            if chromagram {
                let hop_size = cqt.num_bins().max(256);
                let cqt_spec = CqtSpectrogram::from_signal(&samples, &cqt, hop_size);
                let chroma = Chromagram::from_cqt_spectrogram(&cqt_spec, bins_per_octave);

                println!("\nChromagram (pitch class distribution):");
                let pitch_names = Chromagram::pitch_class_names();

                // Compute average chroma across all frames
                let mut avg_chroma = [0.0f32; 12];
                for frame in &chroma.data {
                    for (i, &val) in frame.iter().enumerate() {
                        avg_chroma[i] += val;
                    }
                }
                let num_frames = chroma.data.len().max(1) as f32;
                for val in &mut avg_chroma {
                    *val /= num_frames;
                }

                // Normalize to max
                let max_chroma = avg_chroma.iter().copied().fold(0.0f32, f32::max);
                if max_chroma > 0.0 {
                    for val in &mut avg_chroma {
                        *val /= max_chroma;
                    }
                }

                for (i, &val) in avg_chroma.iter().enumerate() {
                    let bar_len = (val * 30.0) as usize;
                    let bar = "#".repeat(bar_len);
                    println!("  {:>3}: {:.3} {}", pitch_names[i], val, bar);
                }
            }

            // Write CSV if requested
            if let Some(output_path) = output {
                let mut csv = String::new();
                csv.push_str("frequency_hz,magnitude,magnitude_db,midi_note\n");
                for (i, &freq) in result.frequencies.iter().enumerate() {
                    let mag = result.magnitudes.get(i).copied().unwrap_or(0.0);
                    let db = magnitude_db.get(i).copied().unwrap_or(-120.0);
                    let midi = midi_notes.get(i).copied().unwrap_or(0.0);
                    csv.push_str(&format!("{:.2},{:.6},{:.2},{:.2}\n", freq, mag, db, midi));
                }
                std::fs::write(&output_path, csv)?;
                println!("\nWrote CQT to {}", output_path.display());
            }
        }
    }

    Ok(())
}

/// Convert MIDI note number to note name (e.g., 69 -> "A4")
fn midi_to_note_name(midi: f32) -> String {
    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let midi_rounded = midi.round() as i32;
    if !(0..=127).contains(&midi_rounded) {
        return "---".to_string();
    }
    let note = (midi_rounded % 12) as usize;
    let octave = (midi_rounded / 12) - 1;
    format!("{}{}", note_names[note], octave)
}

/// Simple pseudo-random number generator (for surrogate shuffling)
#[allow(unsafe_code)]
fn rand_simple() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    static mut SEED: u64 = 0;

    unsafe {
        if SEED == 0 {
            SEED = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos() as u64;
        }
        // XorShift64
        SEED ^= SEED << 13;
        SEED ^= SEED >> 7;
        SEED ^= SEED << 17;
        (SEED as f64) / (u64::MAX as f64)
    }
}
