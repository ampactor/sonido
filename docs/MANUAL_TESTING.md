# Manual Testing Guide

Comprehensive manual testing protocol for the Sonido CLI, TUI, and GUI.

## Prerequisites

### Build the Project

```bash
# Build all binaries in release mode
cargo build --release -p sonido-cli -p sonido-gui

# Or use debug builds for faster iteration
cargo build -p sonido-cli -p sonido-gui
```

### Audio Device Setup

1. Connect an audio input device (microphone, audio interface, or loopback)
2. Connect an audio output device (speakers, headphones)
3. Verify devices are recognized:
   ```bash
   cargo run -p sonido-cli -- devices list
   ```

### Test Audio Files

Create or obtain test audio files. Generate test signals using the CLI:

```bash
# Create a test directory
mkdir -p test_audio

# Generate common test signals
cargo run -p sonido-cli -- generate tone test_audio/sine_440.wav --freq 440 --duration 2.0
cargo run -p sonido-cli -- generate sweep test_audio/sweep.wav --duration 3.0
cargo run -p sonido-cli -- generate noise test_audio/noise.wav --duration 2.0 --amplitude 0.3
```

---

## CLI Testing Protocol

### Effect Processing Tests

Test all 15 effects with the `process` command.

#### Individual Effect Tests

Run each effect and verify the output file is created and plays correctly:

```bash
# Preamp
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect preamp --param gain=6

# Distortion
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect distortion --param drive=15

# Compressor
cargo run -p sonido-cli -- process test_audio/noise.wav /tmp/out.wav --effect compressor --param threshold=-20

# Gate
cargo run -p sonido-cli -- process test_audio/noise.wav /tmp/out.wav --effect gate --param threshold=-40

# Parametric EQ
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect eq --param low_gain=6

# Filter
cargo run -p sonido-cli -- process test_audio/noise.wav /tmp/out.wav --effect filter --param cutoff=2000

# Wah
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect wah --param sensitivity=80

# Tremolo
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect tremolo --param rate=5 --param depth=0.8

# Chorus
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect chorus --param rate=2 --param depth=0.5

# Flanger
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect flanger --param rate=0.5 --param depth=0.7

# Phaser
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect phaser --param rate=0.3 --param depth=0.8

# MultiVibrato
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect multivibrato --param depth=0.5

# Delay
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect delay --param time=300 --param feedback=0.4

# Tape
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect tape --param drive=12

# Reverb
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect reverb --param mix=0.5 --param room_size=0.8
```

**Verification**: Each output file should exist, be a valid WAV, and have audible effect applied.

#### Effect Chain Tests

Test chaining multiple effects:

```bash
# Simple chain
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav \
    --chain "preamp:gain=6|distortion:drive=12"

# Complex chain with multiple effects
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav \
    --chain "preamp:gain=6|distortion:drive=12|compressor:threshold=-20|delay:time=300|reverb:mix=0.3"

# Chain with whitespace (should be trimmed)
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav \
    --chain "distortion | delay:time=400 | reverb"
```

#### Preset Tests

```bash
# List available presets
cargo run -p sonido-cli -- presets list

# Process with a preset
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --preset crunch
```

---

### Synthesis Tests (sonido-synth)

Test the new synthesis commands.

#### Oscillator Waveforms

```bash
# Sine wave
cargo run -p sonido-cli -- generate osc /tmp/sine.wav --freq 440 --waveform sine --duration 1.0

# Triangle wave
cargo run -p sonido-cli -- generate osc /tmp/tri.wav --freq 440 --waveform triangle --duration 1.0

# Saw wave
cargo run -p sonido-cli -- generate osc /tmp/saw.wav --freq 220 --waveform saw --duration 1.0

# Square wave
cargo run -p sonido-cli -- generate osc /tmp/square.wav --freq 440 --waveform square --duration 1.0

# Noise
cargo run -p sonido-cli -- generate osc /tmp/noise.wav --waveform noise --duration 1.0
```

**Verification**: Each waveform should sound correct. Saw and square should be band-limited (no harsh aliasing).

#### Chord Generation

