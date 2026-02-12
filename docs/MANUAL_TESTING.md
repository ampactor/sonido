# Manual Testing Protocol

Structured walkthrough for testing every aspect of the Sonido CLI and GUI.
Work through each phase in order. Every step has a checkbox and pass/fail criteria.

---

## Phase 0: Setup

### 0.1 Build

```bash
cargo build
```

- [ ] Workspace compiles without errors

### 0.2 Generate Test Audio

```bash
mkdir -p test_audio
cargo run -p sonido-cli -- generate tone test_audio/sine_440.wav --freq 440 --duration 2.0
cargo run -p sonido-cli -- generate sweep test_audio/sweep.wav --duration 3.0
cargo run -p sonido-cli -- generate noise test_audio/noise.wav --duration 2.0 --amplitude 0.3
```

- [X] `test_audio/sine_440.wav` exists (96000 samples at 48kHz)
- [X] `test_audio/sweep.wav` exists
- [X] `test_audio/noise.wav` exists

### 0.3 Verify Audio Devices

```bash
cargo run -p sonido-cli -- devices list
cargo run -p sonido-cli -- devices info
```

- [X] At least one input and one output device listed
- [X] Default device info displayed

---

## Phase 1: CLI — Effect Discovery

### 1.1 Help and Version

```bash
cargo run -p sonido-cli -- --help
cargo run -p sonido-cli -- --version
```

- [X] Help shows all 9 commands: process, realtime, generate, analyze, compare, devices, effects, presets, help

### 1.2 List Effects

```bash
cargo run -p sonido-cli -- effects
```

- [X] Lists 15 effects with descriptions

### 1.3 Effect Detail

```bash
cargo run -p sonido-cli -- effects distortion
cargo run -p sonido-cli -- effects reverb
```

- [X] Shows parameters with names, ranges, defaults, and units

### 1.4 Effect Examples

```bash
cargo run -p sonido-cli -- effects --examples
```

- [X] Prints example CLI commands

---

## Phase 2: CLI — Effect Processing

Run each effect on a test file. Pass = exits with "Done!", output WAV exists, stats printed.

### 2.1 All 15 Individual Effects

| # | Command | Pass |
|---|---------|------|
| 1 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect preamp --param gain=6` | [ ] |
| 2 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect distortion --param drive=15` | [ ] |
| 3 | `cargo run -p sonido-cli -- process test_audio/noise.wav --effect compressor --param threshold=-20` | [ ] |
| 4 | `cargo run -p sonido-cli -- process test_audio/noise.wav --effect gate --param threshold=-40` | [ ] |
| 5 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect eq --param low_gain=6` | [ ] |
| 6 | `cargo run -p sonido-cli -- process test_audio/noise.wav --effect filter --param cutoff=2000` | [ ] |
| 7 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect wah --param sensitivity=0.8` | [ ] |
| 8 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect tremolo --param rate=5 --param depth=0.8` | [ ] |
| 9 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect chorus --param rate=2 --param depth=0.5` | [ ] |
| 10 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect flanger --param rate=0.5 --param depth=0.7` | [ ] |
| 11 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect phaser --param rate=0.3 --param depth=0.8` | [ ] |
| 12 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect multivibrato --param depth=0.5` | [ ] |
| 13 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect delay --param time=300 --param feedback=0.4` | [ ] |
| 14 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect tape --param drive=12` | [ ] |
| 15 | `cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect reverb --param mix=0.5 --param room_size=0.8` | [ ] |

### 2.2 Effect Aliases

Verify aliases resolve to the same effect:

```bash
cargo run -p sonido-cli -- process test_audio/noise.wav --effect lowpass --param cutoff=2000
cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect vibrato --param depth=0.5
cargo run -p sonido-cli -- process test_audio/noise.wav --effect noisegate --param threshold=-40
cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect autowah --param sensitivity=0.8
cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect parametriceq --param low_gain=6
```

- [ ] All 5 aliases produce output without error

### 2.3 Effect Chains

```bash
# Simple 2-effect chain
cargo run -p sonido-cli -- process test_audio/sine_440.wav \
    --chain "preamp:gain=6|distortion:drive=12"

