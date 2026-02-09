//! Effect listing and information command.

#![allow(clippy::print_literal)] // Table headers use literal strings intentionally

use crate::effects::available_effects;
use clap::Args;

#[derive(Args)]
pub struct EffectsArgs {
    /// Show details for a specific effect
    #[arg(value_name = "EFFECT")]
    effect: Option<String>,

    /// Show example commands
    #[arg(long)]
    examples: bool,
}

pub fn run(args: EffectsArgs) -> anyhow::Result<()> {
    let effects = available_effects();

    if let Some(effect_name) = &args.effect {
        // Show details for specific effect
        let effect = effects
            .iter()
            .find(|e| e.name.eq_ignore_ascii_case(effect_name))
            .ok_or_else(|| anyhow::anyhow!("Unknown effect: {}", effect_name))?;

        println!("{}", effect.name);
        println!("{}", "=".repeat(effect.name.len()));
        println!();
        println!("{}", effect.description);
        println!();

        println!("Parameters:");
        println!();
        println!(
            "  {:12}  {:40}  {:12}  {}",
            "Name", "Description", "Default", "Range"
        );
        println!(
            "  {:12}  {:40}  {:12}  {}",
            "----", "-----------", "-------", "-----"
        );

        for param in effect.parameters {
            println!(
                "  {:12}  {:40}  {:12}  {}",
                param.name, param.description, param.default, param.range
            );
        }

        println!();
        println!("Example usage:");
        println!();

        // Generate example command
        let params: Vec<String> = effect
            .parameters
            .iter()
            .take(2)
            .map(|p| format!("{}={}", p.name, p.default))
            .collect();

        if params.is_empty() {
            println!(
                "  sonido process input.wav output.wav --effect {}",
                effect.name
            );
            println!("  sonido realtime --effect {}", effect.name);
        } else {
            println!(
                "  sonido process input.wav output.wav --effect {} --param {}",
                effect.name,
                params.join(" --param ")
            );
            println!(
                "  sonido realtime --effect {} --param {}",
                effect.name,
                params.join(" --param ")
            );
            println!();
            println!("  # Or using chain syntax:");
            println!(
                "  sonido process input.wav output.wav --chain \"{}:{}\"",
                effect.name,
                params.join(",")
            );
        }
    } else {
        // List all effects
        println!("Available Effects");
        println!("=================");
        println!();

        for effect in &effects {
            println!("  {:15} - {}", effect.name, effect.description);
        }

        println!();
        println!("Use 'sonido effects <name>' for detailed parameter info.");

        if args.examples {
            println!();
            println!("Example Commands");
            println!("----------------");
            println!();
            println!("  # Process a file with a single effect");
            println!("  sonido process input.wav output.wav --effect distortion --param drive=15");
            println!();
            println!("  # Process with an effect chain");
            println!(
                "  sonido process input.wav output.wav --chain \"preamp:gain=6|distortion:drive=12|delay:time=300\""
            );
            println!();
            println!("  # Real-time processing");
            println!("  sonido realtime --effect chorus --param rate=1.5 --param depth=0.5");
            println!();
            println!("  # Generate test signals");
            println!("  sonido generate sweep test_sweep.wav --start 20 --end 20000");
            println!("  sonido generate tone a440.wav --freq 440");
            println!();
            println!("  # Analyze audio");
            println!("  sonido analyze spectrum input.wav --peaks 10");
            println!("  sonido analyze transfer dry.wav wet.wav");
            println!();
            println!("  # A/B comparison for reverse engineering");
            println!("  sonido compare hardware_recording.wav software_output.wav --detailed");
        } else {
            println!("Use 'sonido effects --examples' for example commands.");
        }
    }

    Ok(())
}
