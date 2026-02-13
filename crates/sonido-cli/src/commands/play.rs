//! Audio file playback command with optional effect processing.

use super::common::{load_preset, parse_key_val};
use crate::effects::{create_effect_with_params, parse_chain};
use clap::Args;
use sonido_io::{AudioStream, ProcessingEngine, StreamConfig, read_wav_stereo};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

#[derive(Args)]
pub struct PlayArgs {
    /// WAV file to play
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Single effect to apply during playback
    #[arg(short, long)]
    effect: Option<String>,

    /// Effect chain specification (e.g., "preamp:gain=6|distortion:drive=15")
    #[arg(short, long)]
    chain: Option<String>,

    /// Preset name or path
    #[arg(short, long)]
    preset: Option<String>,

    /// Effect parameters (e.g., "drive=15")
    #[arg(long, value_parser = parse_key_val, number_of_values = 1)]
    param: Vec<(String, String)>,

    /// Output device (index, exact name, or partial name)
    #[arg(short, long)]
    output: Option<String>,

    /// Loop playback
    #[arg(short, long, alias = "repeat")]
    r#loop: bool,

    /// Force mono output
    #[arg(long)]
    mono: bool,
}

pub fn run(args: PlayArgs) -> anyhow::Result<()> {
    // Load file
    println!("Loading {}...", args.file.display());
    let (samples, spec) = read_wav_stereo(&args.file)?;
    let sample_rate = spec.sample_rate as f32;
    let total_frames = samples.len();

    println!(
        "  {} frames, {} Hz, {:.1}s",
        total_frames,
        spec.sample_rate,
        total_frames as f32 / sample_rate
    );

    // Build optional effect chain
    let mut engine = ProcessingEngine::new(sample_rate);
    let has_effects;

    if let Some(preset_name) = &args.preset {
        let preset = load_preset(preset_name)?;
        println!("Loading preset: {}", preset.name);
        for effect_cfg in &preset.effects {
            if effect_cfg.bypassed {
                continue;
            }
            let effect = create_effect_with_params(
                &effect_cfg.effect_type,
                sample_rate,
                &effect_cfg.params,
            )?;
            engine.add_effect(effect);
        }
        has_effects = !engine.is_empty();
    } else if let Some(chain_spec) = &args.chain {
        let effects = parse_chain(chain_spec, sample_rate)?;
        for effect in effects {
            engine.add_effect(effect);
        }
        has_effects = !engine.is_empty();
    } else if let Some(effect_name) = &args.effect {
        let params: HashMap<String, String> = args.param.into_iter().collect();
        let effect = create_effect_with_params(effect_name, sample_rate, &params)?;
        engine.add_effect(effect);
        has_effects = true;
    } else {
        has_effects = false;
    }

    if has_effects {
        println!("Processing through {} effect(s)", engine.len());
    }

    let looping = args.r#loop;

    println!(
        "\nPlaying{}... Press Ctrl+C to stop.\n",
        if looping { " (looping)" } else { "" }
    );

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);
    ctrlc::set_handler(move || {
        println!("\nStopping...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Playback position (in frames)
    let position = Arc::new(AtomicUsize::new(0));

    // Pre-copy sample data for the audio callback
    let left = Arc::new(samples.left);
    let right = Arc::new(samples.right);

    let stream_config = StreamConfig {
        sample_rate: spec.sample_rate,
        buffer_size: 256,
        input_device: None,
        output_device: args.output,
    };

    let mut stream = AudioStream::new(stream_config)?;
    let out_channels = stream.output_channels() as usize;
    // If mono requested, force 1-channel logic; otherwise use device channels
    let channels = if args.mono { 1 } else { out_channels };

    let cb_running = Arc::clone(&running);
    let cb_position = Arc::clone(&position);

    stream.run_output(move |data: &mut [f32]| {
        if !cb_running.load(Ordering::Relaxed) {
            data.fill(0.0);
            return;
        }

        let frames = data.len() / channels;
        let mut pos = cb_position.load(Ordering::Relaxed);

        for i in 0..frames {
            if pos >= total_frames {
                if looping {
                    pos = 0;
                } else {
                    let sample_start = i * channels;
                    data[sample_start..].fill(0.0);
                    cb_running.store(false, Ordering::Relaxed);
                    break;
                }
            }

            let mut l = left[pos];
            let mut r = right[pos];

            if has_effects {
                (l, r) = engine.process_stereo(l, r);
            }

            let idx = i * channels;
            match channels {
                1 => data[idx] = (l + r) * 0.5,
                2 => {
                    data[idx] = l;
                    data[idx + 1] = r;
                }
                _ => {
                    // Multi-channel: L/R in first two, silence the rest
                    data[idx] = l;
                    data[idx + 1] = r;
                    for c in 2..channels {
                        data[idx + c] = 0.0;
                    }
                }
            }

            pos += 1;
        }

        cb_position.store(pos, Ordering::Relaxed);
    })?;

    println!("Done!");
    Ok(())
}
