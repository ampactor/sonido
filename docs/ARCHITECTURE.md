# Architecture

## Overview

Sonido is a production-grade DSP library designed for multi-target deployment:
- **Desktop**: CLI and GUI applications
- **Embedded**: Electrosmith Daisy / Hothouse hardware
- **Plugins**: VST3/AU (future)

The library is built with stereo-first processing and no_std compatibility at its core.

## Crate Diagram

```
┌───────────────────────────────────────────────────────────────────────────┐
│                           Applications                                     │
│  ┌─────────────┐  ┌─────────────┐  ┌───────────┐  ┌──────────────┐        │
│  │ sonido-cli  │  │ sonido-gui  │  │ VST3/AU   │  │sonido-hothouse│        │
│  │  (binary)   │  │  (egui)     │  │ (future)  │  │  (embedded)   │        │
│  └──────┬──────┘  └──────┬──────┘  └─────┬─────┘  └──────┬───────┘        │
└─────────┼────────────────┼───────────────┼───────────────┼────────────────┘
          │                │               │               │
          └────────────────┼───────────────┼───────────────┘
                           │               │
          ┌────────────────┴───────────────┴────────────────┐
          │                                                 │
          ▼                                                 ▼
┌─────────────────────┐                    ┌─────────────────────────────────┐
│   sonido-config     │                    │        sonido-platform          │
│  Presets + Config   │                    │  PlatformController + Mapping   │
│       [std]         │                    │           [no_std]              │
└─────────┬───────────┘                    └───────────────┬─────────────────┘
          │                                                │
          │                ┌───────────────┬───────────────┤
          │                │               │               │
          ▼                ▼               ▼               ▼
┌────────────────┐ ┌─────────────────┐ ┌─────────────┐ ┌─────────────────┐
│  sonido-io     │ │ sonido-analysis │ │  sonido-    │ │  sonido-synth   │
│  (audio I/O)   │ │ (FFT/CFC/PAC)   │ │  registry   │ │  (synthesis)    │
│     [std]      │ │     [std]       │ │  [no_std]   │ │    [no_std]     │
└───────┬────────┘ └────────┬────────┘ └──────┬──────┘ └────────┬────────┘
        │                   │                 │                  │
        └───────────────────┴─────────────────┴──────────────────┘
                                    │
                                    ▼
                           ┌───────────────┐
                           │sonido-effects │
                           │  (15 effects) │
                           │   [no_std]    │
                           └───────┬───────┘
                                   │
                                   ▼
                           ┌───────────────┐
                           │  sonido-core  │
                           │ (primitives)  │
                           │   [no_std]    │
                           └───────────────┘
```

## Crate Responsibilities

### sonido-core

The foundation crate providing DSP primitives. Designed for `no_std` environments.

**Why a separate core crate?** Every DSP system needs the same building blocks: delay lines, filters, LFOs, parameter smoothing. By isolating these into `sonido-core` with `no_std` support, the same primitives compile for desktop (x86_64) and embedded (ARM Cortex-M7) targets without conditional compilation in the effects. This mirrors the layered architecture used in commercial DSP frameworks like JUCE's `dsp` module and Faust's core library, but with Rust's zero-cost abstractions ensuring no runtime overhead from the abstraction boundary.

**Key components:**
- `Effect` trait: Object-safe interface all effects implement
- `SmoothedParam`: Zipper-free parameter changes with exponential/linear smoothing
- `InterpolatedDelay` / `FixedDelayLine`: Delay buffers with fractional interpolation
- `Biquad`: IIR filter building block for EQ, lowpass, highpass, etc.
- `StateVariableFilter`: Multi-output filter (LP, HP, BP simultaneously)
- `CombFilter`: Comb filter with damping for reverb algorithms
- `AllpassFilter`: Schroeder allpass for diffusion
- `Lfo`: Low-frequency oscillator for modulation effects (5 waveforms)
- `EnvelopeFollower`: Amplitude envelope detection for dynamics
- `Oversampled`: Generic 2x/4x/8x oversampling wrapper for anti-aliasing
- `ModulationSource` trait: Unified interface for LFOs, envelopes, followers
- `TempoManager`: Tempo tracking with musical timing utilities
- `NoteDivision`: Musical note divisions (whole, half, quarter, dotted, triplet)

### sonido-effects

Audio effect implementations built on sonido-core. All `no_std` compatible with full stereo support.

