//! Audio device management command.

use clap::{Args, Subcommand};
use sonido_io::{default_device, list_devices};

#[derive(Args)]
pub struct DevicesArgs {
    #[command(subcommand)]
    command: Option<DevicesCommand>,
}

#[derive(Subcommand)]
enum DevicesCommand {
    /// List all available audio devices
    List {
        /// Include virtual/loopback device information and setup guidance
        #[arg(long)]
        include_virtual: bool,
    },

    /// Show default device information
    Info,
}

pub fn run(args: DevicesArgs) -> anyhow::Result<()> {
    match args.command.unwrap_or(DevicesCommand::List {
        include_virtual: false,
    }) {
        DevicesCommand::List { include_virtual } => {
            let devices = list_devices()?;

            if devices.is_empty() {
                println!("No audio devices found.");
                return Ok(());
            }

            println!("Available Audio Devices");
            println!("=======================\n");

            // Collect input devices with indices
            let inputs: Vec<_> = devices.iter().filter(|d| d.is_input).collect();

            if !inputs.is_empty() {
                println!("Input Devices:");
                for (idx, device) in inputs.iter().enumerate() {
                    let also_output = if device.is_output {
                        " (also output)"
                    } else {
                        ""
                    };
                    println!(
                        "  [{}] {} ({} Hz){}",
                        idx, device.name, device.default_sample_rate, also_output
                    );
                }
                println!();
            }

            // Show all output devices with proper indexing
            let outputs: Vec<_> = devices.iter().filter(|d| d.is_output).collect();
            if !outputs.is_empty() {
                println!("Output Devices:");
                for (idx, device) in outputs.iter().enumerate() {
                    let also_input = if device.is_input { " (also input)" } else { "" };
                    println!(
                        "  [{}] {} ({} Hz){}",
                        idx, device.name, device.default_sample_rate, also_input
                    );
                }
                println!();
            }

            println!(
                "Total: {} input(s), {} output(s)",
                inputs.len(),
                outputs.len()
            );
            println!();
            println!("Tip: Use device index or partial name with --input/--output:");
            println!("  sonido realtime --input 0 --output 0 --effect reverb");
            println!("  sonido realtime --input \"USB\" --output \"USB\" --effect reverb");

            if include_virtual {
                println!();
                print_loopback_guidance(&devices);
            }
        }

        DevicesCommand::Info => {
            let (input, output) = default_device()?;

            println!("Default Audio Devices");
            println!("=====================\n");

            if let Some(device) = input {
                println!("Default Input:");
                println!("  Name: {}", device.name);
                println!("  Sample Rate: {} Hz", device.default_sample_rate);
                println!();
            } else {
                println!("Default Input: None");
                println!();
            }

            if let Some(device) = output {
                println!("Default Output:");
                println!("  Name: {}", device.name);
                println!("  Sample Rate: {} Hz", device.default_sample_rate);
            } else {
                println!("Default Output: None");
            }
        }
    }

    Ok(())
}

fn print_loopback_guidance(devices: &[sonido_io::AudioDevice]) {
    // Check for common loopback device names
    let loopback_keywords = ["loopback", "blackhole", "virtual", "vb-audio", "cable"];

    let virtual_devices: Vec<_> = devices
        .iter()
        .filter(|d| {
            let name_lower = d.name.to_lowercase();
            loopback_keywords.iter().any(|kw| name_lower.contains(kw))
        })
        .collect();

    println!("Virtual/Loopback Devices:");
    println!("-------------------------");

    if virtual_devices.is_empty() {
        println!("  [!] No loopback devices detected");
        println!();
        println!("  To capture system audio, install a virtual audio driver:");
        println!();

        #[cfg(target_os = "windows")]
        {
            println!("  Windows:");
            println!("    - VB-Audio Virtual Cable: https://vb-audio.com/Cable/");
            println!("    - VoiceMeeter: https://vb-audio.com/Voicemeeter/");
        }

        #[cfg(target_os = "macos")]
        {
            println!("  macOS:");
            println!("    - BlackHole: https://existential.audio/blackhole/");
            println!("    - Loopback: https://rogueamoeba.com/loopback/");
        }

        #[cfg(target_os = "linux")]
        {
            println!("  Linux (PulseAudio/PipeWire):");
            println!("    pactl load-module module-loopback");
            println!("    # Or create a null sink:");
            println!("    pactl load-module module-null-sink sink_name=virtual");
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            println!("  Platform-specific guidance not available.");
            println!("  Search for \"virtual audio cable\" for your operating system.");
        }
    } else {
        for device in virtual_devices {
            let kind = if device.is_input && device.is_output {
                "input/output"
            } else if device.is_input {
                "input"
            } else {
                "output"
            };
            println!(
                "  - {} ({}, {} Hz)",
                device.name, kind, device.default_sample_rate
            );
        }
    }
}
