# Sonido Presets

This directory contains preset files for the Sonido CLI.

## Usage

```bash
# Process a file with a preset
sonido process input.wav output.wav --preset presets/guitar_crunch.toml

# Real-time processing with a preset
sonido realtime --preset presets/tape_delay.toml
```

## Preset Format

Presets are TOML files with the following structure:

```toml
name = "Preset Name"
description = "Optional description"

[[effects]]
type = "effect_name"
[effects.params]
param1 = "value1"
param2 = "value2"

[[effects]]
type = "another_effect"
[effects.params]
param = "value"
```

## Available Effects

| Effect | Parameters |
|--------|------------|
| `distortion` | drive, tone, level, waveshape |
| `compressor` | threshold, ratio, attack, release, makeup |
| `chorus` | rate, depth, mix |
| `delay` | time, feedback, mix |
| `filter` | cutoff, resonance |
| `multivibrato` | depth |
| `tape` | drive, saturation, hf_rolloff, bias, wow, flutter, hysteresis, head_bump, bump_freq, output |
| `preamp` | gain, headroom, output |
| `eq` | low_freq, low_gain, low_q, mid_freq, mid_gain, mid_q, high_freq, high_gain, high_q, output |
| `bitcrusher` | bit_depth, downsample, jitter, mix, output |
| `limiter` | threshold, ceiling, release, lookahead, output |
| `gate` | threshold, attack, release, hold, output |

## Included Presets

- **guitar_crunch.toml** - Classic overdrive sound with subtle compression
- **tape_delay.toml** - Warm tape-style echo with saturation
- **subtle_chorus.toml** - Light stereo widening effect
- **clean_boost.toml** - Transparent gain boost
- **full_chain.toml** - Complete signal chain example
- **justice_dance_bass.toml** - Gritty, compressed bass inspired by Justice - D.A.N.C.E.

## Creating Custom Presets

1. Copy an existing preset as a starting point
2. Modify effect types and parameters
3. Test with `sonido process` or `sonido realtime`
4. Save with a descriptive name
