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
| `-p, --preset <NAME>` | Preset name or path (factory, user, or file) |
| `--param <KEY=VALUE>` | Effect parameter |
| `-i, --input <DEVICE>` | Input device (index, name, or partial name) |
| `-o, --output <DEVICE>` | Output device (index, name, or partial name) |
| `--sample-rate <N>` | Sample rate (default: 48000) |
| `--buffer-size <N>` | Buffer size in samples (default: 256) |
| `--mono` | Force mono processing |

### Device Selection

Devices can be selected by:
- **Index**: Use the number shown in `sonido devices list` (e.g., `--input 0`)
- **Exact name**: Full device name (e.g., `--input "USB Audio Interface"`)
- **Partial name**: Case-insensitive substring match (e.g., `--input "USB"`)

### Examples

```bash
# Simple chorus effect
sonido realtime --effect chorus --param rate=2 --param depth=0.6

# Select devices by index
sonido realtime --input 0 --output 0 --effect reverb

# Select devices by partial name match
sonido realtime --input "USB" --output "USB" --effect distortion

# Chain with custom devices
sonido realtime \
    --chain "preamp:gain=6|distortion:drive=12" \
    --input "USB Audio" \
    --output "Built-in Output"

# Using a preset
sonido realtime --preset tape_warmth

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
| `--rt60` | Estimate and display RT60 reverberation time |

```bash
# 1. Generate sweep
sonido generate sweep sweep.wav --duration 3.0

# 2. Play through system and record
# (external step)

# 3. Extract IR
sonido analyze ir sweep.wav recorded.wav -o impulse_response.wav

# With RT60 estimation
sonido analyze ir sweep.wav recorded.wav -o impulse_response.wav --rt60
```

#### distortion

Analyze harmonic distortion (THD, THD+N).

```bash
sonido analyze distortion <INPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--fundamental <HZ>` | Fundamental frequency (auto-detected if omitted) |
| `--fft-size <N>` | FFT size (default: 8192) |
| `-o, --output <FILE>` | Output JSON file |

```bash
# Analyze distortion of a test tone
sonido analyze distortion test_tone.wav --fft-size 16384

# With known fundamental
sonido analyze distortion 1khz_through_amp.wav --fundamental 1000
```

#### spectrogram

Generate time-frequency spectrogram.

```bash
sonido analyze spectrogram <INPUT> -o <OUTPUT.csv> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--fft-size <N>` | FFT size (default: 2048) |
| `--hop <N>` | Hop size (default: fft_size / 4) |
| `-o, --output <FILE>` | Output CSV file (required) |

```bash
sonido analyze spectrogram recording.wav -o spectrogram.csv --fft-size 4096 --hop 512
```

#### dynamics

Analyze dynamics (RMS, crest factor, dynamic range).

```bash
sonido analyze dynamics <INPUT>
```

```bash
sonido analyze dynamics master.wav
```

Output includes:
- Peak level (dBFS)
- RMS level (dBFS)
- Crest factor (dB)
- Dynamic range (dB)
- Headroom (dB)

#### pac

Analyze Phase-Amplitude Coupling between frequency bands.

```bash
sonido analyze pac <INPUT> [OPTIONS]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--phase-low <HZ>` | Phase band lower frequency | 4.0 |
| `--phase-high <HZ>` | Phase band upper frequency | 8.0 |
| `--amp-low <HZ>` | Amplitude band lower frequency | 30.0 |
| `--amp-high <HZ>` | Amplitude band upper frequency | 100.0 |
| `--method <METHOD>` | `mvl` (Mean Vector Length) or `kl` (Kullback-Leibler) | mvl |
| `--surrogates <N>` | Number of surrogate iterations for significance testing | 0 |
| `-o, --output <FILE>` | Output JSON file | - |

```bash
# Analyze theta-gamma coupling
sonido analyze pac eeg_recording.wav \
    --phase-low 4 --phase-high 8 \
    --amp-low 30 --amp-high 80 \
    --method mvl

# With surrogate significance testing
sonido analyze pac eeg_recording.wav \
    --phase-low 4 --phase-high 8 \
    --amp-low 30 --amp-high 100 \
    --surrogates 200 \
    --output pac_results.json
