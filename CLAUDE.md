# Sonido DSP Framework

Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.

## Crates

| Crate | Purpose | no_std |
|-------|---------|--------|
| sonido-core | Effect trait, ParameterInfo, SmoothedParam, delays, filters, LFOs | Yes |
| sonido-effects | Distortion, Compressor, Chorus, Delay, Reverb, etc. (all implement ParameterInfo) | Yes |
| sonido-registry | Effect factory and discovery by name/category | Yes |
| sonido-platform | Hardware abstraction: PlatformController trait, ControlMapper, ControlId | Yes |
| sonido-analysis | FFT, spectral analysis, transfer functions | No |
| sonido-io | WAV I/O, real-time audio streaming (cpal), stereo support | No |
| sonido-cli | Command-line processor and analyzer | No |
| sonido-gui | egui-based real-time effects GUI | No |

## Effect Trait

Stereo-first design with backwards compatibility:

```rust
pub trait Effect {
    // Primary stereo processing (implement for true stereo effects)
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32);

    // Mono processing (implement for mono effects, or use default)
    fn process(&mut self, input: f32) -> f32;

    // Block processing
    fn process_block(&mut self, input: &[f32], output: &mut [f32]);
    fn process_block_stereo(&mut self, left_in: &[f32], right_in: &[f32],
                            left_out: &mut [f32], right_out: &mut [f32]);

    // Metadata
    fn is_true_stereo(&self) -> bool;  // true for decorrelated L/R processing
    fn set_sample_rate(&mut self, sample_rate: f32);
    fn reset(&mut self);
    fn latency_samples(&self) -> usize;
}
```

**True stereo effects** (decorrelated L/R): Reverb, Chorus, Delay (ping-pong), Phaser, Flanger
**Dual-mono effects** (independent L/R): Distortion, Compressor, Filter, Gate, Tremolo, etc.

## Key Patterns

**SmoothedParam** - Use `advance()` per sample:
```rust
let mut gain = SmoothedParam::with_config(1.0, 48000.0, 10.0);
gain.set_target(0.5);
for sample in buffer { *sample *= gain.advance(); }
```

**Effect chaining**: `.chain()` for static, `Vec<Box<dyn Effect>>` for dynamic.

**ParameterInfo** - All effects implement this for runtime introspection:
```rust
pub trait ParameterInfo {
    fn param_count(&self) -> usize;
    fn param_info(&self, index: usize) -> Option<ParamDescriptor>;
    fn get_param(&self, index: usize) -> f32;
    fn set_param(&mut self, index: usize, value: f32);
}
```

**Effect Registry** - Create effects by name:
```rust
let registry = EffectRegistry::new();
let mut effect = registry.create("distortion", 48000.0).unwrap();
```

## Commands

```bash
cargo test                          # All tests
cargo test --no-default-features    # no_std check
cargo bench                         # Benchmarks
cargo run -p sonido-gui             # Launch GUI
cargo run -p sonido-cli -- --help   # CLI help
```

## Key Files

| Component | Location |
|-----------|----------|
| Effect trait | crates/sonido-core/src/effect.rs |
| ParameterInfo trait | crates/sonido-core/src/param_info.rs |
| SmoothedParam | crates/sonido-core/src/param.rs |
| Effect Registry | crates/sonido-registry/src/lib.rs |
| Reverb | crates/sonido-effects/src/reverb.rs |
| CombFilter/AllpassFilter | crates/sonido-core/src/comb.rs, allpass.rs |
| GUI app | crates/sonido-gui/src/app.rs |
| CLI commands | crates/sonido-cli/src/main.rs |
| CLI presets | crates/sonido-cli/src/preset.rs |

## Conventions

- SmoothedParam: 5-10ms default smoothing, call `advance()` per sample
- libm for no_std math, std::f32 with std feature
- Tests: `#[cfg(test)] mod tests` in each module
- Benchmarks: block sizes 64/128/256/512/1024
- All public items documented

## Hardware Context

See `docs/HARDWARE.md` for embedded target details (Daisy Seed, Hothouse DIY pedal platform).

## audioDNA RE Priority

1. Modulation (Ventura, Modela)
2. Delay (Obscura)
3. Filter/synth (Dirty Robot)
4. Reverb (Polara, Supernatural)
