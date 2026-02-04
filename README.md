# Sonido DSP Framework

Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.

## Features

- **15 audio effects**: distortion, compressor, chorus, delay, filter, vibrato, tape saturation, preamp, reverb, tremolo, gate, flanger, phaser, wah, parametric EQ
- **Synthesis engine**: Oscillators with PolyBLEP anti-aliasing, ADSR envelopes, voice management, modulation matrix
- **Cross-frequency coupling analysis**: Phase-amplitude coupling (PAC) for biosignal research (EEG, electric fish, etc.)
- **no_std compatible**: Core primitives work on embedded targets without heap allocation
- **Stereo-first processing**: True stereo effects with decorrelated L/R, backwards-compatible mono API
- **Real-time audio I/O**: Process live audio via the CLI or GUI
- **Professional GUI**: egui-based interface with real-time metering, preset management, and drag-and-drop effect chaining
- **Spectral analysis toolkit**: FFT-based tools for reverse engineering hardware effects
- **Zero-cost effect chaining**: Build complex signal chains with static or dynamic composition
- **Parameter smoothing**: Click-free automation with exponential and linear smoothing
- **Tempo sync**: Musical note divisions for tempo-synchronized effects

## Why Sonido?

| Feature | Typical Crates | Sonido |
|---------|---------------|--------|
| Parameter smoothing | None or ad-hoc | Two strategies (exponential + linear) |
| Effect chaining | `Vec<Box<dyn Effect>>` | Zero-cost static `Chain<A,B>` |
| no_std support | Afterthought | First-class design principle |
| Oversampling | Per-effect or missing | Generic `Oversampled<N, E>` wrapper |
| Latency reporting | Missing | Built-in for DAW compensation |
| Documentation | Sparse | Every public item documented |
| Testing | Minimal | 290+ unit tests |

## Quick Start

Add sonido to your `Cargo.toml`:

```toml
[dependencies]
sonido-core = "0.1"
sonido-effects = "0.1"
```

Create and process audio through an effect:

```rust
use sonido_core::Effect;
use sonido_effects::Distortion;

// Create a distortion effect at 48kHz
let mut distortion = Distortion::new(48000.0);
distortion.set_drive_db(15.0);
distortion.set_tone_hz(4000.0);

// Process audio sample by sample
let output = distortion.process(input_sample);

// Or process blocks for efficiency
distortion.process_block(&input_buffer, &mut output_buffer);
```

### Effect Chaining

```rust
use sonido_core::{Effect, EffectExt};
use sonido_effects::{Distortion, Chorus, Delay, Reverb};

// Create and configure effects
let dist = Distortion::new(48000.0);
let chorus = Chorus::new(48000.0);
let delay = Delay::new(48000.0);
let reverb = Reverb::new(48000.0);

// Chain with zero-cost static dispatch (no heap allocation)
let mut chain = dist.chain(chorus).chain(delay).chain(reverb);

// Process entire buffer
chain.process_block(&input, &mut output);
```

## Crate Overview

| Crate | Description | no_std |
|-------|-------------|--------|
| `sonido-core` | DSP primitives: Effect trait, parameters, delays, filters, LFOs, tempo | Yes |
| `sonido-effects` | Effect implementations: distortion, compressor, chorus, delay, etc. | Yes |
| `sonido-synth` | Synthesis: oscillators, envelopes, voice management, modulation matrix | Yes |
| `sonido-registry` | Effect factory and discovery by name/category | Yes |
| `sonido-platform` | Hardware abstraction: PlatformController trait, ControlMapper, ControlId | Yes |
| `sonido-analysis` | Spectral analysis, PAC/CFC analysis, Hilbert transform, filter banks | No |
| `sonido-io` | Audio I/O: WAV files (mono/stereo), real-time streaming via cpal | No |
| `sonido-cli` | Command-line interface for processing and analysis | No |
| `sonido-gui` | Real-time effects GUI with preset management | No |

## CLI Usage

Install the CLI:

```bash
cargo install --path crates/sonido-cli
```

### Process audio files

```bash
# Apply a single effect
sonido process input.wav output.wav --effect distortion --param drive=15

# Chain multiple effects
sonido process input.wav output.wav --chain "preamp:gain=6|distortion:drive=12|delay:time=300"

# Use a preset file
sonido process input.wav output.wav --preset presets/guitar_crunch.toml
```

### Real-time processing

```bash
# Process live audio through effects
sonido realtime --effect chorus --param rate=2 --param depth=0.6

# Use a preset
sonido realtime --preset presets/tape_delay.toml
```

### Generate test signals

```bash
# Generate a sine sweep for analysis
sonido generate sweep sweep.wav --start 20 --end 20000 --duration 3.0

# Generate a test tone
sonido generate tone tone.wav --freq 440 --duration 2.0

# Generate noise
sonido generate noise noise.wav --duration 1.0 --amplitude 0.5
```

### Analyze audio

