# Architecture

## Crate Diagram

```
                    ┌─────────────────┐
                    │   sonido-cli    │
                    │  (binary crate) │
                    └────────┬────────┘
                             │
         ┌───────────────────┼───────────────────┐
         │                   │                   │
         ▼                   ▼                   ▼
┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
│   sonido-io     │ │ sonido-effects  │ │ sonido-analysis │
│  (audio I/O)    │ │   (effects)     │ │    (FFT/IR)     │
└────────┬────────┘ └────────┬────────┘ └────────┬────────┘
         │                   │                   │
         └───────────────────┼───────────────────┘
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
- `DelayLine`: Sample-accurate delay buffer with fractional interpolation
- `Biquad`: IIR filter building block for EQ, lowpass, highpass, etc.
- `Lfo`: Low-frequency oscillator for modulation effects
- `Oversampling`: 2x/4x oversampling for anti-aliasing in nonlinear effects

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
