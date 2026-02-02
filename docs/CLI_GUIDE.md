# CLI Guide

Complete reference for the `sonido` command-line tool.

## Installation

```bash
cargo install --path crates/sonido-cli
```

Or build from source:

```bash
cargo build --release -p sonido-cli
# Binary at target/release/sonido
```

## Commands Overview

| Command | Description |
|---------|-------------|
| `process` | Process audio files through effects |
| `realtime` | Real-time audio processing |
| `generate` | Generate test signals |
| `analyze` | Spectral analysis |
| `compare` | A/B audio comparison |
| `devices` | List audio devices |
| `effects` | List available effects |

---

## process

Process an audio file through one or more effects.

### Basic Usage

```bash
sonido process <INPUT> <OUTPUT> [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-e, --effect <NAME>` | Single effect to apply |
| `-c, --chain <SPEC>` | Effect chain specification |
| `-p, --preset <FILE>` | Preset file (TOML) |
| `--param <KEY=VALUE>` | Effect parameter (can repeat) |
| `--block-size <N>` | Processing block size (default: 512) |
| `--bit-depth <N>` | Output bit depth: 16, 24, or 32 (default: 32) |

### Examples

```bash
# Single effect with default parameters
sonido process input.wav output.wav --effect distortion

# Single effect with custom parameters
sonido process input.wav output.wav --effect distortion --param drive=15 --param tone=4000

# Effect chain (effects separated by |)
sonido process input.wav output.wav \
    --chain "preamp:gain=6|distortion:drive=12|delay:time=300,feedback=0.4"

# Using a preset file
sonido process input.wav output.wav --preset presets/guitar_crunch.toml

# Output as 16-bit WAV
sonido process input.wav output.wav --effect compressor --bit-depth 16
```

### Chain Syntax

```
effect1:param1=value1,param2=value2|effect2:param=value|effect3
```

- Effects are separated by `|`
- Parameters are separated by `,`
- Parameter names and values are separated by `=`
- Effects with no parameters can omit the colon
- Whitespace around `|` separators is trimmed (ignored)

**Edge Cases:**

```bash
# Effects without parameters can omit the colon entirely
sonido process input.wav output.wav --chain "distortion|delay:time=300"

# Whitespace around pipes is ignored
sonido process input.wav output.wav --chain "preamp:gain=6 | distortion | delay:time=300"

# Empty segments between pipes are skipped
sonido process input.wav output.wav --chain "preamp:gain=6||distortion"
```

---

## realtime

Process live audio through effects in real-time.

### Basic Usage

```bash
sonido realtime [OPTIONS]
```

### Options

| Option | Description |
|--------|-------------|
| `-e, --effect <NAME>` | Single effect to apply |
| `-c, --chain <SPEC>` | Effect chain specification |
| `-p, --preset <FILE>` | Preset file (TOML) |
| `--param <KEY=VALUE>` | Effect parameter |
| `--input-device <NAME>` | Input device name |
| `--output-device <NAME>` | Output device name |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--buffer-size <N>` | Buffer size in samples (default: 256) |

### Examples

```bash
# Simple chorus effect
sonido realtime --effect chorus --param rate=2 --param depth=0.6

# Chain with custom devices
sonido realtime \
    --chain "preamp:gain=6|distortion:drive=12" \
    --input-device "USB Audio" \
    --output-device "Built-in Output"

# Using a preset
sonido realtime --preset presets/tape_delay.toml

# Lower latency with smaller buffer
sonido realtime --effect delay --buffer-size 128
```

Press `Ctrl+C` to stop real-time processing.

---

## generate

Generate test signals for analysis and testing.

### Subcommands

#### sweep

Generate a logarithmic sine sweep (chirp).

```bash
sonido generate sweep <OUTPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--start <HZ>` | Start frequency (default: 20) |
| `--end <HZ>` | End frequency (default: 20000) |
| `--duration <SEC>` | Duration in seconds (default: 2.0) |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--amplitude <N>` | Amplitude 0-1 (default: 0.8) |

```bash
sonido generate sweep sweep.wav --start 20 --end 20000 --duration 3.0
```

#### tone

Generate a sine tone.

