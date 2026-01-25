//! Sonido Core - DSP primitives for audio effects
//!
//! This crate provides the foundational building blocks for audio DSP:
//!
//! - [`Effect`] trait for all audio effects
//! - [`SmoothedParam`] and [`LinearSmoothedParam`] for zipper-free parameter changes
//! - [`Biquad`] second-order IIR filter
//! - [`StateVariableFilter`] multi-output filter
//! - [`Lfo`] low-frequency oscillator
//! - [`InterpolatedDelay`] and [`FixedDelayLine`] for delay-based effects
//! - [`EnvelopeFollower`] for dynamics processing
//! - [`Oversampled`] wrapper for anti-aliased nonlinear processing
//!
//! ## no_std Support
//!
//! This crate is `no_std` compatible. Use `default-features = false` in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! sonido-core = { version = "0.1", default-features = false }
//! ```
//!
//! ## Example
//!
//! ```rust,ignore
//! use sonido_core::{Effect, EffectExt};
//!
//! // Chain effects together with static dispatch
//! let mut chain = distortion.chain(chorus).chain(delay);
//!
//! // Or use dynamic dispatch for runtime flexibility
//! let effects: Vec<Box<dyn Effect>> = vec![
//!     Box::new(distortion),
//!     Box::new(chorus),
//! ];
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod effect;
pub mod param;
pub mod math;
pub mod biquad;
pub mod svf;
pub mod lfo;
pub mod delay;
pub mod envelope;
pub mod oversample;

// Re-export main types at crate root
pub use effect::{Effect, EffectExt, Chain};
pub use param::{SmoothedParam, LinearSmoothedParam};
pub use math::{db_to_linear, linear_to_db, fast_tanh, soft_clip, hard_clip, foldback, asymmetric_clip};
pub use biquad::{Biquad, lowpass_coefficients, highpass_coefficients, bandpass_coefficients, notch_coefficients};
pub use svf::{StateVariableFilter, SvfOutput};
pub use lfo::{Lfo, LfoWaveform};
pub use delay::{InterpolatedDelay, FixedDelayLine, Interpolation};
pub use envelope::EnvelopeFollower;
pub use oversample::{Oversampled, MAX_OVERSAMPLE_FACTOR};
