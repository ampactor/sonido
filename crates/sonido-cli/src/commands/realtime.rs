//! Real-time audio processing command.

use crate::effects::{create_effect_with_params, parse_chain};
use crate::preset::Preset;
use clap::Args;
use sonido_io::{default_device, AudioStream, ProcessingEngine, StreamConfig};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

#[derive(Args)]
pub struct RealtimeArgs {
    /// Single effect to apply
    #[arg(short, long)]
    effect: Option<String>,

    /// Effect chain specification
    #[arg(short, long)]
    chain: Option<String>,

    /// Preset file (TOML)
    #[arg(short, long)]
    preset: Option<PathBuf>,

    /// Effect parameters (e.g., "drive=15")
    #[arg(long, value_parser = parse_key_val, number_of_values = 1)]
    param: Vec<(String, String)>,

    /// Input device name
    #[arg(long)]
    input_device: Option<String>,

    /// Output device name
    #[arg(long)]
    output_device: Option<String>,

    /// Sample rate
    #[arg(long, default_value = "48000")]
    sample_rate: u32,

    /// Buffer size
    #[arg(long, default_value = "256")]
    buffer_size: u32,

    /// Force mono processing (ignore stereo input/output)
    #[arg(long)]
    mono: bool,
}

fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid parameter format: '{}' (expected key=value)",
            s
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

pub fn run(args: RealtimeArgs) -> anyhow::Result<()> {
    let sample_rate = args.sample_rate as f32;

    // Build effect chain
    let mut engine = ProcessingEngine::new(sample_rate);

    if let Some(preset_path) = &args.preset {
        // Load preset from file
        let preset_content = std::fs::read_to_string(preset_path)?;
        let preset: Preset = toml::from_str(&preset_content)?;

        println!("Loading preset: {}", preset.name);
        for effect_cfg in preset.effects {
            let effect = create_effect_with_params(
                &effect_cfg.effect_type,
                sample_rate,
                &effect_cfg.params,
            )?;
            engine.add_effect(effect);
        }
    } else if let Some(chain_spec) = &args.chain {
        let effects = parse_chain(chain_spec, sample_rate)?;
        for effect in effects {
            engine.add_effect(effect);
        }
    } else if let Some(effect_name) = &args.effect {
        let params: HashMap<String, String> = args.param.into_iter().collect();
        let effect = create_effect_with_params(effect_name, sample_rate, &params)?;
        engine.add_effect(effect);
    } else {
        anyhow::bail!("No effect specified. Use --effect, --chain, or --preset");
    }

    if engine.is_empty() {
        anyhow::bail!("No effects to process");
    }

    // Show device info
    let (default_input, default_output) = default_device()?;
    let input_name = args
        .input_device
        .as_ref()
        .or(default_input.as_ref().map(|d| &d.name))
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    let output_name = args
        .output_device
        .as_ref()
        .or(default_output.as_ref().map(|d| &d.name))
        .cloned()
        .unwrap_or_else(|| "none".to_string());

    let mode = if args.mono { "mono" } else { "stereo" };
    println!("Real-time {} processing with {} effect(s)", mode, engine.len());
    println!("  Input:  {}", input_name);
    println!("  Output: {}", output_name);
    println!("  Sample rate: {} Hz", args.sample_rate);
    println!("  Buffer size: {} samples", args.buffer_size);
    println!("\nPress Ctrl+C to stop...\n");

    // Set up Ctrl+C handler
    let running = Arc::new(AtomicBool::new(true));
    let r = Arc::clone(&running);
    ctrlc::set_handler(move || {
        println!("\nStopping...");
        r.store(false, Ordering::SeqCst);
    })?;

    // Create audio stream
    let config = StreamConfig {
        sample_rate: args.sample_rate,
        buffer_size: args.buffer_size,
        input_device: args.input_device,
        output_device: args.output_device,
    };

    let mut stream = AudioStream::new(config)?;

    // Run the audio stream on the main thread
    // Use stereo or mono processing based on flag
    if args.mono {
        stream.run(move |input, output| {
            engine.process_block(input, output);
        })?;
    } else {
        stream.run_stereo(move |left_in, right_in, left_out, right_out| {
            engine.process_block_stereo(left_in, right_in, left_out, right_out);
        })?;
    }

    println!("Done!");
    Ok(())
}
