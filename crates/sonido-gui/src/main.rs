//! Sonido GUI - Professional DSP effect processor interface.
//!
//! A real-time audio effects application built on the Sonido DSP framework.

use clap::Parser;
use eframe::egui;
use sonido_gui::SonidoApp;

/// Sonido DSP GUI application.
#[derive(Parser, Debug)]
#[command(name = "sonido-gui")]
#[command(about = "Professional DSP effect processor GUI")]
#[command(version)]
struct Args {
    /// Input audio device name (optional, uses default if not specified)
    #[arg(long)]
    input: Option<String>,

    /// Output audio device name (optional, uses default if not specified)
    #[arg(long)]
    output: Option<String>,

    /// Sample rate in Hz (default: 48000)
    #[arg(long, default_value = "48000")]
    sample_rate: u32,

    /// Buffer size in samples (default: 512)
    #[arg(long, default_value = "512")]
    buffer_size: u32,
}

fn main() -> eframe::Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = Args::parse();

    log::info!("Starting Sonido GUI");
    log::info!("Sample rate: {} Hz", args.sample_rate);
    log::info!("Buffer size: {} samples", args.buffer_size);

    if let Some(ref input) = args.input {
        log::info!("Input device: {}", input);
    }
    if let Some(ref output) = args.output {
        log::info!("Output device: {}", output);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Sonido"),
        ..Default::default()
    };

    eframe::run_native(
        "Sonido",
        options,
        Box::new(|cc| Ok(Box::new(SonidoApp::new(cc)))),
    )
}