**Why separate effects from core?** The `Effect` trait and DSP primitives change rarely; effect implementations change often as new algorithms are added or refined. Separating them means adding a new effect never risks breaking the core infrastructure. It also means `sonido-core` can be used independently for custom DSP work without pulling in all 15 effect implementations.

**15 Effects:**

*True Stereo:*
- `Reverb`: Freeverb-style with decorrelated L/R tanks, stereo width control
- `Chorus`: Dual-voice with L/R panning for stereo spread
- `Delay`: Feedback delay with optional ping-pong stereo mode
- `Phaser`: 4-stage allpass with stereo LFO phase offset
- `Flanger`: Modulated delay with stereo modulation offset

*Dual-Mono:*
- `Distortion`: Waveshaping with soft clip, hard clip, foldback, asymmetric modes
- `Compressor`: Dynamics compressor with soft knee, attack/release, makeup gain
- `Gate`: Noise gate with threshold, attack/release, hold time
- `Wah`: Auto-wah and manual wah with resonant filter
- `ParametricEq`: 3-band parametric EQ with Q control
- `Tremolo`: Amplitude modulation with multiple waveforms
- `TapeSaturation`: J37-style tape warmth with HF rolloff
- `CleanPreamp`: Simple gain stage with input/output control
- `LowPassFilter`: Resonant 2-pole lowpass (SVF-based)
- `MultiVibrato`: 10-unit tape wow/flutter simulation

### sonido-analysis

Spectral analysis tools for reverse engineering hardware and biosignal research. Requires `std` for FFT.

**Why include biosignal analysis in an audio DSP library?** The mathematics of spectral analysis are domain-agnostic -- an FFT doesn't care whether its input is a guitar chord or an EEG trace. Including cross-frequency coupling (CFC/PAC) analysis alongside audio tools means researchers working with biosignals can leverage the same proven filter implementations, windowing functions, and I/O infrastructure. The `sonido-analysis` crate requires `std` because FFT computation uses heap allocation for twiddle factors and scratch buffers, which is acceptable since analysis is never performed in real-time audio callbacks.

**Components:**
- `Fft`: FFT wrapper around rustfft
- `Window`: Window functions (Hamming, Blackman, Hann)
- `TransferFunction`: Measure frequency response between two signals
- `SineSweep`: Generate logarithmic sine sweeps for IR capture

**Cross-Frequency Coupling (CFC):**
- `FilterBank`: Multi-band bandpass filter bank with 4th-order Butterworth filters
- `FrequencyBand`: Frequency band specification with EEG bands (delta, theta, alpha, beta, gamma)
- `HilbertTransform`: FFT-based Hilbert transform for instantaneous phase/amplitude
- `PacAnalyzer`: Phase-Amplitude Coupling analyzer (Mean Vector Length, Kullback-Leibler)
- `PacResult`: PAC analysis results (modulation index, preferred phase, phase histogram)
- `Comodulogram`: Multi-frequency PAC analysis for visualizing coupling patterns

### sonido-io

Audio I/O layer using cpal and hound. Full stereo support.

**Components:**
- `read_wav` / `write_wav`: Mono WAV file I/O
- `read_wav_stereo` / `write_wav_stereo`: Stereo WAV file I/O
- `StereoSamples`: Helper struct for stereo audio with conversions
- `AudioStream`: Real-time audio streaming (mono and stereo)
- `ProcessingEngine`: Block-based effect chain runner with stereo methods

**Stereo I/O:**
```rust
use sonido_io::{read_wav_stereo, write_wav_stereo, StereoSamples};

let (samples, sample_rate) = read_wav_stereo("input.wav")?;
// samples.left, samples.right, samples.to_interleaved(), etc.

write_wav_stereo("output.wav", &processed, sample_rate)?;
```

### sonido-synth

Full synthesis engine for building synthesizers. `no_std` compatible.

**Why a separate synth crate?** Synthesis has different concerns from effects processing. Effects transform existing audio; synthesizers generate it. The voice management, note allocation, and modulation matrix concepts have no analog in effect processing. Keeping synthesis separate also means an embedded effects pedal can depend on `sonido-core` + `sonido-effects` without pulling in oscillator, envelope, and voice code it will never use.

**Oscillators:**
- `Oscillator`: Audio-rate oscillator with PolyBLEP anti-aliasing
- `OscillatorWaveform`: Sine, Triangle, Saw, Square, Pulse, Noise

