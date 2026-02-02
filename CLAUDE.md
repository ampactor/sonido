# Sonido DSP Framework

Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.

## Crates

| Crate | Purpose | no_std |
|-------|---------|--------|
| sonido-core | Effect trait, ParameterInfo, SmoothedParam, delays, filters, LFOs | Yes |
| sonido-effects | Distortion, Compressor, Chorus, Delay, Reverb, etc. (all implement ParameterInfo) | Yes |
| sonido-registry | Effect factory and discovery by name/category | Yes |
| sonido-analysis | FFT, spectral analysis, transfer functions | No |
| sonido-io | WAV I/O, real-time audio streaming (cpal) | No |
| sonido-cli | Command-line processor and analyzer | No |
| sonido-gui | egui-based real-time effects GUI | No |

## Effect Trait

```rust
pub trait Effect {
    fn process(&mut self, input: f32) -> f32;
    fn process_block(&mut self, input: &[f32], output: &mut [f32]);
    fn set_sample_rate(&mut self, sample_rate: f32);
    fn reset(&mut self);
    fn latency_samples(&self) -> usize;
}
```

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

See `docs/HARDWARE.md` for embedded target details (Daisy Seed, bkshepherd 125B pedal).

## audioDNA RE Priority

1. Modulation (Ventura, Modela)
2. Delay (Obscura)
3. Filter/synth (Dirty Robot)
4. Reverb (Polara, Supernatural)
