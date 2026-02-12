//! Display WAV file metadata.

use clap::Args;
use sonido_io::{WavFormat, read_wav_info};

/// Display WAV file information.
#[derive(Args)]
pub struct InfoArgs {
    /// Path to the WAV file
    pub file: std::path::PathBuf,
}

/// Run the info command.
pub fn run(args: InfoArgs) -> anyhow::Result<()> {
    let info = read_wav_info(&args.file)?;

    let format_str = match info.format {
        WavFormat::Pcm => "PCM",
        WavFormat::IeeeFloat => "IEEE Float",
    };

    println!("File:        {}", args.file.display());
    println!("Format:      {} {}-bit", format_str, info.bits_per_sample);
    println!("Channels:    {}", info.channels);
    println!("Sample Rate: {} Hz", info.sample_rate);
    println!(
        "Duration:    {:.3}s ({} frames)",
        info.duration_secs, info.num_frames
    );

    let file_size = std::fs::metadata(&args.file)?.len();
    println!("File Size:   {}", format_bytes(file_size));

    Ok(())
}

fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}