```bash
sonido generate tone <OUTPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--freq <HZ>` | Frequency (default: 440) |
| `--duration <SEC>` | Duration in seconds (default: 1.0) |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--amplitude <N>` | Amplitude 0-1 (default: 0.8) |

```bash
sonido generate tone a440.wav --freq 440 --duration 2.0
```

#### noise

Generate white noise.

```bash
sonido generate noise <OUTPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--duration <SEC>` | Duration in seconds (default: 1.0) |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--amplitude <N>` | Amplitude 0-1 (default: 0.5) |

```bash
sonido generate noise noise.wav --duration 1.0 --amplitude 0.3
```

#### impulse

Generate a single impulse (Dirac delta).

```bash
sonido generate impulse <OUTPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--length <N>` | Length in samples (default: 48000) |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--amplitude <N>` | Impulse amplitude (default: 1.0) |

> **Note:** Unlike other generate commands, `--length` is specified in **samples**, not seconds.
> For a 1-second impulse response at 48kHz, use `--length 48000`.

```bash
# 1 second impulse at 48kHz
sonido generate impulse impulse.wav --length 48000

# 2 second impulse at 44.1kHz
sonido generate impulse impulse.wav --length 88200 --sample-rate 44100
```

#### silence

Generate silence.

```bash
sonido generate silence <OUTPUT> [OPTIONS]
```

```bash
sonido generate silence silence.wav --duration 2.0
```

---

## analyze

Spectral analysis tools.

### Subcommands

#### spectrum

Compute and display the frequency spectrum.

```bash
sonido analyze spectrum <INPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--fft-size <N>` | FFT size (default: 4096) |
| `--window <TYPE>` | Window function: hamming, blackman, hann, rectangular (default: blackman) |
| `-o, --output <FILE>` | Output CSV file |
| `--peaks <N>` | Show top N peaks (default: 10) |

```bash
# Analyze and show top peaks
sonido analyze spectrum recording.wav --peaks 20

# Export to CSV
sonido analyze spectrum recording.wav --output spectrum.csv
```

#### transfer

Compute transfer function between input and output recordings.

```bash
sonido analyze transfer <INPUT> <OUTPUT_FILE> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--fft-size <N>` | FFT size (default: 4096) |
| `-o, --output <FILE>` | Output JSON file |

```bash
# Measure pedal response
sonido analyze transfer dry.wav wet.wav --output response.json
```

#### ir

Extract impulse response using deconvolution.

```bash
sonido analyze ir <SWEEP> <RESPONSE> -o <OUTPUT>
```

| Option | Description |
|--------|-------------|
| `-o, --output <FILE>` | Output IR WAV file (required) |

```bash
# 1. Generate sweep
sonido generate sweep sweep.wav --duration 3.0

# 2. Play through system and record
# (external step)

# 3. Extract IR
sonido analyze ir sweep.wav recorded.wav -o impulse_response.wav
```

---

## compare

Compare two audio files (A/B comparison).

```bash
sonido compare <FILE_A> <FILE_B> [OPTIONS]
```

Calculates:
- RMS difference
- Peak difference
- Correlation coefficient
- Spectral differences

---

## devices

List and manage audio devices.

```bash
sonido devices
```

Shows:
- Available input devices
- Available output devices
- Default devices
- Supported sample rates

---

## effects

List available effects and their parameters.

```bash
sonido effects
```

Shows all effects with:
- Description
- Parameter names
- Default values
- Valid ranges

### Effect Aliases

Several effects have shorter alias names that can be used interchangeably:

| Effect | Alias(es) |
|--------|-----------|
| `filter` | `lowpass` |
| `multivibrato` | `vibrato` |
| `tape` | `tapesaturation` |
| `preamp` | `cleanpreamp` |

Examples:
```bash
# These are equivalent
sonido process input.wav output.wav --effect filter --param cutoff=2000
sonido process input.wav output.wav --effect lowpass --param cutoff=2000

# These are equivalent
sonido process input.wav output.wav --effect multivibrato --param depth=0.6
sonido process input.wav output.wav --effect vibrato --param depth=0.6
```

---

## Preset Files

Presets are TOML files defining effect chains:

```toml
name = "Guitar Crunch"
description = "Classic overdrive sound"

[[effects]]
type = "preamp"
[effects.params]
gain = "6"

[[effects]]
type = "distortion"
[effects.params]
drive = "15"
tone = "4000"
level = "-6"
```

Use with:

```bash
sonido process input.wav output.wav --preset my_preset.toml
sonido realtime --preset my_preset.toml
```
