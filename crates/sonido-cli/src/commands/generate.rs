//! Test signal generation command.

use clap::{Args, Subcommand};
use sonido_analysis::SineSweep;
use sonido_io::{write_wav, WavSpec};
use std::path::PathBuf;

#[derive(Args)]
pub struct GenerateArgs {
    #[command(subcommand)]
    command: GenerateCommand,
}

#[derive(Subcommand)]
enum GenerateCommand {
    /// Generate a sine sweep (chirp)
    Sweep {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Start frequency in Hz
        #[arg(long, default_value = "20.0")]
        start: f32,

        /// End frequency in Hz
        #[arg(long, default_value = "20000.0")]
        end: f32,

        /// Duration in seconds
        #[arg(long, default_value = "2.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.8")]
        amplitude: f32,
    },

    /// Generate an impulse
    Impulse {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Length in samples
        #[arg(long, default_value = "48000")]
        length: usize,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Impulse amplitude
        #[arg(long, default_value = "1.0")]
        amplitude: f32,
    },

    /// Generate white noise
    Noise {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Duration in seconds
        #[arg(long, default_value = "1.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.5")]
        amplitude: f32,
    },

    /// Generate a sine tone
    Tone {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Frequency in Hz
        #[arg(long, default_value = "440.0")]
        freq: f32,

        /// Duration in seconds
        #[arg(long, default_value = "1.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.8")]
        amplitude: f32,
    },

    /// Generate silence
    Silence {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Duration in seconds
        #[arg(long, default_value = "1.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,
    },
}

pub fn run(args: GenerateArgs) -> anyhow::Result<()> {
    match args.command {
        GenerateCommand::Sweep {
            output,
            start,
            end,
            duration,
            sample_rate,
            amplitude,
        } => {
            println!("Generating sine sweep...");
            println!("  {} Hz to {} Hz over {:.2}s", start, end, duration);

            let sweep = SineSweep::new(sample_rate as f32, start, end, duration);
            let samples: Vec<f32> = sweep.generate().iter().map(|s| s * amplitude).collect();

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Impulse {
            output,
            length,
            sample_rate,
            amplitude,
        } => {
            println!("Generating impulse...");

            let mut samples = vec![0.0; length];
            if !samples.is_empty() {
                samples[0] = amplitude;
            }

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Noise {
            output,
            duration,
            sample_rate,
            amplitude,
        } => {
            println!("Generating white noise...");
            println!("  {:.2}s at {} Hz", duration, sample_rate);

            let num_samples = (duration * sample_rate as f32) as usize;
            let samples: Vec<f32> = (0..num_samples)
                .map(|_| (rand_f32() * 2.0 - 1.0) * amplitude)
                .collect();

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Tone {
            output,
            freq,
            duration,
            sample_rate,
            amplitude,
        } => {
            println!("Generating sine tone...");
            println!("  {} Hz for {:.2}s", freq, duration);

            let num_samples = (duration * sample_rate as f32) as usize;
            let samples: Vec<f32> = (0..num_samples)
                .map(|i| {
                    let t = i as f32 / sample_rate as f32;
                    (2.0 * std::f32::consts::PI * freq * t).sin() * amplitude
                })
                .collect();

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Silence {
            output,
            duration,
            sample_rate,
        } => {
            println!("Generating silence...");
            println!("  {:.2}s at {} Hz", duration, sample_rate);

            let num_samples = (duration * sample_rate as f32) as usize;
            let samples = vec![0.0; num_samples];

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }
    }

    Ok(())
}

/// Simple PRNG for noise generation (xorshift32)
fn rand_f32() -> f32 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u32> = const { Cell::new(0x12345678) };
    }

    STATE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        state.set(x);
        (x as f32) / (u32::MAX as f32)
    })
}
