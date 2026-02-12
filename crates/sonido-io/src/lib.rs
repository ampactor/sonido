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

mod engine;
mod stream;
mod wav;

pub use engine::ProcessingEngine;
pub use stream::{
    AudioDevice, AudioStream, StreamConfig, default_device, find_device_by_index,
    find_device_fuzzy, list_devices,
};
pub use wav::{
    StereoSamples, WavFormat, WavInfo, WavSpec, read_wav, read_wav_info, read_wav_stereo,
    write_wav, write_wav_stereo,
};

/// Error types for audio I/O operations.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// WAV file read/write error.
    #[error("WAV file error: {0}")]
    Wav(#[from] hound::Error),

    /// Audio stream setup or runtime error.
    #[error("Audio stream error: {0}")]
    Stream(String),

    /// No audio device available on the system.
    #[error("No audio device available")]
    NoDevice,

    /// The requested sample format is not supported.
    #[error("Unsupported sample format: {0}")]
    UnsupportedFormat(String),

    /// The requested audio device was not found.
    #[error("Device not found: {0}")]
    DeviceNotFound(String),

    /// Standard I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience result type for audio I/O operations.
pub type Result<T> = std::result::Result<T, Error>;
