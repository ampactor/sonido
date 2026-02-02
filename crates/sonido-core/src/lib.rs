//! Sonido Core - DSP primitives for audio effects
//!
//! This crate provides the foundational building blocks for audio DSP, designed for
//! real-time audio processing with zero allocation in the audio path.
//!
//! # Core Abstractions
//!
//! ## Effect System
//!
//! - [`Effect`] - Object-safe trait for all audio effects
//! - [`EffectExt`] - Extension trait for effect chaining
//! - [`Chain`] - Zero-cost effect chain combinator
//!
//! ## Parameter Smoothing
//!
//! Zipper-free parameter changes for click-free automation:
//!
//! - [`SmoothedParam`] - Exponential smoothing (RC-like response)
//! - [`LinearSmoothedParam`] - Linear ramps (constant rate)
//!
//! ## Filters
//!
//! - [`Biquad`] - Second-order IIR filter with RBJ cookbook coefficients
//! - [`StateVariableFilter`] - Multi-output SVF (lowpass, highpass, bandpass simultaneously)
//! - [`CombFilter`] - Comb filter with damping for reverb algorithms
//! - [`AllpassFilter`] - Schroeder allpass for diffusion
//!
//! ## Delay Lines
//!
//! - [`InterpolatedDelay`] - Variable-length delay with interpolation
//! - [`FixedDelayLine`] - Fixed-length delay (compile-time size)
//!
//! ## Modulation & Dynamics
//!
//! - [`Lfo`] - Low-frequency oscillator (5 waveforms)
//! - [`EnvelopeFollower`] - Amplitude envelope detection
//!
//! ## Utilities
//!
//! - [`Oversampled`] - Generic wrapper for anti-aliased nonlinear processing
//! - Math functions: [`db_to_linear`], [`linear_to_db`], [`fast_tanh`], etc.
//!
//! # no_std Support
//!
//! This crate is `no_std` compatible for embedded audio applications.
//! Disable the default `std` feature in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! sonido-core = { version = "0.1", default-features = false }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use sonido_core::{Effect, EffectExt, SmoothedParam};
//!
//! // Create effects and chain them with zero-cost static dispatch
//! let mut chain = distortion.chain(chorus).chain(delay);
//!
//! // Process audio sample-by-sample
//! for sample in audio_buffer.iter_mut() {
//!     *sample = chain.process(*sample);
//! }
//!
//! // Or process entire blocks for efficiency
//! chain.process_block(&input, &mut output);
//!
//! // For runtime flexibility, use dynamic dispatch
//! let effects: Vec<Box<dyn Effect>> = vec![
//!     Box::new(distortion),
//!     Box::new(chorus),
//! ];
//! ```
//!
//! # Design Principles
//!
//! - **Real-time safe**: No allocations in audio processing paths
//! - **No dependencies on std**: Pure `no_std` with `libm` for math
//! - **Object-safe traits**: Dynamic dispatch when needed
//! - **Zero-cost abstractions**: Static dispatch chains optimize away

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod effect;
pub mod param;
pub mod param_info;
pub mod math;
pub mod biquad;
pub mod svf;
pub mod lfo;
pub mod delay;
pub mod envelope;
pub mod oversample;
pub mod comb;
pub mod allpass;
pub mod tempo;
pub mod modulation;

// Re-export main types at crate root
pub use effect::{Effect, EffectExt, Chain};
pub use param::{SmoothedParam, LinearSmoothedParam};
pub use math::{db_to_linear, linear_to_db, fast_tanh, soft_clip, hard_clip, foldback, asymmetric_clip};
pub use biquad::{Biquad, lowpass_coefficients, highpass_coefficients, bandpass_coefficients, notch_coefficients, peaking_eq_coefficients};
pub use svf::{StateVariableFilter, SvfOutput};
pub use lfo::{Lfo, LfoWaveform};
pub use delay::{InterpolatedDelay, FixedDelayLine, Interpolation};
pub use envelope::EnvelopeFollower;
pub use oversample::{Oversampled, MAX_OVERSAMPLE_FACTOR};
pub use comb::CombFilter;
pub use allpass::AllpassFilter;
pub use param_info::{ParameterInfo, ParamDescriptor, ParamUnit};
pub use tempo::{TempoManager, NoteDivision, TransportState};
pub use modulation::{ModulationSource, ModulationAmount};