# Complex 5-effect chain
cargo run -p sonido-cli -- process test_audio/sine_440.wav \
    --chain "preamp:gain=6|distortion:drive=12|compressor:threshold=-20|delay:time=300|reverb:mix=0.3"

# Chain with whitespace (should be trimmed)
cargo run -p sonido-cli -- process test_audio/sine_440.wav \
    --chain "distortion | delay:time=400 | reverb"
```

- [ ] 2-effect chain: output shows "2 effect(s)"
- [ ] 5-effect chain: output shows "5 effect(s)"
- [ ] Whitespace chain: processes without error

### 2.4 Process Options

```bash
# Force mono output (default is always stereo)
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect reverb --mono

# Custom block size
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect reverb --block-size 128

# Custom bit depth
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/out.wav --effect reverb --bit-depth 16
```

- [ ] Mono output produces 1-channel WAV (--mono forces mono; without it, output is always 2-channel)
- [ ] Block size 128 processes without error
- [ ] 16-bit output produces valid WAV

---

## Phase 3: CLI — Presets

### 3.1 List Presets

```bash
cargo run -p sonido-cli -- presets list
```

- [ ] Shows factory presets and user presets (if any)

### 3.2 Show Preset Detail

```bash
cargo run -p sonido-cli -- presets show crunch
```

- [ ] Displays preset name, effects, and parameter values

### 3.3 Process with Preset

```bash
cargo run -p sonido-cli -- process test_audio/sine_440.wav --preset crunch
```

- [ ] Processes with preset-defined effect chain

### 3.4 Preset Paths

```bash
cargo run -p sonido-cli -- presets paths
```

- [ ] Shows factory and user preset directories

### 3.5 Save and Delete User Preset

```bash
cargo run -p sonido-cli -- presets save test_preset --chain "distortion:drive=10|delay:time=200"
cargo run -p sonido-cli -- presets show test_preset
cargo run -p sonido-cli -- presets delete test_preset --force
```

- [ ] Save succeeds
- [ ] Show displays the saved chain
- [ ] Delete succeeds

---

## Phase 4: CLI — Signal Generation

### 4.1 Basic Generators

```bash
cargo run -p sonido-cli -- generate tone /tmp/tone.wav --freq 1000 --duration 1.0
cargo run -p sonido-cli -- generate sweep /tmp/sweep.wav --duration 2.0
cargo run -p sonido-cli -- generate noise /tmp/noise.wav --duration 1.0 --amplitude 0.5
cargo run -p sonido-cli -- generate impulse /tmp/impulse.wav
cargo run -p sonido-cli -- generate silence /tmp/silence.wav --duration 1.0
```

- [ ] tone: 1kHz sine, 48000 samples
- [ ] sweep: chirp signal
- [ ] noise: white noise at -6dB
- [ ] impulse: single sample
- [ ] silence: all zeros

### 4.2 PolyBLEP Oscillator Waveforms

```bash
cargo run -p sonido-cli -- generate osc /tmp/osc_sine.wav --freq 440 --waveform sine --duration 1.0
cargo run -p sonido-cli -- generate osc /tmp/osc_tri.wav --freq 440 --waveform triangle --duration 1.0
cargo run -p sonido-cli -- generate osc /tmp/osc_saw.wav --freq 220 --waveform saw --duration 1.0
cargo run -p sonido-cli -- generate osc /tmp/osc_square.wav --freq 440 --waveform square --duration 1.0
cargo run -p sonido-cli -- generate osc /tmp/osc_noise.wav --waveform noise --duration 1.0
```

- [ ] All 5 waveforms generate without error
- [ ] Saw and square should sound band-limited (no harsh aliasing at 220/440Hz)

### 4.3 Chord Generation (Polyphonic Synth)

```bash
# C major triad
cargo run -p sonido-cli -- generate chord /tmp/cmajor.wav --notes "60,64,67" --duration 2.0

