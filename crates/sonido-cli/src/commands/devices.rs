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
    List,

    /// Show default device information
    Info,
}

pub fn run(args: DevicesArgs) -> anyhow::Result<()> {
    match args.command.unwrap_or(DevicesCommand::List) {
        DevicesCommand::List => {
            let devices = list_devices()?;

            if devices.is_empty() {
                println!("No audio devices found.");
                return Ok(());
            }

            println!("Available Audio Devices");
            println!("=======================\n");

            let inputs: Vec<_> = devices.iter().filter(|d| d.is_input).collect();
            let outputs: Vec<_> = devices.iter().filter(|d| d.is_output).collect();

            if !inputs.is_empty() {
                println!("Input Devices:");
                for device in &inputs {
                    let also_output = if device.is_output { " (also output)" } else { "" };
                    println!(
                        "  - {} ({} Hz){}",
                        device.name, device.default_sample_rate, also_output
                    );
                }
                println!();
            }

            if !outputs.is_empty() {
                println!("Output Devices:");
                for device in &outputs {
                    if device.is_input {
                        continue; // Already shown above
                    }
                    println!("  - {} ({} Hz)", device.name, device.default_sample_rate);
                }
                println!();
            }

            println!("Total: {} input(s), {} output(s)", inputs.len(), outputs.len());
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