```

Output includes:
- Modulation Index (0-1 coupling strength)
- Preferred phase (radians and degrees)
- Amplitude distribution by phase bin
- p-value (if surrogates > 0)

See [CFC_ANALYSIS.md](CFC_ANALYSIS.md) for detailed PAC analysis documentation.

#### comodulogram

Compute coupling across multiple frequency pairs.

```bash
sonido analyze comodulogram <INPUT> -o <OUTPUT.csv> [OPTIONS]
```

| Option | Description | Default |
|--------|-------------|---------|
| `--phase-range <LOW-HIGH>` | Phase frequency range | 2-20 |
| `--amp-range <LOW-HIGH>` | Amplitude frequency range | 20-200 |
| `--phase-step <HZ>` | Phase frequency step | 2.0 |
| `--amp-step <HZ>` | Amplitude frequency step | 10.0 |
| `--bandwidth <RATIO>` | Bandwidth as fraction of center frequency | 0.5 |
| `-o, --output <FILE>` | Output CSV file (required) | - |

```bash
# Full comodulogram
sonido analyze comodulogram recording.wav \
    --phase-range 2-20 \
    --amp-range 20-200 \
    --phase-step 2 \
    --amp-step 10 \
    --output comodulogram.csv
```

The output CSV can be visualized as a heatmap showing coupling strength across frequency pairs.

#### bandpass

Extract a frequency band using bandpass filtering.

```bash
sonido analyze bandpass <INPUT> -o <OUTPUT.wav> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--low <HZ>` | Lower cutoff frequency (required) |
| `--high <HZ>` | Upper cutoff frequency (required) |
| `--order <N>` | Filter order: 2, 4, or 6 (default: 4) |
| `-o, --output <FILE>` | Output WAV file (required) |

```bash
# Extract theta band (4-8 Hz)
sonido analyze bandpass eeg.wav --low 4 --high 8 -o theta_band.wav

# Extract with higher-order filter for sharper cutoff
sonido analyze bandpass eeg.wav --low 30 --high 80 --order 6 -o gamma_band.wav
```

#### hilbert

Extract instantaneous phase and amplitude using Hilbert transform.

```bash
sonido analyze hilbert <INPUT> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--phase-output <FILE>` | Output WAV file for phase |
| `--amp-output <FILE>` | Output WAV file for amplitude envelope |
| `--bandpass <LOW-HIGH>` | Optional bandpass filter before transform |

```bash
# Extract amplitude envelope
sonido analyze hilbert recording.wav --amp-output envelope.wav

# Extract phase after bandpass filtering
sonido analyze hilbert eeg.wav \
    --bandpass 4-8 \
    --phase-output theta_phase.wav \
    --amp-output theta_amplitude.wav
```

The phase output is normalized to [-1, 1] (representing [-pi, pi] radians).
The amplitude output is normalized to [0, 1].

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

### Subcommands

#### list

List all available audio devices with indices.

```bash
sonido devices list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--include-virtual` | Show virtual/loopback device info and setup guidance |

Output shows numbered devices that can be used with `--input` and `--output`:

```
Input Devices:
  [0] Built-in Microphone (48000 Hz)
  [1] USB Audio Interface (44100 Hz)

Output Devices:
  [0] Built-in Speaker (48000 Hz)
  [1] USB Audio Interface (44100 Hz)
```

#### info

Show default device information.

```bash
sonido devices info
```

### Virtual Audio / Loopback

To capture system audio (e.g., for recording what's playing), use `--include-virtual` to see loopback device guidance:

```bash
sonido devices list --include-virtual
```

If no loopback devices are detected, platform-specific installation instructions are shown:
- **Windows**: VB-Audio Virtual Cable
- **macOS**: BlackHole
- **Linux**: PulseAudio/PipeWire module-loopback

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

## presets

Manage effect presets (list, show, save, delete, export).

### Subcommands

#### list

List available presets.

```bash
sonido presets list [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--factory` | Show only factory presets |
| `--user` | Show only user presets |

#### show

Show details of a preset.

```bash
sonido presets show <NAME>
```

#### save

Save an effect chain as a user preset.

```bash
sonido presets save <NAME> --chain <SPEC> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-c, --chain <SPEC>` | Effect chain specification (required) |
| `-d, --description <TEXT>` | Preset description |
| `--force` | Overwrite if preset exists |

```bash
sonido presets save my_tone --chain "preamp:gain=6|distortion:drive=12" \
    --description "My custom crunch tone"
```

#### delete

Delete a user preset.

```bash
sonido presets delete <NAME> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--force` | Don't ask for confirmation |

#### copy

Copy a factory preset to user presets for customization.

```bash
sonido presets copy <SOURCE> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `-n, --name <NAME>` | New preset name (default: source name) |

```bash
sonido presets copy crunch --name my_crunch
```

#### export-factory

Export all factory presets to a directory as individual TOML files.

```bash
sonido presets export-factory <OUTPUT_DIR> [OPTIONS]
```

| Option | Description |
|--------|-------------|
| `--force` | Overwrite existing files |

```bash
sonido presets export-factory ./presets/
```

This is useful for:
- Distributing presets with release builds
- Inspecting factory preset configurations
- Using as templates for custom presets

#### paths

Show preset directory locations.

```bash
sonido presets paths
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