**Envelopes:**
- `AdsrEnvelope`: Attack-Decay-Sustain-Release envelope generator
- `EnvelopeState`: Envelope stage tracking (Idle, Attack, Decay, Sustain, Release)

**Voice Management:**
- `Voice`: Single synthesizer voice (oscillators + filter + envelopes)
- `VoiceManager`: Polyphonic voice allocation with stealing strategies
- `VoiceAllocationMode`: Oldest, Newest, Quietest, HighestNote, LowestNote

**Modulation:**
- `ModulationMatrix`: Flexible routing of modulation sources to destinations
- `ModulationRoute`: Single modulation routing with depth and curve
- `AudioModSource`: Use audio input as modulation source
- `AudioGate`: Convert audio amplitude to gate signal

**Complete Synths:**
- `MonophonicSynth`: Single-voice synth with portamento/glide
- `PolyphonicSynth<N>`: N-voice polyphonic synth

### sonido-registry

Central registry for discovering and instantiating effects. Provides a unified API for CLI, GUI, and future hardware targets.

**Why a registry pattern?** The CLI, GUI, and potential plugin hosts all need to create effects by name or category. Without a registry, each application would duplicate the mapping from strings to constructors. The registry centralizes this with `EffectDescriptor` metadata, enabling features like "list all modulation effects" or "create effect by name from a preset file" without the application knowing about individual effect types. This is the Factory pattern adapted for DSP -- common in plugin frameworks like VST3 (which uses a similar `IPluginFactory` interface).

**Key components:**
- `EffectRegistry`: Factory for creating effects by name
- `EffectDescriptor`: Metadata (id, name, description, category, param_count)
- `EffectCategory`: Effect categorization (Dynamics, Distortion, Modulation, etc.)
- `EffectWithParams`: Helper trait for accessing ParameterInfo through boxed effects

**Usage:**
```rust
use sonido_registry::EffectRegistry;

let registry = EffectRegistry::new();
let mut effect = registry.create("distortion", 48000.0).unwrap();
```

### sonido-config

CLI-first configuration and preset management. Requires `std`.

**Key components:**
- `Preset`: Effect chain preset with metadata and effect configurations
- `EffectConfig`: Single effect configuration with parameters
- `EffectChain`: Runtime effect chain builder
- `validation`: Effect type and parameter validation
- `paths`: Platform-specific preset directories (user, system)
- `factory_presets`: Built-in presets for common use cases

**Usage:**
```rust
use sonido_config::{Preset, EffectConfig, user_presets_dir};

// Load a preset
let preset = Preset::load("my_preset.toml")?;

// Create programmatically
let preset = Preset {
    name: "My Preset".to_string(),
    description: Some("Custom effect chain".to_string()),
    sample_rate: 48000,
    effects: vec![
        EffectConfig::new("distortion").with_param("drive", "0.6"),
        EffectConfig::new("reverb").with_param("room_size", "0.8"),
    ],
};

preset.save(&user_presets_dir().join("my_preset.toml"))?;
```

### sonido-platform

Hardware abstraction layer for multi-target deployment. Provides `no_std` compatible traits for physical controls and parameter mapping.

**Why abstract hardware controls?** A physical knob on a Hothouse pedal, a GUI slider, a MIDI CC message, and a DAW automation lane all represent the same concept: a continuous control mapped to an effect parameter. The `PlatformController` trait and `ControlMapper` let effect code remain hardware-agnostic. The `ControlId` namespacing (`0x00XX` = hardware, `0x01XX` = GUI, `0x02XX` = MIDI) means the same mapping code handles all input sources without conflicts.

**Key components:**
- `PlatformController`: Trait abstracting hardware I/O (knobs, toggles, footswitches, LEDs)
- `ControlMapper`: Maps normalized control values (0-1) to effect parameters
- `ControlId`: Namespaced control identifiers (hardware, GUI, MIDI, automation)
- `ControlType`: Enumeration of control types (Knob, Toggle3Way, Footswitch, Led, etc.)
- `ControlState`: Control value with change tracking

**Control ID Namespaces:**
- `0x00XX`: Hardware controls (knobs, switches)
- `0x01XX`: GUI controls
- `0x02XX`: MIDI CC
- `0x03XX`: Automation parameters

