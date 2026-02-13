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

pub mod allpass;
pub mod biquad;
pub mod comb;
pub mod dc_blocker;
pub mod delay;
pub mod effect;
pub mod envelope;
pub mod fast_math;
pub mod gain;
pub mod lfo;
pub mod math;
pub mod modulation;
pub mod one_pole;
pub mod oversample;
pub mod param;
pub mod param_info;
pub mod svf;
pub mod tempo;

// Re-export main types at crate root
pub use allpass::AllpassFilter;
pub use biquad::{
    Biquad, bandpass_coefficients, highpass_coefficients, lowpass_coefficients, notch_coefficients,
    peaking_eq_coefficients,
};
pub use comb::CombFilter;
pub use dc_blocker::DcBlocker;
pub use delay::{FixedDelayLine, InterpolatedDelay, Interpolation};
pub use effect::{Chain, Effect, EffectExt};
pub use envelope::EnvelopeFollower;
pub use fast_math::{
    fast_db_to_linear, fast_exp2, fast_linear_to_db, fast_log2, fast_sin_turns, fast_tan,
};
pub use lfo::{Lfo, LfoWaveform};
pub use math::{
    asymmetric_clip, db_to_linear, fast_tanh, flush_denormal, foldback, hard_clip, linear_to_db,
    mono_sum, soft_clip, wet_dry_mix, wet_dry_mix_stereo,
};
pub use modulation::{ModulationAmount, ModulationSource};
pub use one_pole::OnePole;
pub use oversample::{MAX_OVERSAMPLE_FACTOR, Oversampled};
pub use param::{LinearSmoothedParam, SmoothedParam};
pub use param_info::{ParamDescriptor, ParamUnit, ParameterInfo};
pub use svf::{StateVariableFilter, SvfOutput};
pub use tempo::{NoteDivision, TempoManager, TransportState};
