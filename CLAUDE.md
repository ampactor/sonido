# Sonido DSP Framework

Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.

## Crates

| Crate | Purpose | no_std |
|-------|---------|--------|
| sonido-core | Effect trait, ParameterInfo, SmoothedParam, delays, filters, LFOs, tempo, modulation | Yes |
| sonido-effects | Distortion, Compressor, Chorus, Delay, Reverb, etc. (all implement ParameterInfo) | Yes |
| sonido-synth | Synthesis engine: oscillators (PolyBLEP), ADSR envelopes, voices, modulation matrix | Yes |
| sonido-registry | Effect factory and discovery by name/category | Yes |
| sonido-config | CLI-first configuration and preset management | No |
| sonido-platform | Hardware abstraction: PlatformController trait, ControlMapper, ControlId | Yes |
| sonido-analysis | FFT, spectral analysis, transfer functions, CFC/PAC analysis | No |
| sonido-io | WAV I/O, real-time audio streaming (cpal), stereo support | No |
| sonido-cli | Command-line processor and analyzer | No |
| sonido-gui-core | Shared GUI widgets, theme, and ParamBridge trait for standalone + plugin UIs | No |
| sonido-gui | egui-based real-time effects GUI | No |

## Documentation Rules (Mandatory)

Documentation must stay synchronized with code. Every code change that modifies behavior,
API surface, or DSP algorithms must include corresponding documentation updates in the same
commit. These rules are non-negotiable.

### Doc-to-Code Mapping Table

When you modify a source file, you **must** update every documentation target listed in the same row.