# A minor 7th with saw wave
cargo run -p sonido-cli -- generate chord /tmp/am7.wav --notes "57,60,64,67" --waveform saw --duration 2.0

# With filter and custom envelope
cargo run -p sonido-cli -- generate chord /tmp/pad.wav --notes "60,64,67,72" --attack 200 --release 2000 --duration 4.0
```

- [ ] C major: 3 voices at 261.6, 329.6, 392.0 Hz
- [ ] Am7: 4 voices with saw timbre
- [ ] Pad: slow attack audible, long release tail

### 4.4 ADSR Envelope Tests

```bash
# Standard envelope
cargo run -p sonido-cli -- generate adsr /tmp/adsr.wav --attack 50 --decay 100 --sustain 0.7 --release 200

# Pluck (fast attack/decay, zero sustain)
cargo run -p sonido-cli -- generate adsr /tmp/pluck.wav --attack 1 --decay 50 --sustain 0.0 --release 100

# Pad (slow attack)
cargo run -p sonido-cli -- generate adsr /tmp/pad_env.wav --attack 500 --decay 200 --sustain 0.8 --release 2000 --gate-duration 3.0
```

- [ ] Standard: audible attack-decay-sustain-release shape
- [ ] Pluck: percussive, short
- [ ] Pad: gradual fade-in

---

## Phase 5: CLI — Analysis

### 5.1 Spectrum Analysis

```bash
cargo run -p sonido-cli -- analyze spectrum test_audio/sine_440.wav --peaks 5
```

- [ ] Shows peak near 440Hz as highest amplitude
- [ ] Lists 5 frequency peaks with dB levels

### 5.2 Distortion Analysis (THD)

```bash
cargo run -p sonido-cli -- analyze distortion test_audio/sine_440.wav
```

- [ ] Reports THD (should be very low for pure sine)

### 5.3 Dynamics Analysis

```bash
cargo run -p sonido-cli -- analyze dynamics test_audio/noise.wav
```

- [ ] Reports RMS level, peak level, crest factor, dynamic range

### 5.4 Impulse Response Extraction

```bash
cargo run -p sonido-cli -- generate sweep /tmp/sweep_dry.wav --duration 3.0
cargo run -p sonido-cli -- process /tmp/sweep_dry.wav /tmp/sweep_wet.wav --effect reverb --param mix=1
cargo run -p sonido-cli -- analyze ir /tmp/sweep_dry.wav /tmp/sweep_wet.wav -o /tmp/ir.wav --rt60
```

- [ ] Sweep generated
- [ ] Sweep processed through reverb
- [ ] IR extracted, RT60 reported

### 5.5 CQT Analysis

```bash
cargo run -p sonido-cli -- analyze cqt test_audio/sine_440.wav --peaks 5
cargo run -p sonido-cli -- analyze cqt test_audio/sine_440.wav --chromagram
```

- [ ] Peaks include A4 (440Hz)
- [ ] Chromagram shows energy at note A

### 5.6 Bandpass and Hilbert

```bash
cargo run -p sonido-cli -- analyze bandpass test_audio/noise.wav --low 100 --high 500 -o /tmp/bandpassed.wav
cargo run -p sonido-cli -- analyze hilbert test_audio/sine_440.wav --amp-output /tmp/envelope.wav
```

- [ ] Bandpassed output contains only 100-500Hz content
- [ ] Hilbert envelope output exists

### 5.7 PAC and Comodulogram

```bash
cargo run -p sonido-cli -- generate tone /tmp/carrier.wav --freq 50 --duration 5.0
cargo run -p sonido-cli -- analyze pac /tmp/carrier.wav --phase-low 4 --phase-high 8 --amp-low 30 --amp-high 100
cargo run -p sonido-cli -- analyze comodulogram test_audio/noise.wav \
    --phase-range 2-20 --amp-range 20-200 --phase-step 4 --amp-step 20 -o /tmp/comod.csv
