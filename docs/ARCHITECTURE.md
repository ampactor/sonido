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

Central registry for discovering and instantiating effects. Provides a unified
API for CLI, GUI, and future hardware targets.

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

Hardware abstraction layer for multi-target deployment. Provides `no_std` compatible traits
for physical controls and parameter mapping.

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

All effects implement the `Effect` trait with stereo-first design:

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

All effects implement the `ParameterInfo` trait for runtime parameter discovery:

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

`sonido-core` and `sonido-effects` support `no_std` for embedded use:

```toml
[dependencies]
sonido-core = { version = "0.1", default-features = false }
sonido-effects = { version = "0.1", default-features = false }
```

Key design decisions for `no_std`:
- Use `libm` for math functions instead of `std::f32`
- Pre-allocated delay lines (no dynamic allocation in audio path)
- All state stored in structs (no thread-locals or statics)

## Parameter Smoothing

Parameters use `SmoothedParam` to avoid zipper noise:

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