| Source File(s) | Documentation Target(s) | What to Update |
|---|---|---|
| `crates/sonido-core/src/effect.rs` | This file (Effect Trait section), `docs/ARCHITECTURE.md`, `docs/DESIGN_DECISIONS.md` ADR-001 | Trait signature, method docs, stereo/mono classification |
| `crates/sonido-core/src/param.rs` | This file (Key Patterns: SmoothedParam), `docs/DSP_FUNDAMENTALS.md` (Parameter Smoothing) | Smoothing config, advance() usage, default timing |
| `crates/sonido-core/src/param_info.rs` | This file (Key Patterns: ParameterInfo) | Trait methods, ParamDescriptor fields |
| `crates/sonido-core/src/modulation.rs` | This file (Key Patterns: ModulationSource), `docs/DSP_FUNDAMENTALS.md` (Modulation Effects) | Trait interface, bipolar/unipolar ranges |
| `crates/sonido-core/src/tempo.rs` | This file (Key Patterns: TempoManager), `docs/DSP_FUNDAMENTALS.md` (Tempo Sync) | NoteDivision variants, BPM conversion formulas |
| `crates/sonido-core/src/biquad.rs` | `docs/DSP_FUNDAMENTALS.md` (Biquad), `docs/DESIGN_DECISIONS.md` ADR-007 | Filter types, coefficient formulas, Direct Form choice |
| `crates/sonido-core/src/svf.rs` | `docs/DSP_FUNDAMENTALS.md` (SVF), `docs/DESIGN_DECISIONS.md` ADR-008 | SVF topology, modulation stability notes |
| `crates/sonido-core/src/comb.rs`, `allpass.rs` | `docs/DSP_FUNDAMENTALS.md` (Reverb: Freeverb), `docs/EFFECTS_REFERENCE.md` (Reverb) | Delay lengths, feedback structure |
| `crates/sonido-core/src/delay.rs` | `docs/DSP_FUNDAMENTALS.md` (Delay Lines), `docs/DESIGN_DECISIONS.md` ADR-013 | Interpolation methods, buffer sizing |
| `crates/sonido-core/src/oversample.rs` | `docs/DSP_FUNDAMENTALS.md` (Oversampling), `docs/DESIGN_DECISIONS.md` ADR-003 | Const generic design, filter coefficients |
| `crates/sonido-core/src/lfo.rs` | `docs/DSP_FUNDAMENTALS.md` (Modulation Effects) | Waveform shapes, phase accumulator |
| `crates/sonido-effects/src/reverb.rs` | `docs/EFFECTS_REFERENCE.md` (Reverb), `docs/DSP_FUNDAMENTALS.md` (Freeverb), `docs/DESIGN_DECISIONS.md` ADR-009 | Comb tunings, stereo spread, parameters |
| `crates/sonido-effects/src/distortion.rs` | `docs/EFFECTS_REFERENCE.md` (Distortion), `docs/DSP_FUNDAMENTALS.md` (Waveshaping) | Clipping modes, waveshaper transfer functions |
| `crates/sonido-effects/src/compressor.rs` | `docs/EFFECTS_REFERENCE.md` (Compressor), `docs/DSP_FUNDAMENTALS.md` (Dynamics) | Attack/release, knee, ratio, makeup gain |
| `crates/sonido-effects/src/chorus.rs` | `docs/EFFECTS_REFERENCE.md` (Chorus), `docs/DSP_FUNDAMENTALS.md` (Modulation Effects) | Delay modulation, voice count, stereo spread |
| `crates/sonido-effects/src/delay.rs` | `docs/EFFECTS_REFERENCE.md` (Delay) | Feedback, ping-pong, tempo sync |
| `crates/sonido-effects/src/phaser.rs` | `docs/EFFECTS_REFERENCE.md` (Phaser) | Allpass stage count, sweep range |
| `crates/sonido-effects/src/flanger.rs` | `docs/EFFECTS_REFERENCE.md` (Flanger) | Feedback polarity, delay range |
| `crates/sonido-effects/src/*.rs` (any new effect) | `docs/EFFECTS_REFERENCE.md`, `README.md` (features + count), this file (Key Files) | Full effect entry with parameters, DSP theory, example |
| `crates/sonido-effects/src/*.rs`, `crates/sonido-core/src/gain.rs` | `docs/DSP_QUALITY_STANDARD.md` | Quality rules, compliance table, measurement protocol |
| `crates/sonido-synth/src/oscillator.rs` | `docs/SYNTHESIS.md`, `docs/DSP_FUNDAMENTALS.md` (PolyBLEP), `docs/DESIGN_DECISIONS.md` ADR-014 | Waveforms, poly_blep correction, anti-aliasing |
| `crates/sonido-synth/src/envelope.rs` | `docs/SYNTHESIS.md` | ADSR stages, curve shapes, gate behavior |
| `crates/sonido-synth/src/voice.rs` | `docs/SYNTHESIS.md` | Voice allocation, stealing policy |
| `crates/sonido-synth/src/mod_matrix.rs` | `docs/SYNTHESIS.md` | Routing, source/destination IDs, depth scaling |
| `crates/sonido-registry/src/lib.rs` | This file (Key Patterns: Effect Registry), `docs/DESIGN_DECISIONS.md` ADR-011 | Registration API, create-by-name pattern |
| `crates/sonido-analysis/src/cfc.rs` | `docs/BIOSIGNAL_ANALYSIS.md`, `docs/CFC_ANALYSIS.md` | PAC algorithm, comodulogram, surrogate stats |
| `crates/sonido-analysis/src/filterbank.rs` | `docs/BIOSIGNAL_ANALYSIS.md` | EEG band definitions, filter bank design |
| `crates/sonido-analysis/src/hilbert.rs` | `docs/BIOSIGNAL_ANALYSIS.md` | Analytic signal, instantaneous phase/amplitude |
| `crates/sonido-platform/src/*.rs` | `docs/HARDWARE.md`, `docs/DESIGN_DECISIONS.md` ADR-012 | PlatformController trait, ControlId namespaces |
| `crates/sonido-cli/src/commands/*.rs` | `docs/CLI_GUIDE.md` | Command syntax, flags, examples |
| `crates/sonido-gui/src/app.rs` | `docs/GUI.md` | GUI features, layout, controls |
| `crates/sonido-config/src/*.rs` | `docs/GETTING_STARTED.md` (presets section) | Preset format, config paths, validation |
| Any new crate | This file (Crates table + Key Files table), `docs/ARCHITECTURE.md` (diagram) | Crate purpose, no_std status, dependency position |

### Inline Documentation Rules

These rules apply to all Rust source files. Do not merge code that violates them.

1. **Every public item gets `///` rustdoc.** Structs, enums, traits, functions, methods, constants, and type aliases all require doc comments. No exceptions.

2. **DSP functions must document the math.** For any function that implements a DSP algorithm (filter coefficient calculation, waveshaping, envelope curves, etc.), the doc comment must explain:
   - What the algorithm does in signal processing terms
   - The mathematical formula or transfer function (if applicable)
   - The reference source (paper, textbook, cookbook)
   - Parameter ranges and units (Hz, dB, ms, normalized 0-1, etc.)

