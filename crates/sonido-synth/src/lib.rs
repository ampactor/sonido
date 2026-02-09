//! Sonido Synth - Synthesis engine for the sonido DSP framework
//!
//! This crate provides synthesis building blocks including oscillators,
//! envelopes, voice management, and complete synthesizer implementations.
//!
//! # Core Components
//!
//! ## Oscillators
//!
//! Audio-rate oscillators with PolyBLEP anti-aliasing:
//!
//! - [`Oscillator`] - Main audio oscillator with multiple waveforms
//! - [`OscillatorWaveform`] - Waveform types (Sine, Triangle, Saw, Square, Pulse, Noise)
//!
//! ```rust
//! use sonido_synth::{Oscillator, OscillatorWaveform};
//!
//! let mut osc = Oscillator::new(48000.0);
//! osc.set_frequency(440.0);
//! osc.set_waveform(OscillatorWaveform::Saw);
//!
//! let sample = osc.advance();
//! ```
//!
//! ## Envelopes
//!
//! ADSR envelope generators:
//!
//! - [`AdsrEnvelope`] - Attack-Decay-Sustain-Release envelope
//! - [`EnvelopeState`] - Envelope stage tracking
//!
//! ```rust
//! use sonido_synth::{AdsrEnvelope, EnvelopeState};
//!
//! let mut env = AdsrEnvelope::new(48000.0);
//! env.set_attack_ms(10.0);
//! env.set_decay_ms(100.0);
//! env.set_sustain(0.7);
//! env.set_release_ms(200.0);
//!
//! env.gate_on();
//! let level = env.advance();
//! ```
//!
//! ## Voice Management
//!
//! For building polyphonic synthesizers:
//!
//! - [`Voice`] - Single synthesizer voice
//! - [`VoiceManager`] - Polyphonic voice allocation
//! - [`VoiceAllocationMode`] - Voice stealing strategies
//!
//! ## Modulation
//!
//! Flexible modulation routing:
//!
//! - [`ModulationMatrix`] - Route modulation sources to destinations
//! - [`ModSourceId`] / [`ModDestination`] - Source and destination identifiers
//! - [`AudioModSource`] - Use audio input as modulation
//!
//! ## Complete Synthesizers
//!
//! Ready-to-use synthesizer implementations:
//!
//! - [`MonophonicSynth`] - Single-voice synth with glide
//! - [`PolyphonicSynth`] - Multi-voice synth
//!
//! # no_std Support
//!
//! This crate is `no_std` compatible. Disable the default `std` feature:
//!
//! ```toml
//! [dependencies]
//! sonido-synth = { version = "0.1", default-features = false }
//! ```
//!
//! # Example: Simple Polyphonic Synth
//!
//! ```rust
//! use sonido_synth::{PolyphonicSynth, OscillatorWaveform, VoiceAllocationMode};
//!
//! // Create an 8-voice synth
//! let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(48000.0);
//!
//! // Configure sound
//! synth.set_osc1_waveform(OscillatorWaveform::Saw);
//! synth.set_osc2_waveform(OscillatorWaveform::Saw);
//! synth.set_osc2_detune(7.0); // 7 cents detune for thickness
//! synth.set_filter_cutoff(2000.0);
//! synth.set_filter_resonance(2.0);
//! synth.set_amp_attack(10.0);
//! synth.set_amp_release(500.0);
//!
//! // Play a chord
//! synth.note_on(60, 100); // C4
//! synth.note_on(64, 100); // E4
//! synth.note_on(67, 100); // G4
//!
//! // Generate audio
//! let mut buffer = vec![0.0; 1024];
//! for sample in buffer.iter_mut() {
//!     *sample = synth.process();
//! }
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

pub mod audio_mod;
pub mod envelope;
pub mod mod_matrix;
pub mod oscillator;
pub mod synth;
pub mod voice;

// Re-export main types at crate root
pub use audio_mod::{AudioGate, AudioModSource};
pub use envelope::{AdsrEnvelope, EnvelopeState};
pub use mod_matrix::{
    ModDestination, ModSourceId, ModulationMatrix, ModulationRoute, ModulationValues,
};
pub use oscillator::{Oscillator, OscillatorWaveform};
pub use synth::{MonophonicSynth, PolyphonicSynth};
pub use voice::{
    Voice, VoiceAllocationMode, VoiceManager, cents_to_ratio, freq_to_midi, midi_to_freq,
};

// Re-export commonly used types from sonido-core
pub use sonido_core::{Lfo, LfoWaveform, ModulationSource, StateVariableFilter, SvfOutput};
