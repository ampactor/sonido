//! File-based effect processing command.

use crate::effects::{create_effect_with_params, parse_chain};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use sonido_io::{read_wav, write_wav, ProcessingEngine, WavSpec};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Args)]
pub struct ProcessArgs {
    /// Input WAV file
    #[arg(value_name = "INPUT")]
    input: PathBuf,

    /// Output WAV file
    #[arg(value_name = "OUTPUT")]
    output: PathBuf,

    /// Single effect to apply
    #[arg(short, long)]
    effect: Option<String>,

    /// Effect chain specification (e.g., "preamp:gain=6|distortion:drive=15")
    #[arg(short, long)]
    chain: Option<String>,

    /// Preset file (TOML)
    #[arg(short, long)]
    preset: Option<PathBuf>,

    /// Effect parameters (e.g., "drive=15")
    #[arg(long, value_parser = parse_key_val, number_of_values = 1)]
    param: Vec<(String, String)>,

    /// Processing block size
    #[arg(long, default_value = "512")]
    block_size: usize,

    /// Output bit depth (16, 24, or 32)
    #[arg(long, default_value = "32")]
    bit_depth: u16,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!("Invalid parameter format: '{}' (expected key=value)", s));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub fn run(args: ProcessArgs) -> anyhow::Result<()> {
    // Read input file
    println!("Reading {}...", args.input.display());
    let (samples, spec) = read_wav(&args.input)?;
    let sample_rate = spec.sample_rate as f32;

    println!(
        "  {} samples, {} Hz, {:.2}s",
        samples.len(),
        spec.sample_rate,
        samples.len() as f32 / sample_rate
    );

    // Build effect chain
    let mut engine = ProcessingEngine::new(sample_rate);

    if let Some(preset_path) = &args.preset {
        // Load preset from file
        let preset_content = std::fs::read_to_string(preset_path)?;
        let preset: Preset = toml::from_str(&preset_content)?;

        println!("Loading preset: {}", preset.name);
        for effect_cfg in preset.effects {
            let effect =
                create_effect_with_params(&effect_cfg.effect_type, sample_rate, &effect_cfg.params)?;
            engine.add_effect(effect);
        }
    } else if let Some(chain_spec) = &args.chain {
        // Parse chain specification
        let effects = parse_chain(chain_spec, sample_rate)?;
        for effect in effects {
            engine.add_effect(effect);
        }
    } else if let Some(effect_name) = &args.effect {
        // Single effect with optional parameters
        let params: HashMap<String, String> = args.param.into_iter().collect();
        let effect = create_effect_with_params(effect_name, sample_rate, &params)?;
        engine.add_effect(effect);
    } else {
        anyhow::bail!("No effect specified. Use --effect, --chain, or --preset");
    }

    if engine.is_empty() {
        anyhow::bail!("No effects to process");
    }

    println!("Processing with {} effect(s)...", engine.len());

    // Process with progress bar
    let pb = ProgressBar::new(samples.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
            .unwrap()
            .progress_chars("##-"),
    );

    let mut output = vec![0.0; samples.len()];
    let block_size = args.block_size;

    for (i, (in_chunk, out_chunk)) in samples
        .chunks(block_size)
        .zip(output.chunks_mut(block_size))
        .enumerate()
    {
        let len = in_chunk.len();
        engine.process_block(in_chunk, &mut out_chunk[..len]);
        pb.set_position(((i + 1) * block_size).min(samples.len()) as u64);
    }

    pb.finish_with_message("done");

    // Calculate stats
    let input_rms = rms(&samples);
    let output_rms = rms(&output);
    let input_peak = peak(&samples);
    let output_peak = peak(&output);

    println!("\nStats:");
    println!(
        "  Input:  RMS {:.1} dB, Peak {:.1} dB",
        linear_to_db(input_rms),
        linear_to_db(input_peak)
    );
    println!(
        "  Output: RMS {:.1} dB, Peak {:.1} dB",
        linear_to_db(output_rms),
        linear_to_db(output_peak)
    );

    // Write output file
    let out_spec = WavSpec {
        channels: 1,
        sample_rate: spec.sample_rate,
        bits_per_sample: args.bit_depth,
    };

    println!("\nWriting {}...", args.output.display());
    write_wav(&args.output, &output, out_spec)?;
    println!("Done!");

    Ok(())
}

fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f32 = samples.iter().map(|s| s * s).sum();
    (sum / samples.len() as f32).sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0, f32::max)
}

fn linear_to_db(linear: f32) -> f32 {
    if linear <= 0.0 {
        -120.0
    } else {
        20.0 * linear.log10()
    }
}

/// Preset file format.
#[derive(Debug, serde::Deserialize)]
struct Preset {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default = "default_sample_rate")]
    sample_rate: u32,
    effects: Vec<EffectConfig>,
}

fn default_sample_rate() -> u32 {
    48000
}

#[derive(Debug, serde::Deserialize)]
struct EffectConfig {
    #[serde(rename = "type")]
    effect_type: String,
    #[serde(default)]
    params: HashMap<String, String>,
}
