//! Sonido Effects - Audio effect implementations
//!
//! This crate provides production-ready audio effects built on sonido-core:
//!
//! - [`Distortion`] - Waveshaping distortion with multiple algorithms
//! - [`Compressor`] - Dynamics compressor with soft knee
//! - [`Chorus`] - Classic dual-voice chorus
//! - [`Delay`] - Tape-style feedback delay
//! - [`LowPassFilter`] - Biquad-based lowpass filter
//! - [`MultiVibrato`] - 10-unit tape wow/flutter simulation
//! - [`TapeSaturation`] - Tape warmth and HF rolloff
//! - [`CleanPreamp`] - High-headroom preamp stage
//!
//! ## Example
//!
//! ```rust,ignore
//! use sonido_core::{Effect, EffectExt};
//! use sonido_effects::{Distortion, Chorus, Delay};
//!
//! let mut dist = Distortion::new(48000.0);
//! dist.set_drive_db(20.0);
//!
//! let chorus = Chorus::new(48000.0);
//! let delay = Delay::new(48000.0);
//!
//! // Chain effects together
//! let mut chain = dist.chain(chorus).chain(delay);
//! let output = chain.process(input);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod distortion;
pub mod compressor;
pub mod chorus;
pub mod delay;
pub mod filter;
pub mod multi_vibrato;
pub mod tape_saturation;
pub mod preamp;

// Re-export main types at crate root
pub use distortion::{Distortion, WaveShape};
pub use compressor::Compressor;
pub use chorus::Chorus;
pub use delay::Delay;
pub use filter::LowPassFilter;
pub use multi_vibrato::MultiVibrato;
pub use tape_saturation::TapeSaturation;
pub use preamp::CleanPreamp;
