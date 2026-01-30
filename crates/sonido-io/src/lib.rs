//! Audio I/O layer for the Sonido DSP framework.
//!
//! This crate provides:
//!
//! - **WAV file I/O**: [`read_wav`] and [`write_wav`] for loading/saving audio files
//! - **Real-time streaming**: [`AudioStream`] for live audio input/output
//! - **Effect processing**: [`ProcessingEngine`] for applying effect chains to audio
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use sonido_io::{read_wav, write_wav, ProcessingEngine};
//! use sonido_effects::Reverb;
//!
//! // Load audio file
//! let (samples, spec) = read_wav("input.wav")?;
//!
//! // Process with effects
//! let mut engine = ProcessingEngine::new(spec.sample_rate as f32);
//! engine.add_effect(Box::new(Reverb::new(spec.sample_rate as f32)));
//! let processed = engine.process_buffer(&samples);
//!
//! // Save result
//! write_wav("output.wav", &processed, spec)?;
//! ```

mod wav;
mod stream;
mod engine;

pub use wav::{read_wav, write_wav, WavSpec};
pub use stream::{AudioStream, AudioDevice, StreamConfig, list_devices, default_device};
pub use engine::ProcessingEngine;

/// Error types for audio I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("WAV file error: {0}")]
    Wav(#[from] hound::Error),

    #[error("Audio stream error: {0}")]
    Stream(String),

    #[error("No audio device available")]
    NoDevice,

    #[error("Unsupported sample format: {0}")]
    UnsupportedFormat(String),

    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