3. **Module-level `//!` docs are required.** Every `.rs` file must begin with a `//!` comment block that states the module's purpose and key concepts. For DSP modules, include a brief theory section.

4. **Parameter setters must document ranges.**
   ```rust
   /// Sets the filter cutoff frequency.
   ///
   /// Range: 20.0 to 20000.0 Hz. Values are clamped to this range.
   /// The cutoff frequency determines the -3dB point of the filter response.
   pub fn set_cutoff(&mut self, freq_hz: f32) { /* ... */ }
   ```

5. **Struct docs must list all parameters with defaults.**
   ```rust
   /// Chorus effect with configurable voice count and modulation depth.
   ///
   /// ## Parameters
   /// - `rate`: LFO rate in Hz (0.1 to 10.0, default 1.0)
   /// - `depth`: Modulation depth in ms (0.0 to 10.0, default 3.0)
   /// - `mix`: Wet/dry ratio (0.0 to 1.0, default 0.5)
   /// - `voices`: Number of chorus voices (1 to 4, default 2)
   pub struct Chorus { /* ... */ }
   ```

### DSP Theory Documentation Requirements

When adding a new DSP algorithm or significantly modifying an existing one:

1. **Update `docs/DSP_FUNDAMENTALS.md`** with a theory section that explains:
   - The signal processing concept (what problem it solves)
   - The mathematical basis (transfer functions, difference equations, etc.)
   - Why this implementation was chosen over alternatives
   - Known limitations or trade-offs
   - File path references to the implementation

2. **Update `docs/DESIGN_DECISIONS.md`** with a new ADR if the choice has architectural implications (e.g., choosing SVF over biquad for modulated filters, choosing PolyBLEP over MinBLEP for anti-aliasing).

3. **Cite references.** Every algorithm must trace back to a published source:
   - Robert Bristow-Johnson, "Audio EQ Cookbook" (biquad filters)
   - Valimaki/Smith, "Principles of Digital Signal Processing" (delay-based effects)
   - Jezar's Freeverb (reverb topology)
   - Välimäki et al., "Antialiasing Oscillators" (PolyBLEP)
   - Zolzer, "DAFX" (general effects)

### Documentation Verification Checklist

Run this checklist before every commit that touches code:

- [ ] `cargo doc --no-deps --all-features` produces no warnings
- [ ] `cargo test --doc` passes (all rustdoc examples compile and run)
- [ ] Every new public item has a `///` doc comment
- [ ] Every new `.rs` file has a `//!` module doc comment
- [ ] DSP functions document the algorithm and cite the reference
- [ ] Parameter setters document the valid range and units
- [ ] The Key Files table below is up to date
- [ ] `docs/EFFECTS_REFERENCE.md` is updated if any effect changed
- [ ] `docs/DSP_FUNDAMENTALS.md` is updated if any DSP algorithm changed
- [ ] `docs/DESIGN_DECISIONS.md` has an ADR for any new architectural choice
- [ ] `README.md` reflects any user-facing changes
- [ ] `docs/CHANGELOG.md` has an entry for the change
- [ ] No stale references to renamed or removed items in any `.md` file

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

**SmoothedParam** - Use `advance()` per sample. Preset constructors: `fast` (5ms), `standard` (10ms), `slow` (20ms), `interpolated` (50ms):
```rust
let mut gain = SmoothedParam::standard(1.0, 48000.0);  // 10ms smoothing
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

**Effect Registry** - Create effects by name. Returns `Box<dyn EffectWithParams + Send>` (combines `Effect` + `ParameterInfo`):
```rust
let registry = EffectRegistry::new();
let mut effect = registry.create("distortion", 48000.0).unwrap();
effect.process(0.5);                          // Effect trait
effect.effect_set_param(0, 20.0);             // EffectWithParams trait
let idx = registry.param_index_by_name("distortion", "drive"); // lookup by name
```

**ModulationSource** - Unified interface for LFOs, envelopes, followers:
```rust
use sonido_core::{Lfo, ModulationSource};
let mut lfo = Lfo::new(48000.0, 2.0);
let value = lfo.mod_advance();  // -1.0 to 1.0 for bipolar sources
let uni = lfo.mod_advance_unipolar();  // 0.0 to 1.0
```

**TempoManager** - Tempo-synced timing for delays and LFOs:
```rust
use sonido_core::{TempoManager, NoteDivision};
let tempo = TempoManager::new(48000.0, 120.0);
let delay_ms = tempo.division_to_ms(NoteDivision::DottedEighth);  // 375ms at 120 BPM
let lfo_hz = tempo.division_to_hz(NoteDivision::Quarter);  // 2 Hz at 120 BPM
```

**Oversampling** - Wrap any nonlinear effect for anti-aliasing:
```rust
use sonido_core::{Effect, Oversampled};
use sonido_effects::Distortion;