```bash
# C major triad (C4-E4-G4)
cargo run -p sonido-cli -- generate chord /tmp/cmajor.wav --notes "60,64,67" --duration 2.0

# A minor 7th
cargo run -p sonido-cli -- generate chord /tmp/am7.wav --notes "57,60,64,67" --waveform saw --duration 2.0

# Power chord (E2-B2)
cargo run -p sonido-cli -- generate chord /tmp/power.wav --notes "40,47" --filter-cutoff 1500 --duration 2.0

# Chord with custom envelope
cargo run -p sonido-cli -- generate chord /tmp/pad.wav --notes "60,64,67,72" --attack 200 --release 2000 --duration 4.0
```

**Verification**: Chords should have multiple voices, correct pitches, and proper envelope shaping.

#### ADSR Envelope Test

```bash
# Standard envelope
cargo run -p sonido-cli -- generate adsr /tmp/adsr.wav --attack 50 --decay 100 --sustain 0.7 --release 200

# Pluck-style (fast attack/decay, no sustain)
cargo run -p sonido-cli -- generate adsr /tmp/pluck.wav --attack 1 --decay 50 --sustain 0.0 --release 100

# Pad-style (slow attack)
cargo run -p sonido-cli -- generate adsr /tmp/pad.wav --attack 500 --decay 200 --sustain 0.8 --release 2000 --gate-duration 3.0
```

**Verification**: Envelope shape should be audible. Pluck should be percussive, pad should have slow attack.

---

### Analysis Tests

#### Standard Analysis Commands

```bash
# Spectrum analysis
cargo run -p sonido-cli -- analyze spectrum test_audio/sine_440.wav --peaks 5

# Distortion analysis (THD)
cargo run -p sonido-cli -- analyze distortion test_audio/sine_440.wav

# Dynamics analysis
cargo run -p sonido-cli -- analyze dynamics test_audio/noise.wav
```

#### Transfer Function / IR

```bash
# Generate sweep and process through an effect
cargo run -p sonido-cli -- generate sweep /tmp/sweep_dry.wav --duration 3.0
cargo run -p sonido-cli -- process /tmp/sweep_dry.wav /tmp/sweep_wet.wav --effect reverb --param mix=1.0

# Extract impulse response
cargo run -p sonido-cli -- analyze ir /tmp/sweep_dry.wav /tmp/sweep_wet.wav -o /tmp/ir.wav --rt60
```

#### CFC Analysis Commands

```bash
# Generate a test signal with low-frequency modulation (for PAC testing)
cargo run -p sonido-cli -- generate tone /tmp/carrier.wav --freq 50 --duration 5.0

# PAC analysis
cargo run -p sonido-cli -- analyze pac /tmp/carrier.wav --phase-low 4 --phase-high 8 --amp-low 30 --amp-high 100

# Bandpass extraction
cargo run -p sonido-cli -- analyze bandpass test_audio/noise.wav --low 100 --high 500 -o /tmp/bandpassed.wav

# Hilbert transform
cargo run -p sonido-cli -- analyze hilbert test_audio/sine_440.wav --amp-output /tmp/envelope.wav

# Comodulogram (takes longer)
cargo run -p sonido-cli -- analyze comodulogram test_audio/noise.wav \
    --phase-range 2-20 --amp-range 20-200 --phase-step 4 --amp-step 20 -o /tmp/comod.csv
```

#### IMD Analysis

```bash
# Generate two-tone test (manually mix or use stereo)
cargo run -p sonido-cli -- generate tone /tmp/tone1.wav --freq 1000 --duration 2.0 --amplitude 0.4
cargo run -p sonido-cli -- generate tone /tmp/tone2.wav --freq 1200 --duration 2.0 --amplitude 0.4

# Mix tones (using external tool or processing)
# Then analyze IMD
cargo run -p sonido-cli -- analyze imd /tmp/mixed_tones.wav --freq1 1000 --freq2 1200
```

#### CQT Analysis

```bash
# Basic CQT analysis
cargo run -p sonido-cli -- analyze cqt test_audio/sine_440.wav --peaks 5

# With chromagram
cargo run -p sonido-cli -- analyze cqt test_audio/sine_440.wav --chromagram

# Custom frequency range
cargo run -p sonido-cli -- analyze cqt test_audio/sine_440.wav --min-freq 100 --max-freq 2000 --bins-per-octave 24
```

---

### Real-time Tests