**Example:**
```rust
use sonido_platform::{ControlMapper, ControlId, ControlType, ParamTarget, ScaleCurve};

let mut mapper: ControlMapper<4> = ControlMapper::new();
mapper.map(
    ControlId::hardware(0),
    ParamTarget::new(0, 0)      // Effect slot 0, param 0
        .with_range(0.0, 1.0)
        .with_curve(ScaleCurve::Logarithmic),
);
```

### sonido-cli

Command-line interface tying everything together.

**Commands:**
- `process`: File-based effect processing
- `realtime`: Live audio processing
- `generate`: Test signal generation
- `analyze`: Spectral analysis (spectrum, transfer, IR, distortion, spectrogram, dynamics)
- `compare`: A/B audio comparison
- `devices`: Audio device management
- `effects`: List available effects
- `presets`: Preset management (list, show, save, delete)
- `tui`: Interactive terminal UI for effect editing

**Analyze subcommands for CFC research:**
- `pac`: Phase-Amplitude Coupling analysis with surrogate testing
- `comodulogram`: Multi-frequency PAC matrix for coupling visualization
- `bandpass`: Extract frequency band with configurable filter order
- `hilbert`: Compute instantaneous phase and amplitude

### sonido-gui

Real-time audio effects processor with professional GUI built on egui.

**Key modules:**
- `app.rs`: Main application state, UI layout, audio thread management
- `audio_bridge.rs`: Lock-free communication between UI and audio thread
- `chain_view.rs`: Drag-and-drop effect chain builder
- `preset_manager.rs`: Preset save/load with categories
- `effects_ui/`: Per-effect parameter panels (knobs, sliders)
- `widgets/`: Custom UI components (Knob, LevelMeter)
- `theme.rs`: Dark theme configuration

**Architecture:**
- UI thread: egui rendering at 60fps
- Audio thread: Real-time processing via cpal
- Communication: `crossbeam-channel` + atomic params for lock-free updates
- Metering: Peak/RMS levels with configurable decay

## Data Flow

### File Processing (Stereo)

```
┌─────────────┐    ┌──────────────────┐    ┌─────────────────────┐    ┌──────────────────┐
│ WAV input   │───▶│ read_wav_stereo  │───▶│ ProcessingEngine    │───▶│ write_wav_stereo │
│ (mono/stereo)│    │  → StereoSamples │    │ process_file_stereo │    └──────────────────┘
└─────────────┘    └──────────────────┘    └─────────────────────┘           │
                                                                              ▼
                                                                        ┌───────────┐
                                                                        │ WAV output│
                                                                        │ (stereo)  │
                                                                        └───────────┘
```

### Real-time Processing (Stereo)

```
┌──────────┐    ┌───────────────┐    ┌────────────────────┐    ┌──────────┐
│ Audio    │───▶│ AudioStream   │───▶│ Effect::           │───▶│ Audio    │
│ input L/R│    │ run_stereo()  │    │ process_stereo()   │    │ output   │
└──────────┘    │ (cpal stereo) │    │ (per-sample)       │    └──────────┘
                └───────────────┘    └────────────────────┘
```

### CLI Stereo Detection

The CLI automatically detects input format:
- Mono input: duplicates to stereo, processes, outputs stereo
- Stereo input: processes stereo, outputs stereo
- Use `--mono` flag to force mono output

### GUI Processing

```
┌─────────────────────────────────────────────────────────────────────┐
│                          UI Thread (egui)                           │
│  ┌──────────┐   ┌──────────────┐   ┌─────────────┐                 │
│  │ Knob/    │──▶│ SharedParams │──▶│ PresetMgr   │                 │
│  │ Controls │   │ (atomics)    │   │ (save/load) │                 │
│  └──────────┘   └──────┬───────┘   └─────────────┘                 │
│                        │                                            │
│  ┌──────────┐          │                                            │
│  │ Meters   │◀─────────┼───────────────────────────────────────┐   │
│  └──────────┘          │                                       │   │
└────────────────────────┼───────────────────────────────────────┼───┘
                         │ atomic reads                          │
                         ▼                                       │
┌────────────────────────────────────────────────────────────────┼───┐
│                       Audio Thread                             │   │
│  ┌──────────┐   ┌────────────────┐   ┌──────────┐              │   │
│  │ cpal     │──▶│ Effect Chain   │──▶│ cpal     │──────────────┘   │
│  │ input    │   │ (process)      │   │ output   │  metering data   │
│  └──────────┘   └────────────────┘   └──────────┘                  │
└────────────────────────────────────────────────────────────────────┘
```

