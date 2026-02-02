//! Real-time audio processing command.

use crate::effects::{create_effect_with_params, parse_chain};
use clap::Args;
use sonido_config::{get_factory_preset, find_preset as config_find_preset, Preset};
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

    /// Preset name or path (supports factory presets, user presets, and file paths)
    #[arg(short, long)]
    preset: Option<String>,

    /// Effect parameters (e.g., "drive=15")
    #[arg(long, value_parser = parse_key_val, number_of_values = 1)]
    param: Vec<(String, String)>,

    /// Input device (index, exact name, or partial name)
    #[arg(short, long, alias = "input-device")]
    input: Option<String>,

    /// Output device (index, exact name, or partial name)
    #[arg(short, long, alias = "output-device")]
    output: Option<String>,

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

    if let Some(preset_name) = &args.preset {
        // Load preset by name or path using sonido-config
        let preset = load_preset(preset_name)?;

        println!("Loading preset: {}", preset.name);
        for effect_cfg in &preset.effects {
            if effect_cfg.bypassed {
                continue; // Skip bypassed effects
            }
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

    // Resolve device names from index or partial match
    let (default_input, default_output) = default_device()?;

    let (input_name, resolved_input) = match &args.input {
        Some(spec) => {
            // Try to find the device to get its full name for display
            match sonido_io::find_device_fuzzy(spec, true)
                .or_else(|_| {
                    spec.parse::<usize>()
                        .ok()
                        .and_then(|idx| sonido_io::find_device_by_index(idx, true).ok())
                        .ok_or_else(|| anyhow::anyhow!("device not found"))
                })
            {
                Ok(device) => (device.name.clone(), Some(spec.clone())),
                Err(_) => (spec.clone(), Some(spec.clone())), // Pass through, let stream handle errors
            }
        }
        None => (
            default_input
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "none".to_string()),
            None,
        ),
    };

    let (output_name, resolved_output) = match &args.output {
        Some(spec) => {
            match sonido_io::find_device_fuzzy(spec, false)
                .or_else(|_| {
                    spec.parse::<usize>()
                        .ok()
                        .and_then(|idx| sonido_io::find_device_by_index(idx, false).ok())
                        .ok_or_else(|| anyhow::anyhow!("device not found"))
                })
            {
                Ok(device) => (device.name.clone(), Some(spec.clone())),
                Err(_) => (spec.clone(), Some(spec.clone())),
            }
        }
        None => (
            default_output
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "none".to_string()),
            None,
        ),
    };

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
        input_device: resolved_input.or_else(|| args.input.clone()),
        output_device: resolved_output.or_else(|| args.output.clone()),
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

/// Load a preset by name or path.
///
/// Searches in this order:
/// 1. Factory presets (by name)
/// 2. User presets (by name)
/// 3. System presets (by name)
/// 4. File path (if it's a path to a .toml file)
fn load_preset(name: &str) -> anyhow::Result<Preset> {
    // Try factory preset first
    if let Some(preset) = get_factory_preset(name) {
        return Ok(preset);
    }

    // Try to find in user/system directories
    if let Some(path) = config_find_preset(name) {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    // Try as a direct file path
    let path = PathBuf::from(name);
    if path.exists() {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    anyhow::bail!(
        "Preset '{}' not found. Use 'sonido presets list' to see available presets.",
        name
    )
}