```

- [ ] PAC analysis completes with modulation index
- [ ] Comodulogram CSV file generated

### 5.8 A/B Comparison

```bash
cargo run -p sonido-cli -- process test_audio/sine_440.wav /tmp/dist_out.wav --effect distortion --param drive=15
cargo run -p sonido-cli -- compare test_audio/sine_440.wav /tmp/dist_out.wav --detailed
```

- [ ] Reports spectral differences between dry and wet signals

---

## Phase 6: CLI — Real-time Processing

Requires audio devices. Press Ctrl+C to stop each test.

### 6.1 Single Effect

```bash
cargo run -p sonido-cli -- realtime --effect reverb --param mix=0.3
```

- [ ] Audio passes through with reverb applied
- [ ] Ctrl+C exits cleanly

### 6.2 Effect Chain

```bash
cargo run -p sonido-cli -- realtime --chain "preamp:gain=6|distortion:drive=10|delay:time=250"
```

- [ ] 3-effect chain processes live audio

### 6.3 Device Selection

```bash
cargo run -p sonido-cli -- realtime --input 0 --output 0 --effect chorus
```

- [ ] Runs on specified devices

### 6.4 Buffer Size

```bash
cargo run -p sonido-cli -- realtime --effect delay --buffer-size 128
cargo run -p sonido-cli -- realtime --effect delay --buffer-size 1024
```

- [ ] 128: lower latency, no glitches
- [ ] 1024: higher latency, stable

### 6.5 Mono Mode

```bash
cargo run -p sonido-cli -- realtime --effect reverb --mono
```

- [ ] Processes in mono without error

---

## Phase 7: GUI

### 7.1 Launch

```bash
cargo run -p sonido-gui
```

- [ ] Window opens without crash
- [ ] No errors in terminal output

### 7.2 Initial State

- [ ] Audio status indicator visible (green = OK, red = error)
- [ ] Input/output meters present
- [ ] All 15 effect buttons visible in chain strip

### 7.3 Effect Panel Walkthrough

Click each effect button and test its panel:

| Effect | Selects | Panel Renders | Knobs Respond | Bypass Toggles | Pass |
|--------|---------|--------------|---------------|----------------|------|
| Preamp | [ ] | [ ] | [ ] | [ ] | [ ] |
| Distortion | [ ] | [ ] | [ ] | [ ] | [ ] |
| Compressor | [ ] | [ ] | [ ] | [ ] | [ ] |
| Gate | [ ] | [ ] | [ ] | [ ] | [ ] |
| EQ | [ ] | [ ] | [ ] | [ ] | [ ] |
| Filter | [ ] | [ ] | [ ] | [ ] | [ ] |
| Wah | [ ] | [ ] | [ ] | [ ] | [ ] |
| Tremolo | [ ] | [ ] | [ ] | [ ] | [ ] |
| Chorus | [ ] | [ ] | [ ] | [ ] | [ ] |
| Flanger | [ ] | [ ] | [ ] | [ ] | [ ] |
| Phaser | [ ] | [ ] | [ ] | [ ] | [ ] |
| MultiVibrato | [ ] | [ ] | [ ] | [ ] | [ ] |
| Delay | [ ] | [ ] | [ ] | [ ] | [ ] |
| Tape | [ ] | [ ] | [ ] | [ ] | [ ] |
| Reverb | [ ] | [ ] | [ ] | [ ] | [ ] |

### 7.4 Knob Interaction

For any effect panel:

- [ ] Drag knob: value changes smoothly
- [ ] Double-click knob: resets to default value
- [ ] Value display updates in real-time

### 7.5 Bypass

- [ ] Double-click effect button: toggles bypass
- [ ] Bypass LED changes color (green = active, dim = bypassed)
- [ ] Audio is dry when bypassed

### 7.6 Effect-Specific Panel Verification

#### Compressor
- [ ] Gain reduction meter displays (should now work — was a TODO, now fixed)
- [ ] Threshold, ratio, attack, release, makeup knobs present

#### Phaser
- [ ] Stages selector works (2/4/6/8/10/12)

#### Tremolo
- [ ] Wave selector works (Sine/Triangle/Square/S&H)

#### Wah
- [ ] Mode toggle works (Auto/Manual)

#### EQ
- [ ] Three bands (Low/Mid/High) each with Frequency, Gain, Q

### 7.7 Metering

- [ ] Input meter responds to audio input
- [ ] Output meter shows processed signal level
- [ ] Meter ballistics look smooth (not jumpy)

### 7.8 Preset Management

- [ ] Open preset selector
- [ ] Select a factory preset — parameters update
- [ ] Modify a parameter — asterisk (*) appears for unsaved changes
- [ ] Save preset — enter name, confirm save
- [ ] Saved preset appears in list

### 7.9 Performance

- [ ] Status bar shows sample rate, buffer size, latency
- [ ] No audio glitches during parameter changes
- [ ] CPU usage reasonable (check status bar)

Test lower buffer:
```bash
cargo run -p sonido-gui -- --buffer-size 128
```

- [ ] Launches and runs at lower latency

---

## Phase 8: Regression Tests

### 8.1 Golden File Tests

```bash
cargo test -p sonido-effects --test regression
```

- [ ] All 16 regression tests pass

### 8.2 Full Test Suite

```bash
cargo test --workspace
```

- [ ] All tests pass, zero failures

### 8.3 no_std Compatibility

```bash
cargo test --no-default-features -p sonido-core
cargo test --no-default-features -p sonido-effects
cargo test --no-default-features -p sonido-synth
cargo test --no-default-features -p sonido-registry
cargo test --no-default-features -p sonido-platform
```

- [ ] All 5 no_std crates pass

### 8.4 Clippy and Formatting

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] Zero clippy warnings
- [ ] Formatting clean

### 8.5 Documentation Build

```bash
cargo doc --workspace --no-deps
cargo test --workspace --doc
```

- [ ] Zero doc warnings
- [ ] All doc tests pass

---

## Quick Smoke Test

Minimal pass/fail for when you just need a fast sanity check:

```bash
cargo run -p sonido-cli -- generate tone test_audio/sine_440.wav --freq 440 --duration 2.0 && echo "PASS: generate"
cargo run -p sonido-cli -- process test_audio/sine_440.wav --effect reverb --param mix=0.5 && echo "PASS: process"
cargo run -p sonido-cli -- process test_audio/sine_440.wav --chain "distortion:drive=10|delay:time=300|reverb:mix=0.3" && echo "PASS: chain"
cargo run -p sonido-cli -- generate osc /tmp/smoke_osc.wav --waveform saw --freq 220 && echo "PASS: oscillator"
cargo run -p sonido-cli -- generate chord /tmp/smoke_chord.wav --notes "60,64,67" && echo "PASS: chord"
cargo run -p sonido-cli -- analyze spectrum test_audio/sine_440.wav --peaks 3 && echo "PASS: analysis"
cargo run -p sonido-cli -- effects && echo "PASS: effects list"
cargo test -p sonido-effects --test regression && echo "PASS: regression"
cargo test --workspace && echo "PASS: all tests"
```

- [ ] All 9 lines print PASS

---

## Troubleshooting

### Audio Device Not Found
```bash
cargo run -p sonido-cli -- devices list
```
Try specifying device by index: `--input 0 --output 0`

### High Latency / Glitches
Increase buffer size: `--buffer-size 512` or `--buffer-size 1024`. Close other audio applications.

### GUI Won't Launch
```bash
RUST_LOG=debug cargo run -p sonido-gui
```
Check for missing display server, GPU driver issues, or Wayland/X11 compatibility.

### Regression Test Fails
If golden files are missing: `REGENERATE_GOLDEN=1 cargo test -p sonido-effects --test regression`
If audio processing changed intentionally, regenerate golden files and verify the diff is expected.

---

## See Also

- [CLI Guide](CLI_GUIDE.md) — Full command reference
- [Effects Reference](EFFECTS_REFERENCE.md) — All 15 effects with parameters and DSP theory
- [GUI Documentation](GUI.md) — GUI user guide
- [Testing Guide](TESTING.md) — Automated testing patterns
- [DSP Fundamentals](DSP_FUNDAMENTALS.md) — Theory behind the implementations