```bash
# Compute frequency spectrum
sonido analyze spectrum recording.wav --fft-size 4096 --peaks 10

# Measure transfer function between input and output
sonido analyze transfer dry.wav wet.wav --output response.json

# Extract impulse response from sweep recording
sonido analyze ir sweep.wav recorded.wav --output ir.wav

# Analyze phase-amplitude coupling (for EEG/biosignal research)
sonido analyze pac eeg.wav --phase-low 4 --phase-high 8 --amp-low 30 --amp-high 80

# Generate comodulogram across frequency pairs
sonido analyze comodulogram eeg.wav --phase-range 2-20 --amp-range 20-200 -o comod.csv
```

### List available effects

```bash
sonido effects
```

## GUI

Launch the real-time effects processor GUI:

```bash
cargo run -p sonido-gui --release
```

The GUI provides:
- Drag-and-drop effect chain builder
- Real-time input/output metering
- Professional knob controls for all parameters
- Preset save/load with categories
- Dark theme optimized for studio use

## Building

```bash
# Build all crates
cargo build --release

# Run tests
cargo test

# Run benchmarks
cargo bench

# Check no_std compatibility
cargo test --no-default-features -p sonido-core
cargo test --no-default-features -p sonido-effects
```

## Performance

Measured on Intel Core i5-6300U @ 2.40 GHz, block size 256 samples at 48 kHz:

| Effect | µs/block (256) | ns/sample | CPU % (mono, 48 kHz) |
|--------|---------------|-----------|---------------------|
| CleanPreamp | 2.2 | 9 | 0.04% |
| LowPassFilter | 3.4 | 13 | 0.06% |
| Delay | 3.1 | 12 | 0.06% |
| TapeSaturation | 6.7 | 26 | 0.13% |
| Distortion | 14.4 | 56 | 0.27% |
| Chorus | 20.4 | 80 | 0.38% |
| Compressor | 29.1 | 113 | 0.54% |
| Reverb | 44.5 | 174 | 0.83% |
| MultiVibrato | 73.4 | 287 | 1.38% |
| EffectChain (5 effects) | 42.8 | 167 | 0.80% |

CPU % = `ns_per_sample / (1e9 / 48000) × 100`. All effects comfortably fit within a real-time audio callback budget. Run your own: `cargo bench -p sonido-effects`

## Audio Demos

Pre-generated audio samples demonstrating synthesis and effect processing are in [`demos/`](demos/):

| File | Description |
|------|-------------|
| `src_sine_440.wav` | Clean 440 Hz sine tone (dry reference) |
| `src_saw_chord.wav` | PolyBLEP sawtooth C major chord with ADSR envelope |
| `src_perc_adsr.wav` | Short percussive hit for reverb/delay demos |
| `fx_distortion_soft.wav` | Soft-clip distortion — harmonic generation |
| `fx_distortion_hard.wav` | Hard-clip distortion — aggressive saturation |
| `fx_chorus.wav` | Stereo chorus on saw chord |
| `fx_reverb_hall.wav` | Hall reverb on percussive transient |
| `fx_delay.wav` | Tempo-synced delay with feedback |
| `fx_full_chain.wav` | 5-effect chain: preamp → distortion → chorus → delay → reverb |

Regenerate all: `./scripts/generate_demos.sh`

## Documentation

### Design & Theory
- [DSP Fundamentals](docs/DSP_FUNDAMENTALS.md) -- signal processing theory behind the implementations
- [Design Decisions](docs/DESIGN_DECISIONS.md) -- architecture decision records with rationale
- [Architecture Overview](docs/ARCHITECTURE.md) -- crate structure and data flow

### User Guides
- [Getting Started Guide](docs/GETTING_STARTED.md)
- [CLI Guide](docs/CLI_GUIDE.md)
- [Effects Reference](docs/EFFECTS_REFERENCE.md)
- [Synthesis Guide](docs/SYNTHESIS.md)
- [GUI Documentation](docs/GUI.md)

### Specialized Topics
- [CFC/PAC Analysis Guide](docs/CFC_ANALYSIS.md)
- [Biosignal Analysis](docs/BIOSIGNAL_ANALYSIS.md)
- [Hardware Targets](docs/HARDWARE.md)

### Development
- [Contributing](docs/CONTRIBUTING.md)
- [Testing](docs/TESTING.md)
- [Benchmarks](docs/BENCHMARKS.md)
- [Changelog](docs/CHANGELOG.md)

## audioDNA: Reverse-Engineering Reference Implementations

Sonido's effect algorithms are informed by analysis of commercial DSP products.
These are clean-room implementations -- no proprietary code or firmware was used.
The goal is to demonstrate deep understanding of production DSP architectures.

| Target Product | DSP Domain | Sonido Implementation |
|----------------|------------|----------------------|
| DigiTech Ventura / Modela | Modulation (chorus, vibrato, rotary) | `Chorus`, `MultiVibrato`, LFO engine |
| DigiTech Obscura | Delay (analog, tape, lo-fi modes) | `Delay` with feedback coloring, `TapeSaturation` |
| DigiTech Dirty Robot | Envelope-following filter / synth | `Wah` (auto-wah mode), `LowPassFilter`, synth engine |
| DigiTech Polara / Supernatural | Reverb (room, hall, plate, spring) | `Reverb` (Freeverb topology with stereo decorrelation) |

See `sonido compare` CLI command for A/B measurement between hardware captures and Sonido output.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