## Effect Trait

All effects implement the `Effect` trait with stereo-first design.

**Why stereo-first?** Most real-world audio is stereo. A mono-first API forces every stereo application to manually duplicate processing or wrap effects in stereo adapters. With stereo-first, the common case (stereo processing) requires no boilerplate, while mono processing derives automatically from the stereo path. The `is_true_stereo()` method lets hosts distinguish effects that have cross-channel interaction (reverb, ping-pong delay) from those that process channels independently (distortion, compressor). This metadata enables optimization: a host can skip the stereo path for dual-mono effects when rendering to mono.

**Why object-safe?** The `Effect` trait avoids associated types, const generics, and non-object-safe methods so that `Box<dyn Effect>` works for dynamic effect chains. This is critical for the GUI chain view (drag-and-drop reordering), preset loading (creating effects by name at runtime), and the CLI (parsing effect names from command-line arguments). Static dispatch via `.chain()` and generics remains available for maximum performance in known configurations.

```rust
pub trait Effect {
    // === STEREO (primary interface) ===

    /// Process a stereo frame. This is the primary method.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32);

    /// Process stereo block (default: calls process_stereo per sample)
    fn process_block_stereo(&mut self, input: &[(f32, f32)], output: &mut [(f32, f32)]);

    // === MONO (for convenience) ===

    /// Process a single mono sample (derives from stereo)
    fn process(&mut self, input: f32) -> f32;

    /// Process a block of mono samples
    fn process_block(&mut self, input: &[f32], output: &mut [f32]);

    // === METADATA ===

    /// True if effect has meaningful stereo processing (not dual-mono)
    fn is_true_stereo(&self) -> bool;

    /// Update sample rate (call when rate changes)
    fn set_sample_rate(&mut self, sample_rate: f32);

    /// Reset internal state (call when starting new audio)
    fn reset(&mut self);

    /// Report latency for delay compensation
    fn latency_samples(&self) -> usize;
}
```

### True Stereo vs Dual-Mono

Effects fall into two categories:

**True Stereo** (`is_true_stereo() -> true`):
- `Reverb`: Decorrelated L/R tanks with stereo width control
- `Chorus`: Voices panned L/R for stereo spread
- `Delay`: Optional ping-pong mode with cross-channel feedback
- `Phaser`: Offset LFO phase between channels
- `Flanger`: Offset modulation between channels

**Dual-Mono** (`is_true_stereo() -> false`):
- `Distortion`, `Compressor`, `Gate`, `Wah`, `ParametricEq`
- `Tremolo`, `TapeSaturation`, `CleanPreamp`, `LowPassFilter`, `MultiVibrato`

Dual-mono effects process each channel independently with the same algorithm.

## ParameterInfo Trait

All effects implement the `ParameterInfo` trait for runtime parameter discovery.

**Why index-based parameters?** Indexed parameters (rather than named string lookups) enable O(1) access in the audio thread and zero-allocation parameter changes. The `ParamDescriptor` provides metadata (name, unit, range, step) for UI generation without embedding strings in the hot path. The `short_name` field (max 8 characters) exists specifically for the Hothouse hardware target, which has a 128x64 OLED display with limited horizontal space. This design is modeled after the VST3 parameter model, where parameters are identified by index for performance and by descriptor for presentation.

```rust
pub trait ParameterInfo {
    /// Returns the number of parameters this effect has.
    fn param_count(&self) -> usize;

    /// Returns descriptor for parameter at index.
    fn param_info(&self, index: usize) -> Option<ParamDescriptor>;

    /// Gets current value of parameter at index.
    fn get_param(&self, index: usize) -> f32;

    /// Sets value of parameter at index.
    fn set_param(&mut self, index: usize, value: f32);
}

pub struct ParamDescriptor {
    pub name: &'static str,       // "Delay Time"
    pub short_name: &'static str, // "Time" (max 8 chars for hardware)
    pub unit: ParamUnit,          // Milliseconds, Decibels, etc.
    pub min: f32,
    pub max: f32,
    pub default: f32,
    pub step: f32,                // Encoder increment
}
```

This enables:
- **Hardware menus**: Enumerate parameters for 128x64 OLED displays
- **Plugin automation**: Expose parameters to DAW hosts
- **Preset systems**: Save/restore parameter state by index
- **Dynamic UIs**: Auto-generate parameter controls

