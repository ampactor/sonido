# Design Decisions

Architecture Decision Records (ADRs) for the Sonido DSP framework. Each record captures the context, options considered, and rationale behind a significant design choice.

## Table of Contents

- [ADR-001: Stereo-First Effect Trait](#adr-001-stereo-first-effect-trait)
- [ADR-002: `no_std` as First-Class Constraint](#adr-002-no_std-as-first-class-constraint)
- [ADR-003: Const Generic Oversampling](#adr-003-const-generic-oversampling)
- [ADR-004: Index-Based Parameter System](#adr-004-index-based-parameter-system)
- [ADR-005: Static vs. Dynamic Effect Chaining](#adr-005-static-vs-dynamic-effect-chaining)
- [ADR-006: SmoothedParam Smoothing Strategy](#adr-006-smoothedparam-smoothing-strategy)
- [ADR-007: Direct Form I for Biquad Filters](#adr-007-direct-form-i-for-biquad-filters)
- [ADR-008: SVF Alongside Biquad](#adr-008-svf-alongside-biquad)
- [ADR-009: Separate Reverb Tanks for Stereo](#adr-009-separate-reverb-tanks-for-stereo)
- [ADR-010: f32 Throughout](#adr-010-f32-throughout)
- [ADR-011: Effect Registry Pattern](#adr-011-effect-registry-pattern)
- [ADR-012: Platform Abstraction with ControlId Namespaces](#adr-012-platform-abstraction-with-controlid-namespaces)
- [ADR-013: Two Delay Line Implementations](#adr-013-two-delay-line-implementations)
- [ADR-014: PolyBLEP over BLIT/MinBLEP](#adr-014-polyblep-over-blitminblep)
- [ADR-015: ModulationSource Trait Unification](#adr-015-modulationsource-trait-unification)
- [ADR-016: Denormal Protection via flush_denormal()](#adr-016-denormal-protection-via-flush_denormal)

---

## ADR-001: Stereo-First Effect Trait

**Status:** Accepted
**Source:** `crates/sonido-core/src/effect.rs`

### Context

Audio effects libraries typically start with a mono `process(f32) -> f32` interface and add stereo support later. This creates friction for effects that require cross-channel interaction (reverb, ping-pong delay, stereo widener) because the mono interface cannot express them.

### Decision

Make `process_stereo(f32, f32) -> (f32, f32)` the primary processing method. Mono `process()` remains for backwards compatibility and simple effects. Default implementations bridge between them:

- Mono effects implement `process()`. The default `process_stereo()` calls `process()` independently for each channel.
- True stereo effects implement `process_stereo()`. The default `process()` derives mono by sending the same signal to both channels and returning the left output.

An `is_true_stereo()` metadata method lets hosts distinguish the two cases.

### Rationale

- **No API break**: Existing mono effects continue to work without modification
- **Correct stereo by default**: Dual-mono processing (independent L/R) is the correct default for effects without cross-channel interaction
- **Explicit cross-channel**: True stereo effects declare themselves, enabling host optimizations (e.g., skip stereo processing when the source is mono)
- **Block processing**: Both `process_block` and `process_block_stereo` have default implementations, reducing boilerplate while allowing override for SIMD optimization

### Alternatives Considered

- **Separate mono/stereo traits**: Would fragment the API and make `dyn Effect` chaining impossible for mixed chains
- **Channel count as runtime parameter**: More flexible but adds branching to the hot path
- **Always stereo (no mono path)**: Would add overhead for purely mono effects and make the API less ergonomic for simple use cases

---

## ADR-002: `no_std` as First-Class Constraint

**Status:** Accepted
**Source:** All `no_std`-compatible crates

### Context

Sonido targets both desktop and embedded platforms (Electrosmith Daisy Seed, STM32H750). Embedded targets often lack a standard library, heap allocator, or floating-point unit.

### Decision

Core crates (`sonido-core`, `sonido-effects`, `sonido-synth`, `sonido-registry`, `sonido-platform`) are `no_std` by default with an optional `std` feature. This means:

- All math uses `libm` functions (`sinf`, `cosf`, `tanf`, etc.) instead of `f32` methods
- `alloc` is used where heap allocation is needed (delay lines), kept behind feature gates where possible
- `FixedDelayLine<const N: usize>` provides a fully stack-allocated alternative
- No `println!`, `format!`, `std::time`, or file I/O in core crates

### Rationale

- **Compile-time enforcement**: `#![cfg_attr(not(feature = "std"), no_std)]` at the crate level means any accidental `std` dependency is a compile error
- **Embedded-first, not embedded-afterthought**: Adding `no_std` support retroactively is painful; designing for it from the start is cleaner
- **Minimal cost on desktop**: Using `libm` instead of `std::f32` has negligible performance impact (the compiler often inlines to the same hardware instructions)

### Tradeoffs

- `libm` math functions may have slightly different rounding behavior compared to hardware FPU implementations, though this is inaudible for audio
- Some patterns require `alloc` (e.g., `Vec<f32>` for variable-length delay lines), which means truly allocation-free code must use `FixedDelayLine<N>`

---

## ADR-003: Const Generic Oversampling

**Status:** Accepted
**Source:** `crates/sonido-core/src/oversample.rs`

### Context

Oversampling is needed to prevent aliasing from nonlinear processing (distortion, waveshaping). The oversampling factor (2x, 4x, 8x) determines the quality/CPU tradeoff.

### Decision

Use Rust const generics to make the oversampling factor a compile-time parameter:

```rust
pub struct Oversampled<const FACTOR: usize, E: Effect> { ... }
```

### Rationale

- **Zero-cost abstraction**: The compiler can unroll loops and optimize for the specific factor. A `for i in 0..FACTOR` loop where `FACTOR` is known at compile time becomes straight-line code.
- **Type safety**: `Oversampled<4, Distortion>` is a distinct type from `Oversampled<2, Distortion>`, preventing accidental factor changes
- **Static coefficient selection**: The FIR filter coefficients are selected at compile time via a match on `FACTOR`, eliminating runtime branching in the hot path
- **Composable**: The `Oversampled` wrapper implements `Effect`, so it can be chained, wrapped in `Box<dyn Effect>`, or used anywhere an effect is expected

### Tradeoffs

- The oversampling factor cannot be changed at runtime without creating a new instance
- Three sets of FIR coefficients are compiled into the binary even if only one factor is used (though dead code elimination may remove unused ones)

---

## ADR-004: Index-Based Parameter System

**Status:** Accepted
**Source:** `crates/sonido-core/src/param_info.rs`

### Context

Effects need a way to expose their parameters for GUIs, MIDI mapping, presets, and automation. The system must be `no_std` compatible and work with `dyn Effect`.

### Decision

Use integer-indexed parameter access via the `ParameterInfo` trait:

```rust
pub trait ParameterInfo {
    fn param_count(&self) -> usize;
    fn param_info(&self, index: usize) -> Option<ParamDescriptor>;
    fn get_param(&self, index: usize) -> f32;
    fn set_param(&mut self, index: usize, value: f32);
}
```

Each parameter is described by a `ParamDescriptor` with name, unit, range, default, and step size.

### Rationale

- **`no_std` compatible**: No `String`, `HashMap`, or dynamic dispatch needed. Parameter names are `&'static str`.
- **Object-safe**: Works with `dyn ParameterInfo` for runtime-polymorphic effects
- **Consistent**: Every effect uses the same interface, enabling generic GUI generation and preset serialization
- **Efficient**: Index-based access compiles to a simple match statement

### Alternatives Considered

- **String-keyed HashMap**: More ergonomic but requires `std`, heap allocation, and string hashing in the audio thread
- **Macro-generated parameter structs**: Type-safe but breaks object safety and makes dynamic effect chains impossible
- **Enum-based parameter IDs**: Type-safe per effect but incompatible across different effect types

---

## ADR-005: Static vs. Dynamic Effect Chaining

**Status:** Accepted
**Source:** `crates/sonido-core/src/effect.rs` (Chain type)

### Context

Users need to compose effects into signal chains. The two fundamental approaches are:

1. **Static (compile-time)**: The chain structure is known at compile time
2. **Dynamic (runtime)**: Effects can be added/removed/reordered at runtime

### Decision

Provide both:

- **Static**: `EffectExt::chain()` produces `Chain<A, B>` with zero overhead
- **Dynamic**: `Vec<Box<dyn Effect>>` for runtime-configurable chains

### Rationale for Static Chaining

The `Chain<A, B>` type nests statically:

```rust
let chain = dist.chain(chorus).chain(delay);
// Type: Chain<Chain<Distortion, Chorus>, Delay>
```

The compiler inlines `process()` calls across the entire chain, eliminating virtual dispatch and enabling cross-effect optimizations. For a fixed effect chain (common in embedded and plugin contexts), this achieves the same performance as hand-written sequential processing.

The `Chain` type correctly propagates:
- **Latency**: Sum of both effects' latencies
- **Stereo**: True stereo if either effect is true stereo
- **Sample rate**: Set on both effects
- **Reset**: Reset both effects

### When to Use Dynamic

Dynamic chains (`Vec<Box<dyn Effect>>`) are appropriate for:
- GUIs where users can reorder effects
- Configurations loaded at runtime
- Effect counts that vary

The virtual dispatch overhead (~2-3 ns per effect per sample) is negligible for most applications.

---

## ADR-006: SmoothedParam Smoothing Strategy

**Status:** Accepted
**Source:** `crates/sonido-core/src/param.rs`

### Context

Audio parameters need smooth transitions to avoid zipper noise. The choice of smoothing algorithm affects both the sound quality and the computational cost.

### Decision

Provide two smoothing strategies:

1. **`SmoothedParam`**: Exponential smoothing (one-pole lowpass)
2. **`LinearSmoothedParam`**: Linear ramp with exact endpoint

### Rationale

**Exponential** is the default because:
- One multiply-add per sample (minimal CPU)
- Natural-sounding transitions (like physical systems with inertia)
- No state beyond current value and coefficient
- Settles asymptotically without overshoot

**Linear** is provided for:
- Crossfades where equal-power behavior matters
- Situations requiring a predictable, exact transition time
- The final value is reached exactly (no asymptotic tail)

The coefficient formula for exponential smoothing (`param.rs:146-151`):

```rust
self.coeff = 1.0 - expf(-1.0 / samples);
```

This is derived from the one-pole lowpass time constant: after `smoothing_time_ms`, the parameter reaches 63.2% of the way to the target (one time constant). After 5x the smoothing time, it is within 0.7% (inaudible for most parameters).

---

## ADR-007: Direct Form I for Biquad Filters

**Status:** Accepted
**Source:** `crates/sonido-core/src/biquad.rs`

### Context

Biquad filters can be implemented in several topologies: Direct Form I, Direct Form II, Transposed Direct Form II, and others. The choice affects numerical accuracy, state storage, and modulation behavior.

### Decision

Use Direct Form I with 4 state variables (`x[n-1]`, `x[n-2]`, `y[n-1]`, `y[n-2]`).

### Rationale

- **Numerical stability**: Direct Form I separates input and output delay lines, reducing the dynamic range of intermediate values. With `f32`, this matters: Direct Form II can exhibit limit-cycle oscillations (small DC offsets that never decay) when filter coefficients produce poles near the unit circle (high Q, low frequency).
- **Modulation tolerance**: Changing biquad coefficients while the filter is active is technically incorrect for all direct forms, but Direct Form I handles it more gracefully because the delay line values remain valid across coefficient changes. Direct Form II stores internal state that depends on the current coefficients, making coefficient changes produce larger transients.
- **Simplicity**: The implementation is 5 multiply-adds and 4 state updates, easy to verify and audit.

### Tradeoffs

- **Memory**: 4 state variables instead of 2 (Direct Form II). This is 8 extra bytes per filter, negligible even with many parallel filters.
- **For clean modulation**: The SVF topology (ADR-008) is preferred over biquad when filter parameters change at audio rate.

---

## ADR-008: SVF Alongside Biquad

**Status:** Accepted
**Source:** `crates/sonido-core/src/svf.rs`

### Context

The biquad is versatile but has limitations for modulated filters. Rapidly changing biquad coefficients can cause instability or audible artifacts.

### Decision

Provide a State Variable Filter as an alternative filter topology, using the TPT (topology-preserving transform) formulation.

### Rationale

The SVF is preferred over the biquad in specific situations:

1. **Audio-rate modulation**: The SVF's state variables represent physical quantities (integrator outputs) that remain meaningful when parameters change. An envelope follower modulating a filter cutoff at audio rate is stable with the SVF but may click with a biquad.

2. **Multi-output**: A single SVF computation yields lowpass, highpass, bandpass, and notch outputs simultaneously (`svf.rs:105-119`). This is useful for crossover networks, multiband effects, or UI visualizations.

3. **Frequency accuracy at high frequencies**: The `tan()` frequency warping in the SVF provides better frequency accuracy near Nyquist compared to the bilinear transform used in RBJ biquad coefficients.

Both topologies are available because each has strengths:

| Feature | Biquad | SVF |
|---------|--------|-----|
| CPU cost | Lower (5 multiply-adds) | Higher (~12 operations) |
| Peaking/shelving EQ | Direct (RBJ cookbook) | Requires additional computation |
| Static coefficients | Preferred (stable, efficient) | Works but unnecessarily expensive |
| Modulated cutoff | Avoid | Preferred |
| Multi-output | No (one output per computation) | Yes (4 outputs per computation) |

---

## ADR-009: Separate Reverb Tanks for Stereo

**Status:** Accepted
**Source:** `crates/sonido-effects/src/reverb.rs`

### Context

The original Freeverb algorithm produces mono output from 8 comb filters and 4 allpass filters. Stereo can be achieved by either processing both channels through the same tank (fake stereo) or using independent tanks with different tunings (true stereo).

### Decision

Use separate left and right filter banks with slightly offset delay times. The right channel uses different comb and allpass tunings:

```rust
const COMB_TUNINGS_44K:   [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const COMB_TUNINGS_44K_R: [usize; 8] = [1139, 1211, 1300, 1379, 1445, 1514, 1580, 1640];
```

The offset is approximately 23 samples (~0.5 ms at 44.1 kHz).

### Rationale

- **Decorrelation**: Offset delay times ensure the left and right reverb tails are uncorrelated, producing a convincing stereo image. Identical tanks would produce a mono reverb panned to center.
- **Width control**: A `stereo_width` parameter blends between the decorrelated channels (full stereo) and their average (mono), using mid-side processing. This gives users continuous control from mono to wide stereo.
- **No cross-feed**: Each channel has its own processing path. The left input excites only the left tank. This is simpler than cross-coupled topologies and avoids feedback loops between channels.

### Tradeoffs

- **Memory**: Two complete sets of comb and allpass filters (2x the state of a mono reverb). At 48 kHz, the total delay memory is approximately 2 x (sum of all delay lengths) x 4 bytes, roughly 200 KB.
- **CPU**: Processing both tanks takes approximately 2x the CPU of a mono reverb. For the 8+4 filter structure, this is still modest.

---

## ADR-010: f32 Throughout

**Status:** Accepted

### Context

Audio processing can use `f32` (32-bit float, ~24 bits mantissa) or `f64` (64-bit float, ~53 bits mantissa). Some DSP libraries use `f64` internally for precision-critical operations like IIR filter state.

### Decision

Use `f32` exclusively for all audio processing, state storage, and parameter values.

### Rationale

- **Sufficient precision**: `f32` provides ~144 dB of dynamic range, exceeding 24-bit audio converters (~144 dB theoretical) and far exceeding the ~120 dB range of human hearing.
- **Cache efficiency**: `f32` uses half the memory bandwidth of `f64`. For delay lines (potentially tens of thousands of samples), this translates directly to fewer cache misses.
- **SIMD alignment**: Four `f32` values fit in a 128-bit SIMD register (SSE/NEON), enabling future vectorization. Only two `f64` values fit, halving throughput.
- **Embedded compatibility**: ARM Cortex-M7 (Daisy Seed) has a single-precision FPU. Double-precision operations would require software emulation, dramatically reducing performance.

### Where f64 Could Help

IIR filter state accumulation at very low frequencies or very high Q can benefit from `f64`. The current mitigation is to use the SVF topology (which has better numerical behavior with `f32`) for filters in those regimes.

---

## ADR-011: Effect Registry Pattern

**Status:** Accepted
**Source:** `crates/sonido-registry/src/lib.rs`

### Context

Applications need to create effects by name at runtime (for presets, configuration files, and dynamic UIs) without hardcoding every effect type.

### Decision

A centralized `EffectRegistry` that maps string names to factory functions, returning `Box<dyn Effect + dyn ParameterInfo>`.

### Rationale

- **Decoupling**: Application code does not need to import every effect type
- **Categorization**: Effects are organized by `EffectCategory` (Dynamics, Distortion, Modulation, etc.) for UI grouping
- **Metadata**: Each registry entry includes name, description, category, and parameter count
- **`no_std` compatible**: The registry uses `alloc` (for `Box` and `Vec`) but not `std`

---

## ADR-012: Platform Abstraction with ControlId Namespaces

**Status:** Accepted
**Source:** `crates/sonido-platform/src/lib.rs`

### Context

Sonido targets multiple platforms: hardware pedals with physical knobs, desktop GUIs, MIDI controllers, and DAW automation. Each platform has its own control mechanism, but the effect code should be platform-agnostic.

### Decision

A 16-bit `ControlId` with namespace prefixes:

| Prefix | Source |
|--------|--------|
| `0x00XX` | Hardware (physical knobs, switches) |
| `0x01XX` | GUI (software widgets) |
| `0x02XX` | MIDI (CC messages) |
| `0x03XX` | Automation (DAW lanes) |

A `ControlMapper` bridges between `ControlId` values and `ParameterInfo` indices, handling denormalization (mapping 0.0-1.0 control values to parameter ranges).

### Rationale

- **Unified mapping**: The same effect code works whether controlled by a physical knob, GUI slider, MIDI CC, or automation lane
- **Priority/conflict resolution**: The namespace makes it clear which control source has authority when multiple sources control the same parameter
- **Compact**: 16-bit IDs fit in embedded memory constraints while supporting up to 256 controls per namespace
- **Extensible**: New namespaces (e.g., OSC, network) can be added without changing existing code

---

## ADR-013: Two Delay Line Implementations

**Status:** Accepted
**Source:** `crates/sonido-core/src/delay.rs`

### Context

Delay lines are used by many effects (chorus, flanger, delay, reverb comb filters). The requirements differ between desktop and embedded targets.

### Decision

Provide two implementations:

1. **`InterpolatedDelay`**: Heap-allocated (`Vec<f32>`), variable-size, linear interpolation. General-purpose, used by most effects.
2. **`FixedDelayLine<const N: usize>`**: Stack-allocated (`[f32; N]`), compile-time fixed size, configurable interpolation (None/Linear/Cubic). For embedded targets or known-size use cases.

### Rationale

- **`InterpolatedDelay`** is appropriate when the delay size depends on sample rate (computed at runtime) or when multiple effects with different delay requirements share code
- **`FixedDelayLine<N>`** eliminates all heap allocation, making it suitable for bare-metal embedded where `alloc` may not be available
- Both share the same circular buffer concept and similar API, reducing cognitive overhead
- **`FixedDelayLine`** offers cubic interpolation, which is valuable for effects like chorus where modulated delay smoothness matters

---

## ADR-014: PolyBLEP over BLIT/MinBLEP

**Status:** Accepted
**Source:** `crates/sonido-synth/src/oscillator.rs`

### Context

Several algorithms exist for band-limited oscillator synthesis:

- **BLIT** (Band-Limited Impulse Train): Generate a band-limited impulse, integrate for saw/triangle
- **MinBLEP** (Minimum-phase Band-Limited Step): Precomputed tables for precise corrections
- **PolyBLEP**: Polynomial approximation of the band-limited step, applied near discontinuities
- **Wavetable**: Precomputed waveforms at multiple frequencies

### Decision

Use PolyBLEP for all non-sinusoidal waveforms.

### Rationale

- **Low CPU cost**: Only a few arithmetic operations per sample, with the correction limited to a small region near each discontinuity. Most samples pass through without any correction.
- **No lookup tables**: Unlike MinBLEP and wavetable, PolyBLEP requires zero precomputed data. This is important for `no_std` targets with limited memory.
- **Good quality**: While not as alias-free as MinBLEP at extreme frequencies, PolyBLEP provides adequate anti-aliasing for the audible frequency range. The remaining aliasing is typically 60-80 dB below the fundamental, inaudible in most musical contexts.
- **Phase modulation compatible**: PolyBLEP works correctly with the phase modulation used for FM synthesis, whereas wavetable approaches require special handling for phase modulation.
- **Simple implementation**: The entire `poly_blep` function is 10 lines of code, easy to understand and audit.

### Tradeoffs

- At very high fundamental frequencies (above ~8 kHz at 48 kHz sample rate), PolyBLEP's aliasing rejection degrades. For these cases, oversampling the oscillator or switching to sine would be cleaner.

---

## ADR-015: ModulationSource Trait Unification

**Status:** Accepted
**Source:** `crates/sonido-core/src/modulation.rs`

### Context

Audio effects are modulated by various time-varying signals: LFOs, envelope followers, ADSR envelopes, sequencers, and external audio. Each produces values in different ranges (bipolar vs. unipolar) with different semantics.

### Decision

Unify all modulation sources under a single trait:

```rust
pub trait ModulationSource {
    fn mod_advance(&mut self) -> f32;
    fn is_bipolar(&self) -> bool;
    fn mod_reset(&mut self);
    fn mod_value(&self) -> f32;
}
```

Default methods provide automatic range conversion (`mod_advance_unipolar`, `mod_advance_bipolar`).

### Rationale

- **Interchangeability**: An effect can accept `&mut dyn ModulationSource` and work with any modulation type without specialization
- **Range safety**: The `is_bipolar()` flag, combined with conversion methods, prevents the common bug of applying a [-1, 1] LFO where a [0, 1] value was expected
- **Composability**: A `ModulationAmount` struct combines source, depth, and inversion into a reusable modulation routing:

```rust
pub struct ModulationAmount {
    pub depth: f32,
    pub inverted: bool,
}
```

This is a building block for the `ModulationMatrix` in the synthesis engine, where arbitrary source-to-destination routings are configured at runtime.

---

## ADR-016: Denormal Protection via flush_denormal()

**Status:** Accepted
**Source:** `crates/sonido-core/src/math.rs`

### Context

IEEE 754 subnormal (denormalized) floating-point numbers cause severe CPU performance degradation -- up to 100x slower processing on x86 and ARM architectures. In audio DSP, subnormals arise naturally when feedback loops (comb filters, allpass chains, IIR filters) decay toward zero. A reverb tail fading to silence can spike CPU usage dramatically if subnormals are not handled.

### Options Considered

1. **FTZ/DAZ CPU flags**: Set the Flush-To-Zero and Denormals-Are-Zero bits in the CPU's floating-point control register (MXCSR on x86, FPSCR on ARM). This globally prevents subnormal generation at the hardware level.

2. **DC offset injection**: Add a tiny constant (~1e-15) to feedback signals to prevent values from reaching zero and entering the subnormal range.

3. **Explicit value-threshold flush**: A function that replaces values below a threshold with zero, applied at strategic points in feedback paths.

### Decision

Use option 3: an explicit `flush_denormal()` function with a threshold of 1e-20.

```rust
#[inline(always)]
pub fn flush_denormal(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}
```

### Rationale

**Portability**: `flush_denormal()` is pure Rust with no platform-specific instructions, no inline assembly, and no CPU flag manipulation. It works identically on x86, ARM, RISC-V, and any other target Sonido supports, including `no_std` embedded systems.

**Precision**: The flush is applied only where needed (feedback paths, IIR state) rather than globally. FTZ/DAZ flags affect all floating-point operations in the thread, which could interfere with other libraries or computation running alongside the audio engine.

**Safety**: FTZ/DAZ flag manipulation requires `unsafe` code (inline assembly or intrinsics) and is thread-global state. Changing it can affect other code in the same thread without their knowledge. The explicit flush has no such side effects.

**DC offset rejection**: Unlike DC injection (option 2), the flush strategy does not add any DC content to the signal. DC injection can accumulate through cascaded effects and reduce headroom. A separate `DcBlocker` filter is provided for cases where DC offset from other sources (asymmetric waveshaping) needs to be removed.

### Tradeoffs

- **Branch per sample**: The `if` condition adds a branch in the feedback path. This is mitigated by `#[inline(always)]` and the fact that the branch is highly predictable (almost always "not flushing" during active signal processing). Modern branch predictors handle this with negligible overhead.
- **Not bit-exact zero**: Values between 0 and 1e-20 are forcibly zeroed, which technically changes the mathematical result. These values are at -400 dBFS, far below any audible or measurable signal level.
- **Manual placement**: Unlike FTZ/DAZ (which is automatic and covers all operations), `flush_denormal()` must be placed explicitly at each point where subnormals could accumulate. Missing a call site leaves that path vulnerable. This is mitigated by denormal stress tests in the test suite.