// Create a distortion effect
let dist = Distortion::new(48000.0);

// Wrap it with 4x oversampling (inner effect runs at 192kHz)
let mut oversampled = Oversampled::<4, _>::new(dist, 48000.0);

// Process audio - harmonics above Nyquist are filtered out
let output = oversampled.process(0.5);

// Access inner effect to change parameters
oversampled.inner_mut().set_drive_db(20.0);
```

Supported factors: 2 (good balance), 4 (recommended for distortion), 8 (high quality).

## Adding a New Effect

1. **Create file** `crates/sonido-effects/src/my_effect.rs` — implement `Effect`, `ParameterInfo`, and `Default`
2. **Add module + re-export** in `crates/sonido-effects/src/lib.rs`:
   ```rust
   pub mod my_effect;
   pub use my_effect::MyEffect;
   ```
3. **Register in registry** — add an entry in `register_builtin_effects()` in `crates/sonido-registry/src/lib.rs`:
   ```rust
   self.register(
       EffectDescriptor {
           id: "my_effect",
           name: "My Effect",
           description: "...",
           category: EffectCategory::Modulation,
           param_count: 3,
       },
       |sr| Box::new(MyEffect::new(sr)),
   );
   ```
   Update the import at the top of the file and adjust test assertions for `registry.len()` and category counts.
4. **Add to CLI effect list** in `crates/sonido-cli/src/commands/effects.rs` if needed
5. **Add regression test + golden file** — run `REGENERATE_GOLDEN=1 cargo test --test regression -p sonido-effects`
6. **Update docs** — `docs/EFFECTS_REFERENCE.md`, `README.md` effect count, and this file's Key Files table

## Adding a New CLI Command

1. **Create command file** `crates/sonido-cli/src/commands/my_command.rs` with:
   - An args struct deriving `clap::Args` (e.g., `pub struct MyCommandArgs { ... }`)
   - A `pub fn run(args: MyCommandArgs) -> anyhow::Result<()>` function
2. **Register module** in `crates/sonido-cli/src/commands/mod.rs`:
   ```rust
   pub mod my_command;
   ```
3. **Add variant** to the `Commands` enum in `crates/sonido-cli/src/main.rs`:
   ```rust
   /// Description of my command
   MyCommand(commands::my_command::MyCommandArgs),
   ```
4. **Add match arm** in `main()`:
   ```rust
   Commands::MyCommand(args) => commands::my_command::run(args),
   ```
5. **Update docs** — `docs/CLI_GUIDE.md` with syntax, flags, and examples

## Commands

```bash
cargo test                          # All tests
cargo test --no-default-features    # no_std check
cargo bench                         # Benchmarks
cargo run -p sonido-gui             # Launch GUI
cargo run -p sonido-cli -- --help   # CLI help
make dev-install                    # Symlink debug build to ~/.local/bin
```

## Key Files

| Component | Location |
|-----------|----------|
| Effect trait | crates/sonido-core/src/effect.rs |
| ParameterInfo trait | crates/sonido-core/src/param_info.rs |
| SmoothedParam | crates/sonido-core/src/param.rs |
| Gain staging helpers | crates/sonido-core/src/gain.rs |
| OnePole filter | crates/sonido-core/src/one_pole.rs |
| Math (mix, dB, waveshape) | crates/sonido-core/src/math.rs |
| ModulationSource trait | crates/sonido-core/src/modulation.rs |
| TempoManager/NoteDivision | crates/sonido-core/src/tempo.rs |
| Effect Registry | crates/sonido-registry/src/lib.rs |
| Reverb | crates/sonido-effects/src/reverb.rs |
| CombFilter/AllpassFilter | crates/sonido-core/src/comb.rs, allpass.rs |
| DcBlocker | crates/sonido-core/src/dc_blocker.rs |
| flush_denormal | crates/sonido-core/src/math.rs |
| Oscillator (PolyBLEP) | crates/sonido-synth/src/oscillator.rs |
| ADSR Envelope | crates/sonido-synth/src/envelope.rs |
| Voice/VoiceManager | crates/sonido-synth/src/voice.rs |
| ModulationMatrix | crates/sonido-synth/src/mod_matrix.rs |
| Preset/EffectConfig | crates/sonido-config/src/lib.rs |
| PAC/Comodulogram | crates/sonido-analysis/src/cfc.rs |
| FilterBank | crates/sonido-analysis/src/filterbank.rs |
| HilbertTransform | crates/sonido-analysis/src/hilbert.rs |
| ParamBridge trait | crates/sonido-gui-core/src/param_bridge.rs |
| GUI widgets (knob, meter, toggle) | crates/sonido-gui-core/src/widgets/ |
| GUI theme | crates/sonido-gui-core/src/theme.rs |
| ChainManager | crates/sonido-gui/src/chain_manager.rs |
| GUI app | crates/sonido-gui/src/app.rs |
| CLI commands | crates/sonido-cli/src/main.rs |
| CLI analyze commands | crates/sonido-cli/src/commands/analyze.rs |
| DSP Theory Reference | docs/DSP_FUNDAMENTALS.md |
| Architecture Decisions | docs/DESIGN_DECISIONS.md |
| DSP Quality Standard | docs/DSP_QUALITY_STANDARD.md |
| Daisy Seed Integration | docs/DAISY_SEED.md |

## Conventions

- SmoothedParam: use preset constructors (`fast`/`standard`/`slow`/`interpolated`), call `advance()` per sample
- ParamDescriptor: use factories (`::mix()`, `::depth()`, `::feedback()`) for common params
- Output level: all effects expose `output` as last ParameterInfo index, use `gain::output_level_param()`
- Dry/wet mix: use `wet_dry_mix()` / `wet_dry_mix_stereo()` from math.rs
- libm for no_std math, std::f32 with std feature
- Tests: `#[cfg(test)] mod tests` in each module
- Benchmarks: block sizes 64/128/256/512/1024
- All public items documented

