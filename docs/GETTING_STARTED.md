# Getting Started

This guide walks you through creating your first audio effect with Sonido.

## Prerequisites

- Rust 1.75 or later
- For real-time audio: platform audio drivers (ALSA on Linux, CoreAudio on macOS, WASAPI on Windows)

## Installation

### As a Library

Add the crates you need to your `Cargo.toml`:

```toml
[dependencies]
sonido-core = "0.1"      # Core traits and primitives
sonido-effects = "0.1"   # Effect implementations
```

### CLI Tool

Build and install the CLI:

```bash
git clone https://github.com/suds/sonido
cd sonido
cargo install --path crates/sonido-cli
```

## Quick Start Options

### GUI (Graphical Interface)

For a visual, interactive experience:

```bash
# Run the GUI application
cargo run -p sonido-gui

# Or install and run
cargo install --path crates/sonido-gui
sonido-gui
```

See [GUI.md](GUI.md) for detailed GUI documentation.

### Demo Script

Generate test audio and hear effects in action:

```bash
make demo
```

This generates a sweep signal and processes it through various effect chains.

### Example Code

Run the chain demo example:

```bash
cargo run -p sonido-effects --example chain_demo
```

---

## Your First Effect

Let's create a simple distortion effect:

```rust
use sonido_core::Effect;
use sonido_effects::Distortion;

fn main() {
    // Create effect at 48kHz sample rate
    let mut distortion = Distortion::new(48000.0);

    // Configure parameters
    distortion.set_drive_db(12.0);   // 12dB of gain into the waveshaper
    distortion.set_tone_hz(4000.0);  // Lowpass at 4kHz to tame harshness
    distortion.set_level_db(-6.0);   // Output level

    // Generate a test signal (sine wave)
    let sample_rate = 48000.0;
    let frequency = 440.0;  // A4
    let duration_samples = 48000;  // 1 second

    let input: Vec<f32> = (0..duration_samples)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5
        })
        .collect();

    // Process the audio
    let mut output = vec![0.0; input.len()];
    distortion.process_block(&input, &mut output);

    println!("Processed {} samples", output.len());
}
```

## Effect Chaining

Chain multiple effects together:

```rust
use sonido_core::Effect;
use sonido_effects::{CleanPreamp, Distortion, Delay};

fn main() {
    let sample_rate = 48000.0;

    // Create effects
    let mut preamp = CleanPreamp::new(sample_rate);
    preamp.set_gain_db(6.0);  // Boost input

    let mut distortion = Distortion::new(sample_rate);
    distortion.set_drive_db(15.0);

    let mut delay = Delay::new(sample_rate);
    delay.set_delay_time_ms(300.0);
    delay.set_feedback(0.4);
    delay.set_mix(0.3);

    // Process through the chain
    let input = vec![0.5f32; 1024];  // Example input
    let mut buffer = vec![0.0; 1024];
    let mut output = vec![0.0; 1024];

    preamp.process_block(&input, &mut buffer);
    distortion.process_block(&buffer, &mut output);
    buffer.copy_from_slice(&output);
    delay.process_block(&buffer, &mut output);

    // 'output' now contains the processed audio
}
```

## Dynamic Effect Chains

For runtime-configurable chains, use `Box<dyn Effect>`:

```rust
use sonido_core::Effect;
use sonido_effects::{Distortion, Chorus, Delay};

fn main() {
    let sample_rate = 48000.0;

    // Build a dynamic chain
    let mut chain: Vec<Box<dyn Effect + Send>> = Vec::new();

    chain.push(Box::new(Distortion::new(sample_rate)));
    chain.push(Box::new(Chorus::new(sample_rate)));
    chain.push(Box::new(Delay::new(sample_rate)));

    // Process through the chain
    let input = vec![0.5f32; 1024];
    let mut buffer1 = vec![0.0; 1024];
    let mut buffer2 = vec![0.0; 1024];

    buffer1.copy_from_slice(&input);

    for effect in &mut chain {
        effect.process_block(&buffer1, &mut buffer2);
        std::mem::swap(&mut buffer1, &mut buffer2);
    }

    // Result is in buffer1
}
```

## Using the CLI

Process a file through effects:

```bash
# Simple distortion
sonido process input.wav output.wav --effect distortion --param drive=15

# Effect chain
sonido process input.wav output.wav \
    --chain "preamp:gain=6|distortion:drive=12|delay:time=300,feedback=0.4"
```

Real-time processing:

```bash
# List available audio devices
sonido devices

# Process live audio
sonido realtime --effect chorus --param rate=2 --param depth=0.6
```

## Working with WAV Files

```rust
use sonido_io::{read_wav, write_wav, WavSpec};
use sonido_effects::Compressor;
use sonido_core::Effect;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Read input file
    let (samples, spec) = read_wav("input.wav")?;
    let sample_rate = spec.sample_rate as f32;

    // Create and configure effect
    let mut compressor = Compressor::new(sample_rate);
    compressor.set_threshold_db(-18.0);
    compressor.set_ratio(4.0);
    compressor.set_attack_ms(10.0);
    compressor.set_release_ms(100.0);

    // Process
    let mut output = vec![0.0; samples.len()];
    compressor.process_block(&samples, &mut output);

    // Write output file
    let out_spec = WavSpec {
        channels: 1,
        sample_rate: spec.sample_rate,
        bits_per_sample: 32,
    };
    write_wav("output.wav", &output, out_spec)?;

    Ok(())
}
```

## Embedded/no_std Usage

For embedded systems without standard library:

```toml
[dependencies]
sonido-core = { version = "0.1", default-features = false }
sonido-effects = { version = "0.1", default-features = false }
```

```rust
#![no_std]

use sonido_core::Effect;
use sonido_effects::LowPassFilter;

// Pre-allocated buffers
static mut INPUT: [f32; 256] = [0.0; 256];
static mut OUTPUT: [f32; 256] = [0.0; 256];

fn process_audio(filter: &mut LowPassFilter) {
    unsafe {
        filter.process_block(&INPUT, &mut OUTPUT);
    }
}
```

## Next Steps

- See [GUI.md](GUI.md) for graphical interface documentation
- See [Effects Reference](EFFECTS_REFERENCE.md) for all effects and their parameters
- See [CLI Guide](CLI_GUIDE.md) for detailed CLI usage
- See [Architecture](ARCHITECTURE.md) for understanding the codebase
