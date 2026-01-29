//! Audio I/O layer for the Sonido DSP framework.
//!
//! This crate provides:
//! - WAV file reading and writing via [`wav`]
//! - Real-time audio streaming via [`stream`]
//! - Effect chain processing via [`engine`]

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