## Common Pitfalls

1. **no_std math**: Use `libm::sinf()` / `libm::floorf()`, never `f32::sin()` / `f32::floor()` in no_std crates (sonido-core, effects, synth, registry, platform). The `rem_euclid_f32()` helper in oscillator.rs exists because `f32::rem_euclid()` requires std.

2. **SmoothedParam must advance()**: Call `advance()` once per sample in your process loop. Forgetting this means parameters never actually smooth — they jump instantly.

3. **Effect::reset() must clear ALL state**: Delay buffers, filter history, LFO phase, envelope state, smoothed params. Missing any causes bleed between notes/presets.

4. **is_true_stereo() classification**: Return `true` only if L/R outputs are decorrelated (different delay times, different LFO phases, etc.). Dual-mono effects return `false`.

5. **process() / process_stereo() mutual recursion**: The Effect trait has default impls that call each other. You MUST override at least one to avoid infinite recursion.

6. **Golden file regeneration**: After intentional DSP changes, run `REGENERATE_GOLDEN=1 cargo test --test regression -p sonido-effects` to update baselines. Verify the new output sounds correct before committing.

7. **ParameterInfo indices are stable**: Once an effect is published, parameter indices are part of the public API. Add new params at the end, never reorder.

## Testing

**Golden file regression tests**: `crates/sonido-effects/tests/regression.rs` compares effect output against reference WAV files in `tests/golden/`. Three metrics must pass:
- MSE < 1e-6 (sample-level accuracy)
- SNR > 60 dB (signal quality)
- Spectral correlation > 0.9999 (frequency content preserved)

Regenerate after intentional changes: `REGENERATE_GOLDEN=1 cargo test --test regression -p sonido-effects`

**no_std verification**: `cargo test --no-default-features -p sonido-core` (and effects, synth, registry, platform)

## Hardware Context

See `docs/HARDWARE.md` for embedded target details (Daisy Seed, Hothouse DIY pedal platform).

## audioDNA Reference Implementations

Sonido's algorithms are informed by analysis of commercial DSP products (clean-room, no proprietary code).
See the [audioDNA section in README.md](README.md#audiodna-reverse-engineering-reference-implementations) for the full mapping table.

Priority order: Modulation > Delay > Filter/Synth > Reverb.