```bash
# Basic real-time effect
cargo run -p sonido-cli -- realtime --effect reverb --param mix=0.3

# Select specific devices
cargo run -p sonido-cli -- realtime --input 0 --output 0 --effect chorus

# Effect chain in real-time
cargo run -p sonido-cli -- realtime --chain "preamp:gain=6|distortion:drive=10|delay:time=250"

# Lower latency
cargo run -p sonido-cli -- realtime --effect delay --buffer-size 128
```

**Verification**: Audio should pass through with effect applied. Press Ctrl+C to stop.

---

## TUI Testing Protocol

Launch the TUI:

```bash
cargo run -p sonido-cli -- tui
```

### Navigation Tests

| Key | Expected Behavior |
|-----|-------------------|
| `Tab` / Arrow keys | Navigate between effects |
| `Enter` | Select effect for editing |
| `Up/Down` | Adjust selected parameter |
| `b` | Toggle bypass for selected effect |
| `q` / `Esc` | Exit TUI |
| `?` | Show help |

### Effect Selection

1. Use Tab/arrows to highlight each of the 15 effects
2. Verify the effect name is highlighted
3. Press Enter to select and verify parameter panel appears

### Parameter Adjustment

1. Select an effect (e.g., Distortion)
2. Use Up/Down arrows to change parameter values
3. Verify value changes are reflected in the display
4. Verify audio output changes in real-time

### Bypass Toggle

1. Select an effect
2. Press `b` to toggle bypass
3. Verify bypass indicator changes (LED or text)
4. Verify audio is unaffected when bypassed

### Preset Loading

```bash
# Launch with preset
cargo run -p sonido-cli -- tui --preset crunch
```

Verify preset parameters are loaded correctly.

### Expected TUI Layout

```
+--------------------------------------------------+
| SONIDO TUI           [Preset: crunch]            |
+--------------------------------------------------+
| [PRE][DIST][COMP][CHO][DLY][FLT][VIB][TAP][REV]  |
|  ^                                               |
+--------------------------------------------------+
| DISTORTION                                       |
|   Drive: [=====>    ] 15.0 dB                    |
|   Tone:  [======>   ] 4000 Hz                    |
|   Level: [===>      ] -6.0 dB                    |
+--------------------------------------------------+
| Tab: Navigate | Enter: Select | b: Bypass | q: Quit
+--------------------------------------------------+
```

---

## GUI Testing Protocol

Launch the GUI:

```bash
cargo run -p sonido-gui
```

### Initial State Verification

- [ ] Window opens without crash
- [ ] Audio status indicator shows (green = OK, red = error)
- [ ] Input/output meters respond to audio
- [ ] All 15 effect buttons visible in chain strip

### Effect Panel Checklist

For **each of the 15 effects**, verify:

| Effect | Select | Knobs Work | Bypass | Audio Processed |
|--------|--------|------------|--------|-----------------|
| Preamp | [ ] | [ ] | [ ] | [ ] |
| Distortion | [ ] | [ ] | [ ] | [ ] |
| Compressor | [ ] | [ ] | [ ] | [ ] |
| Gate | [ ] | [ ] | [ ] | [ ] |
| EQ | [ ] | [ ] | [ ] | [ ] |
| Filter | [ ] | [ ] | [ ] | [ ] |
| Wah | [ ] | [ ] | [ ] | [ ] |
| Tremolo | [ ] | [ ] | [ ] | [ ] |
| Chorus | [ ] | [ ] | [ ] | [ ] |
| Flanger | [ ] | [ ] | [ ] | [ ] |
| Phaser | [ ] | [ ] | [ ] | [ ] |
| MultiVibrato | [ ] | [ ] | [ ] | [ ] |
| Delay | [ ] | [ ] | [ ] | [ ] |
| Tape | [ ] | [ ] | [ ] | [ ] |
| Reverb | [ ] | [ ] | [ ] | [ ] |

### Testing Each Effect Panel

1. **Click** the effect button in the chain strip
2. **Verify** the parameter panel appears below
3. **Drag** each knob and verify value changes
4. **Double-click** a knob to reset to default
5. **Double-click** the effect button to toggle bypass
6. **Verify** bypass LED changes color (green = active, dim = bypassed)
7. **Listen** for audio changes with bypass on/off

### New Effect Panels (Priority Testing)

The following effect panels were recently added and need thorough testing:

#### Flanger Panel
- Rate knob (0.05-5 Hz)
- Depth knob (0-100%)
- Feedback knob (0-95%)
- Mix knob (0-100%)

#### Phaser Panel
- Rate knob (0.05-5 Hz)
- Depth knob (0-100%)
- Feedback knob (0-95%)
- Mix knob (0-100%)
- Stages selector (2/4/6/8/10/12)

#### Tremolo Panel
- Rate knob (0.5-20 Hz)
- Depth knob (0-100%)
- Wave selector (Sine/Triangle/Square/S&H)

#### Gate Panel
- Threshold knob (-80 to 0 dB)
- Attack knob (0.1-50 ms)
- Release knob (10-1000 ms)
- Hold knob (0-500 ms)

#### Wah Panel
- Frequency knob (200-2000 Hz)
- Resonance knob (1-10)
- Sensitivity knob (0-100%)
- Mode toggle (Auto/Manual)

#### EQ Panel
- Three bands (Low/Mid/High)
- Each band: Frequency, Gain, Q
- Verify frequency ranges don't overlap incorrectly

### Preset Tests

1. **Open** preset selector dropdown
2. **Select** a factory preset
3. **Verify** effect parameters update
4. **Modify** some parameters
5. **Verify** asterisk (*) appears indicating unsaved changes
6. **Click** Save button
7. **Enter** a preset name
8. **Save** and verify preset appears in list

### Audio I/O Tests

1. **Verify** input meter responds to microphone/input
2. **Verify** output meter shows processed signal level
3. **Adjust** input gain knob and verify level change
4. **Adjust** master volume and verify output change

### Latency and Performance

1. **Check** status bar for sample rate, buffer size, latency
2. **Monitor** CPU usage percentage
3. **Test** with smaller buffer for lower latency:
   ```bash
   cargo run -p sonido-gui -- --buffer-size 128
   ```
4. **Verify** no audio glitches during parameter changes

---

## Regression Testing

### Generate Golden Files

Run once to create reference audio files:

```bash
REGENERATE_GOLDEN=1 cargo test -p sonido-effects --test regression
```

This creates `crates/sonido-effects/tests/golden/*.wav` files.

### Run Regression Tests

```bash
cargo test -p sonido-effects --test regression
```

Tests compare current output against golden files using RMS difference threshold.

---

## Verification Summary

### Full Test Run

```bash
# All automated tests
cargo test --workspace

# Generate golden files if missing
REGENERATE_GOLDEN=1 cargo test -p sonido-effects --test regression

# All tests including regression
cargo test --workspace
```

### Quick Smoke Test

```bash
# CLI processes audio
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/test.wav --effect reverb && echo "CLI: OK"

# CLI generates synthesis
cargo run -p sonido-cli -- generate osc /tmp/osc.wav --waveform saw && echo "Synth: OK"

# CLI analysis runs
cargo run -p sonido-cli -- analyze spectrum test_audio/sine_440.wav && echo "Analysis: OK"

# List effects
cargo run -p sonido-cli -- effects && echo "Effects: OK"

# TUI launches (Ctrl+C to exit)
echo "Testing TUI... (press q to exit)"
cargo run -p sonido-cli -- tui

# GUI launches (close window to exit)
echo "Testing GUI... (close window to exit)"
cargo run -p sonido-gui
```

---

## Known Issues and Workarounds

### Audio Device Not Found
- Run `sonido devices list` to verify device names
- Try specifying device by index: `--input 0 --output 0`
- Check system audio settings

### High Latency / Glitches
- Increase buffer size: `--buffer-size 512` or `--buffer-size 1024`
- Close other audio applications
- Check CPU usage

### GUI Won't Launch
```bash
# Check for errors with debug output
RUST_LOG=debug cargo run -p sonido-gui
```

### Regression Test Fails
- If golden files are missing: `REGENERATE_GOLDEN=1 cargo test ...`
- If audio processing changed intentionally, regenerate golden files
- Check RMS threshold in regression test code

---

## See Also

- [CLI Guide](CLI_GUIDE.md) - Full CLI reference
- [GUI Documentation](GUI.md) - GUI user guide
- [Testing Guide](TESTING.md) - Automated testing
- [Benchmarks](BENCHMARKS.md) - Performance testing
