# Sonido DSP Framework

## Project Vision
Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.
Ultimate goal: Help DigiTech revamp their legacy audioDNA pedals (Polara, Obscura, Ventura, etc.)

## Architecture
- sonido-core: no_std primitives (Effect trait, params, delays, filters, LFOs)
- sonido-effects: Effect implementations (distortion, compressor, chorus, delay, tape effects)
- sonido-analysis: Spectral tools for reverse engineering (std only, uses rustfft)

## Core Trait
All effects implement Effect (object-safe, f32):
- process(input: f32) -> f32
- process_block(input: &[f32], output: &mut [f32])
- set_sample_rate(sample_rate: f32)
- reset()
- latency_samples() -> usize

## Conventions
- SmoothedParam for all parameters (5-10ms default smoothing)
- libm for no_std math, std::f32 with std feature
- Tests: #[cfg(test)] mod tests in each module
- Benchmarks: Criterion, test block sizes 64/128/256/512/1024
- All public items documented
- Effect chaining via .chain() (static) or Vec<Box<dyn Effect>> (dynamic)

## Commands
- cargo test                          # All tests
- cargo test --no-default-features    # no_std check
- cargo bench                         # Criterion benchmarks
- cargo build --release               # Release build

## Key Source Locations
- Effect trait: crates/sonido-core/src/effect.rs
- Parameter smoothing: crates/sonido-core/src/param.rs
- Oversampling: crates/sonido-core/src/oversample.rs
- MultiVibrato (original): crates/sonido-effects/src/multi_vibrato.rs

## audioDNA RE Priority
1. Modulation effects (Ventura, Modela) - simplest
2. Delay effects (Obscura)
3. Filter/synth (Dirty Robot)
4. Reverb (Polara, Supernatural) - most complex
