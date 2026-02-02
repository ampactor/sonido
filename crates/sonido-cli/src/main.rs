//! Sonido CLI - Command-line interface for the Sonido DSP framework.

mod commands;
mod effects;
mod preset;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "sonido")]
#[command(author, version, about = "Sonido DSP Framework CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process an audio file through effects
    Process(commands::process::ProcessArgs),

    /// Run real-time audio processing
    Realtime(commands::realtime::RealtimeArgs),

    /// Generate test signals
    Generate(commands::generate::GenerateArgs),

    /// Analyze audio files
    Analyze(commands::analyze::AnalyzeArgs),

    /// Compare two audio files (A/B comparison)
    Compare(commands::compare::CompareArgs),

    /// List and manage audio devices
    Devices(commands::devices::DevicesArgs),

    /// List available effects and their parameters
    Effects(commands::effects::EffectsArgs),
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Process(args) => commands::process::run(args),
        Commands::Realtime(args) => commands::realtime::run(args),
        Commands::Generate(args) => commands::generate::run(args),
        Commands::Analyze(args) => commands::analyze::run(args),
        Commands::Compare(args) => commands::compare::run(args),
        Commands::Devices(args) => commands::devices::run(args),
        Commands::Effects(args) => commands::effects::run(args),
    }
}
