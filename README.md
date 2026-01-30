# Sonido DSP Framework

Production-grade DSP library in Rust for audio effects, plugins, and embedded systems.

## Features

- **8 audio effects**: distortion, compressor, chorus, delay, filter, vibrato, tape saturation, preamp
- **no_std compatible**: Core primitives work on embedded targets without heap allocation
- **Real-time audio I/O**: Process live audio via the CLI
- **Spectral analysis toolkit**: FFT-based tools for reverse engineering hardware effects
- **Effect chaining**: Build complex signal chains with static or dynamic composition

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

## Crate Overview

| Crate | Description | no_std |
|-------|-------------|--------|
| `sonido-core` | DSP primitives: Effect trait, parameters, delays, filters, LFOs | Yes |
| `sonido-effects` | Effect implementations: distortion, compressor, chorus, delay, etc. | Yes |
| `sonido-analysis` | Spectral analysis tools for reverse engineering (FFT, transfer functions) | No |
| `sonido-io` | Audio I/O: WAV files, real-time streaming via cpal | No |
| `sonido-cli` | Command-line interface for processing and analysis | No |

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
```

### List available effects

```bash
sonido effects
```

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

## Documentation

- [Architecture Overview](docs/ARCHITECTURE.md)
- [Getting Started Guide](docs/GETTING_STARTED.md)
- [CLI Guide](docs/CLI_GUIDE.md)
- [Effects Reference](docs/EFFECTS_REFERENCE.md)
- [Contributing](docs/CONTRIBUTING.md)
- [Changelog](docs/CHANGELOG.md)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
