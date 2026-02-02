# Architecture

## Crate Diagram

```
          ┌─────────────────┐         ┌─────────────────┐
          │   sonido-cli    │         │   sonido-gui    │
          │  (binary crate) │         │  (egui app)     │
          └────────┬────────┘         └────────┬────────┘
                   │                           │
                   └───────────┬───────────────┘
                               │
                               ▼
                      ┌─────────────────┐
                      │ sonido-registry │
                      │(effect factory) │
                      └────────┬────────┘
                               │
         ┌─────────────────────┼─────────────────────┐
         │                     │                     │
         ▼                     ▼                     ▼
┌─────────────────┐   ┌─────────────────┐   ┌─────────────────┐
│   sonido-io     │   │ sonido-effects  │   │ sonido-analysis │
│  (audio I/O)    │   │   (effects)     │   │    (FFT/IR)     │
└────────┬────────┘   └────────┬────────┘   └────────┬────────┘
         │                     │                     │
         └─────────────────────┼─────────────────────┘
                               │
                               ▼
                      ┌─────────────────┐
                      │   sonido-core   │
                      │  (primitives)   │
                      │    [no_std]     │
                      └─────────────────┘
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

### sonido-effects

Audio effect implementations built on sonido-core. Also `no_std` compatible.

**Effects:**
- `Distortion`: Waveshaping with soft clip, hard clip, foldback, asymmetric modes
- `Compressor`: Dynamics compressor with soft knee, attack/release, makeup gain
- `Chorus`: Dual-voice modulated delay with LFO
- `Delay`: Tape-style feedback delay with filtering
- `LowPassFilter`: Resonant 2-pole lowpass
- `MultiVibrato`: 10-unit tape wow/flutter simulation
- `TapeSaturation`: Tape-style saturation with HF rolloff
- `CleanPreamp`: Simple gain stage
- `Reverb`: Freeverb-style algorithmic reverb with 8 combs + 4 allpasses

### sonido-analysis

Spectral analysis tools for reverse engineering hardware. Requires `std` for FFT.

**Components:**
- `Fft`: FFT wrapper around rustfft
- `Window`: Window functions (Hamming, Blackman, Hann)
- `TransferFunction`: Measure frequency response between two signals
- `SineSweep`: Generate logarithmic sine sweeps for IR capture

### sonido-io

Audio I/O layer using cpal and hound.

**Components:**
- `read_wav` / `write_wav`: WAV file I/O with format conversion
- `AudioStream`: Real-time audio streaming
- `ProcessingEngine`: Block-based effect chain runner

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

### sonido-cli

Command-line interface tying everything together.

**Commands:**
- `process`: File-based effect processing
- `realtime`: Live audio processing
- `generate`: Test signal generation
- `analyze`: Spectral analysis
- `compare`: A/B audio comparison
- `devices`: Audio device management
- `effects`: List available effects

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

### File Processing

```
┌─────────┐    ┌──────────┐    ┌────────────────┐    ┌──────────┐
│ WAV     │───▶│ read_wav │───▶│ ProcessingEngine│───▶│ write_wav│
│ input   │    └──────────┘    │  (effects)      │    └──────────┘
└─────────┘                    └────────────────┘           │
                                                           ▼
                                                      ┌─────────┐
                                                      │ WAV     │
                                                      │ output  │
                                                      └─────────┘
```

### Real-time Processing

```
┌──────────┐    ┌──────────────┐    ┌────────────────┐    ┌──────────┐
│ Audio    │───▶│ AudioStream  │───▶│ ProcessingEngine│───▶│ Audio    │
│ input    │    │ (cpal)       │    │  (effects)      │    │ output   │
└──────────┘    └──────────────┘    └────────────────┘    └──────────┘
```

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

All effects implement the `Effect` trait:

```rust
pub trait Effect {
    /// Process a single sample
    fn process(&mut self, input: f32) -> f32;

    /// Process a block of samples (default: calls process() per sample)
    fn process_block(&mut self, input: &[f32], output: &mut [f32]);

    /// Update sample rate (call when rate changes)
    fn set_sample_rate(&mut self, sample_rate: f32);

    /// Reset internal state (call when starting new audio)
    fn reset(&mut self);

    /// Report latency for delay compensation
    fn latency_samples(&self) -> usize;
}
```

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
