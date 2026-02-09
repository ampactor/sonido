//! Test signal generation command.

use clap::{Args, Subcommand, ValueEnum};
use sonido_analysis::SineSweep;
use sonido_io::{WavSpec, write_wav};
use sonido_synth::voice::midi_to_freq;
use sonido_synth::{AdsrEnvelope, Oscillator, OscillatorWaveform, PolyphonicSynth};
use std::path::PathBuf;

/// Waveform types for CLI
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum CliWaveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    Noise,
}

impl From<CliWaveform> for OscillatorWaveform {
    fn from(w: CliWaveform) -> Self {
        match w {
            CliWaveform::Sine => OscillatorWaveform::Sine,
            CliWaveform::Triangle => OscillatorWaveform::Triangle,
            CliWaveform::Saw => OscillatorWaveform::Saw,
            CliWaveform::Square => OscillatorWaveform::Square,
            CliWaveform::Noise => OscillatorWaveform::Noise,
        }
    }
}

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

    /// Generate an oscillator waveform using PolyBLEP anti-aliasing
    Osc {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Frequency in Hz
        #[arg(long, default_value = "440.0")]
        freq: f32,

        /// Waveform type
        #[arg(long, value_enum, default_value = "sine")]
        waveform: CliWaveform,

        /// Duration in seconds
        #[arg(long, default_value = "1.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.8")]
        amplitude: f32,

        /// Pulse width (0-1) for pulse wave
        #[arg(long, default_value = "0.5")]
        pulse_width: f32,
    },

    /// Generate a chord using the polyphonic synthesizer
    Chord {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// MIDI notes (comma-separated, e.g., "60,64,67" for C major)
        #[arg(long)]
        notes: String,

        /// Duration in seconds
        #[arg(long, default_value = "2.0")]
        duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.5")]
        amplitude: f32,

        /// Waveform type
        #[arg(long, value_enum, default_value = "saw")]
        waveform: CliWaveform,

        /// Filter cutoff frequency in Hz
        #[arg(long, default_value = "2000.0")]
        filter_cutoff: f32,

        /// Amplitude envelope attack (ms)
        #[arg(long, default_value = "10.0")]
        attack: f32,

        /// Amplitude envelope release (ms)
        #[arg(long, default_value = "500.0")]
        release: f32,
    },

    /// Generate an ADSR envelope test tone
    Adsr {
        /// Output WAV file
        #[arg(value_name = "OUTPUT")]
        output: PathBuf,

        /// Attack time in milliseconds
        #[arg(long, default_value = "50.0")]
        attack: f32,

        /// Decay time in milliseconds
        #[arg(long, default_value = "100.0")]
        decay: f32,

        /// Sustain level (0-1)
        #[arg(long, default_value = "0.7")]
        sustain: f32,

        /// Release time in milliseconds
        #[arg(long, default_value = "200.0")]
        release: f32,

        /// Frequency of test tone in Hz
        #[arg(long, default_value = "440.0")]
        freq: f32,

        /// Gate on duration in seconds (before release)
        #[arg(long, default_value = "1.0")]
        gate_duration: f32,

        /// Sample rate
        #[arg(long, default_value = "48000")]
        sample_rate: u32,

        /// Amplitude (0-1)
        #[arg(long, default_value = "0.8")]
        amplitude: f32,
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

        GenerateCommand::Osc {
            output,
            freq,
            waveform,
            duration,
            sample_rate,
            amplitude,
            pulse_width,
        } => {
            let waveform_name = format!("{:?}", waveform).to_lowercase();
            println!("Generating oscillator waveform...");
            println!("  {} Hz {} for {:.2}s", freq, waveform_name, duration);

            let mut osc = Oscillator::new(sample_rate as f32);
            osc.set_frequency(freq);

            // Handle pulse width for square wave
            let osc_waveform =
                if matches!(waveform, CliWaveform::Square) && (pulse_width - 0.5).abs() > 0.01 {
                    OscillatorWaveform::Pulse(pulse_width)
                } else {
                    waveform.into()
                };
            osc.set_waveform(osc_waveform);

            let num_samples = (duration * sample_rate as f32) as usize;
            let samples: Vec<f32> = (0..num_samples)
                .map(|_| osc.advance() * amplitude)
                .collect();

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Chord {
            output,
            notes,
            duration,
            sample_rate,
            amplitude,
            waveform,
            filter_cutoff,
            attack,
            release,
        } => {
            // Parse MIDI notes
            let midi_notes: Vec<u8> = notes
                .split(',')
                .filter_map(|s| s.trim().parse::<u8>().ok())
                .collect();

            if midi_notes.is_empty() {
                anyhow::bail!("No valid MIDI notes provided. Use format: --notes \"60,64,67\"");
            }

            println!("Generating chord...");
            println!("  Notes: {:?}", midi_notes);
            let freqs: Vec<f32> = midi_notes.iter().map(|&n| midi_to_freq(n)).collect();
            println!(
                "  Frequencies: {:?}",
                freqs
                    .iter()
                    .map(|f| format!("{:.1} Hz", f))
                    .collect::<Vec<_>>()
            );
            println!("  Duration: {:.2}s", duration);

            let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(sample_rate as f32);
            synth.set_osc1_waveform(waveform.into());
            synth.set_filter_cutoff(filter_cutoff);
            synth.set_amp_attack(attack);
            synth.set_amp_release(release);

            // Trigger all notes
            for &note in &midi_notes {
                synth.note_on(note, 100);
            }

            let num_samples = (duration * sample_rate as f32) as usize;
            let gate_off_sample =
                ((duration - release / 1000.0).max(0.1) * sample_rate as f32) as usize;

            let mut samples = Vec::with_capacity(num_samples);
            let mut notes_released = false;

            for i in 0..num_samples {
                // Release notes before the end to allow release envelope
                if !notes_released && i >= gate_off_sample {
                    for &note in &midi_notes {
                        synth.note_off(note);
                    }
                    notes_released = true;
                }

                samples.push(synth.process() * amplitude);
            }

            let spec = WavSpec {
                channels: 1,
                sample_rate,
                bits_per_sample: 32,
            };

            write_wav(&output, &samples, spec)?;
            println!("Wrote {} samples to {}", samples.len(), output.display());
        }

        GenerateCommand::Adsr {
            output,
            attack,
            decay,
            sustain,
            release,
            freq,
            gate_duration,
            sample_rate,
            amplitude,
        } => {
            println!("Generating ADSR envelope test...");
            println!(
                "  A: {:.0}ms, D: {:.0}ms, S: {:.2}, R: {:.0}ms",
                attack, decay, sustain, release
            );
            println!(
                "  Test tone: {:.1} Hz, gate duration: {:.2}s",
                freq, gate_duration
            );

            let mut osc = Oscillator::new(sample_rate as f32);
            osc.set_frequency(freq);
            osc.set_waveform(OscillatorWaveform::Sine);

            let mut env = AdsrEnvelope::new(sample_rate as f32);
            env.set_attack_ms(attack);
            env.set_decay_ms(decay);
            env.set_sustain(sustain);
            env.set_release_ms(release);

            // Calculate total duration: gate + release time (with some margin)
            let total_duration = gate_duration + release / 1000.0 * 5.0; // 5x release time for full decay
            let num_samples = (total_duration * sample_rate as f32) as usize;
            let gate_off_sample = (gate_duration * sample_rate as f32) as usize;

            env.gate_on();

            let mut samples = Vec::with_capacity(num_samples);
            for i in 0..num_samples {
                if i == gate_off_sample {
                    env.gate_off();
                }

                let env_level = env.advance();
                let osc_sample = osc.advance();
                samples.push(osc_sample * env_level * amplitude);
            }

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