## no_std Compatibility

`sonido-core` and `sonido-effects` support `no_std` for embedded use.

**Why `no_std`?** The target hardware (Electrosmith Daisy Seed running on ARM Cortex-M7) has no operating system, no heap allocator by default, and hard real-time constraints. Audio callbacks must complete within the buffer period (e.g., 64 samples / 48 kHz = 1.33 ms) with zero tolerance for missed deadlines. `no_std` compatibility is enforced by CI (`cargo test --no-default-features`) and ensures that no code path in the audio processing chain can call `malloc`, touch the filesystem, or panic due to OOM.

```toml
[dependencies]
sonido-core = { version = "0.1", default-features = false }
sonido-effects = { version = "0.1", default-features = false }
```

Key design decisions for `no_std`:
- Use `libm` for math functions instead of `std::f32` (the `libm` crate provides software implementations of `sinf`, `expf`, `tanf`, etc. that work without libc)
- Pre-allocated delay lines (maximum sizes determined at construction time, no dynamic allocation in the audio path)
- All state stored in structs (no thread-locals, no statics, no `Rc`/`Arc`)
- `core::array::from_fn` for fixed-size array initialization (e.g., reverb comb/allpass banks)

## Parameter Smoothing

Parameters use `SmoothedParam` to avoid zipper noise.

**Why per-sample smoothing?** When a user turns a knob, the parameter changes in discrete steps. Without smoothing, gain changes produce audible clicks ("zipper noise") and filter cutoff changes produce crackles. The `SmoothedParam` (`crates/sonido-core/src/param.rs`) implements a one-pole IIR lowpass filter on the parameter value itself: `current = current + coeff * (target - current)`. The coefficient is derived from the desired smoothing time: `coeff = 1 - exp(-1 / (time_ms/1000 * sample_rate))`. After one time constant, the value reaches 63% of the way to the target; after 5 time constants (~99.3%), it is effectively settled.

**Why exponential rather than linear?** Exponential smoothing (`SmoothedParam`) is the default because it has constant computational cost (one multiply-add per sample) and naturally decelerates as it approaches the target, producing smooth-sounding transitions. Linear smoothing (`LinearSmoothedParam`) is also available for cases like crossfades where a predictable, constant-rate transition is needed.

```rust
// Exponential smoothing (default, natural-sounding)
let mut gain = SmoothedParam::with_config(1.0, 48000.0, 10.0); // 10ms smoothing

// In audio callback
gain.set_target(0.5);  // Will smooth to 0.5 over ~10ms
for sample in buffer {
    *sample *= gain.advance();  // Get smoothed value per sample
}
```

Typical smoothing times:
- Gain/pan: 5-10ms
- Filter cutoff: 20-50ms
- Gradual transitions: 100ms+

## Hardware Targets

### Electrosmith Daisy Seed / Hothouse

The `sonido-platform` crate provides abstractions for hardware deployment:

**Target Hardware:**
- Electrosmith Daisy Seed (ARM Cortex-M7 @ 480MHz, 64MB SDRAM)
- Cleveland Music Co. Hothouse (pedal enclosure with 6 knobs, 3 toggles, 2 footswitches)

**Control Layout:**
| Control | Type | Values |
|---------|------|--------|
| KNOB_1-6 | 10K pot (ADC) | 0.0 - 1.0 |
| TOGGLE_1-3 | 3-way switch | UP / MIDDLE / DOWN |
| FOOTSWITCH_1-2 | Momentary | Pressed / Released |
| LED_1-2 | Status LED | On / Off |

**Bank/Preset System (27 configurations):**
```
TOGGLE_1: Bank (A / B / C)
TOGGLE_2: Preset within bank (1 / 2 / 3)
TOGGLE_3: Mode (normal / alt / dev)
```

See `docs/HARDWARE.md` for detailed pin mappings and design patterns.

## Build Targets

```bash
# Desktop (default)
cargo build
cargo run -p sonido-cli -- process input.wav output.wav --effect reverb
cargo run -p sonido-gui

# Tests including no_std
cargo test
cargo test --no-default-features -p sonido-core
cargo test --no-default-features -p sonido-effects

# Embedded (future)
cargo build -p sonido-hothouse --target thumbv7em-none-eabihf --release
```
