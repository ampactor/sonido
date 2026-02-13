# Sonido GUI

Sonido GUI is a professional real-time DSP effect processor built on the Sonido framework using [egui](https://github.com/emilk/egui).

## Installation

### From Source

```bash
# Build and run
cargo run -p sonido-gui

# Or install globally
cargo install --path crates/sonido-gui
sonido-gui
```

## Command-Line Options

```
sonido-gui [OPTIONS]

OPTIONS:
    --input <NAME>         Input audio device name (uses default if not specified)
    --output <NAME>        Output audio device name (uses default if not specified)
    --sample-rate <N>      Sample rate in Hz (default: 48000)
    --buffer-size <N>      Buffer size in samples (default: 512)
    -h, --help             Print help information
    -V, --version          Print version information
```

### Examples

```bash
# Use default devices
sonido-gui

# Lower latency (smaller buffer)
sonido-gui --buffer-size 128

# Specify devices
sonido-gui --input "USB Audio" --output "Built-in Output"

# Higher sample rate
sonido-gui --sample-rate 96000
```

## User Interface

### Window Layout

```
+------------------------------------------------------------------+
| SONIDO         [Preset Selector v] [Save]        [Audio Status]  |
+------------------------------------------------------------------+
|                                                                  |
| INPUT  |  EFFECT CHAIN                              |  OUTPUT    |
| [Meter]|  [PRE]->[DIST]->[COMP]->[CHO]->[DLY]->... |  [Meter]   |
| [Gain] |  [FLT]->[VIB]->[TAPE]->[REV]              |  [Master]  |
|        |                                            |            |
|        +--------------------------------------------+            |
|        | SELECTED EFFECT PANEL                      |            |
|        | (Parameters for currently selected effect) |            |
|        +--------------------------------------------+            |
+------------------------------------------------------------------+
| 48000 Hz | 512 samples | 10.7 ms | CPU: 2.3%                     |
+------------------------------------------------------------------+
```

### Header Bar

- **SONIDO**: Application title
- **Preset Selector**: Drop-down to choose presets (asterisk * indicates unsaved changes)
- **Save**: Save current settings as a preset
- **Audio Status**: Green dot = audio running, red dot = audio error

### Input/Output Sections

- **Input Meter**: Real-time peak and RMS level display
- **Input Gain**: -20 to +20 dB gain control
- **Output Meter**: Real-time peak and RMS level display
- **Master Volume**: -40 to +6 dB master output control

### Effect Chain Strip

The chain strip shows all 15 effects in processing order:

| Effect | Short Name | Description |
|--------|------------|-------------|
| Preamp | PRE | Clean gain stage |
| Distortion | DIST | Waveshaping distortion |
| Compressor | COMP | Dynamics compressor |
| Gate | GATE | Noise gate |
| Parametric EQ | EQ | 3-band parametric equalizer |
| Filter | FLT | Resonant lowpass filter |
| Wah | WAH | Auto-wah / manual wah |
| Tremolo | TREM | Amplitude modulation |
| Chorus | CHO | Modulated delay chorus |
| Flanger | FLG | Classic flanger effect |
| Phaser | PHAS | Multi-stage phaser |
| MultiVibrato | VIB | 6-unit tape wow/flutter |
| Delay | DLY | Tape-style feedback delay |
| Tape | TAPE | Tape saturation |
| Reverb | REV | Freeverb-style reverb |

**Interactions:**
- **Click**: Select effect to show its parameter panel below
- **Double-click**: Toggle effect bypass (LED indicator: green = active, dim = bypassed)
- **Drag**: Reorder effects in the chain (drag left/right)

### Effect Panels

When an effect is selected, its parameter panel appears below the chain. Each panel includes:

- Effect name header
- Rotary knobs for continuous parameters
- Toggle buttons for discrete options
- Value readouts below knobs

**Common Controls:**
- **Knob drag**: Vertical drag to adjust value
- **Knob double-click**: Reset to default value

### Status Bar

- **Sample Rate**: Current sample rate (e.g., 48000 Hz)
- **Buffer Size**: Audio buffer size in samples
- **Latency**: Round-trip latency in milliseconds
- **CPU**: Audio thread CPU usage percentage

## Effects Reference

### Preamp
- **Gain**: -20 to +20 dB
- **Output**: -20 to +20 dB output level
- **Headroom**: 6 to 40 dB clipping ceiling

### Distortion
- **Drive**: 0 to 40 dB (default 12)
- **Tone**: 500 to 10000 Hz lowpass filter (default 4000)
- **Level**: -20 to 0 dB output level (default -6)
- **Waveshape**: SoftClip, HardClip, Foldback, Asymmetric

### Compressor
- **Threshold**: -60 to 0 dB (default -18)
- **Ratio**: 1:1 to 20:1
- **Attack**: 0.1 to 100 ms
- **Release**: 10 to 1000 ms
- **Makeup**: 0 to 24 dB
- **Knee**: 0 to 12 dB knee width

### Chorus
- **Rate**: 0.1 to 10 Hz LFO rate
- **Depth**: 0 to 1 modulation depth
- **Mix**: 0 to 1 wet/dry mix

### Delay
- **Time**: 1 to 2000 ms delay time (default 300)
- **Feedback**: 0 to 0.95 feedback amount (default 0.4)
- **Mix**: 0 to 1 wet/dry mix
- **Ping-Pong**: On/Off stereo ping-pong mode

### Filter (Lowpass)
- **Cutoff**: 20 to 20000 Hz
- **Resonance**: 0.1 to 10 Q factor

### MultiVibrato
- **Depth**: 0 to 1 overall modulation depth
- **Mix**: 0 to 100% wet/dry mix

### Tape Saturation
- **Drive**: 0 to 24 dB (default 6)
- **Saturation**: 0 to 1
- **Output**: -12 to +12 dB output level
- **HF Rolloff**: 1000 to 20000 Hz high-frequency rolloff
- **Bias**: -0.2 to 0.2 tape bias offset

### Reverb
- **Room Size**: 0 to 1 (default 0.5)
- **Decay**: 0 to 1
- **Damping**: 0 to 1 (0 = bright, 1 = dark)
- **Pre-delay**: 0 to 100 ms
- **Mix**: 0 to 1 wet/dry mix
- **Stereo Width**: 0 to 100% stereo decorrelation
- **Type**: Room, Hall

### Tremolo
- **Rate**: 0.5 to 20 Hz LFO rate
- **Depth**: 0 to 100% modulation depth
- **Wave**: Sine, Triangle, Square, Sample & Hold

### Flanger
- **Rate**: 0.05 to 5 Hz LFO rate
- **Depth**: 0 to 100% modulation depth
- **Feedback**: 0 to 95% feedback amount
- **Mix**: 0 to 100% wet/dry mix

### Phaser
- **Rate**: 0.05 to 5 Hz LFO rate
- **Depth**: 0 to 100% modulation depth
- **Feedback**: 0 to 95% feedback/resonance
- **Mix**: 0 to 100% wet/dry mix
- **Stages**: 2, 4, 6, 8, 10, or 12 allpass stages

### Gate
- **Threshold**: -80 to 0 dB open threshold
- **Attack**: 0.1 to 50 ms attack time
- **Release**: 10 to 1000 ms release time
- **Hold**: 0 to 500 ms hold time

### Wah
- **Frequency**: 200 to 2000 Hz center frequency
- **Resonance**: 1 to 10 filter Q
- **Sensitivity**: 0 to 100% (auto-wah mode)
- **Mode**: Auto (envelope follower), Manual

### Parametric EQ
Three independent bands with identical controls:

**Low Band**
- **Frequency**: 20 to 500 Hz
- **Gain**: -12 to +12 dB
- **Q**: 0.5 to 5.0

**Mid Band**
- **Frequency**: 200 to 5000 Hz
- **Gain**: -12 to +12 dB
- **Q**: 0.5 to 5.0

**High Band**
- **Frequency**: 1000 to 15000 Hz
- **Gain**: -12 to +12 dB
- **Q**: 0.5 to 5.0

## Preset Management

### Factory Presets

Built-in presets are available immediately:

- **Init**: Clean signal, all effects bypassed
- **Crunch**: Moderate distortion
- **High Gain**: Heavy distortion with compression
- **Ambient**: Delay, reverb, and chorus combination
- **Tape Warmth**: Subtle tape saturation

### Saving Presets

1. Adjust effect parameters to your liking
2. Click the **Save** button in the header
3. Enter a preset name and category
4. Click **Save** to store the preset

### Preset Storage Location

User presets are saved as JSON files in:

| Platform | Directory |
|----------|-----------|
| Linux | `~/.config/sonido/presets/` |
| macOS | `~/Library/Application Support/sonido/presets/` |
| Windows | `%APPDATA%\sonido\presets\` |

### Preset File Format

Presets are stored as JSON files containing all effect parameters, bypass states, and effect order:

```json
{
  "name": "My Preset",
  "category": "User",
  "input_gain": 0.0,
  "master_volume": -3.0,
  "distortion_bypass": false,
  "dist_drive": 18.0,
  ...
}
```

## Real-Time Parameter Changes

The GUI uses a lock-free architecture for audio-safe parameter updates:

- **AtomicParam**: Float parameters use atomic bit-cast operations
- **AtomicBool**: Bypass states use atomic booleans
- **No locks**: GUI and audio threads never block each other

Parameter changes are applied smoothly using the `SmoothedParam` system in the DSP code, avoiding clicks and pops.

## Audio Thread Architecture

```
[Input Device] -> [Input Gain] -> [Effect Chain] -> [Master Volume] -> [Output Device]
                                        |
                                        v
                              [Metering Data] -> [GUI Thread]
```

- Audio callback runs in a dedicated thread
- Metering data is sent via bounded channel (drops old data if full)
- Sample-by-sample processing through the effect chain

## Troubleshooting

### No Audio

1. Check the audio status indicator (should be green)
2. Verify input/output devices are connected
3. Run `sonido devices` in CLI to list available devices
4. Try specifying devices explicitly: `sonido-gui --input "Device Name"`

### Audio Glitches/Crackling

1. Increase buffer size: `sonido-gui --buffer-size 512` or `--buffer-size 1024`
2. Close other audio applications
3. Check CPU usage in the status bar

### High Latency

1. Decrease buffer size: `sonido-gui --buffer-size 128` or `--buffer-size 256`
2. Note: smaller buffers require more CPU and may cause glitches on slower systems

### Device Selection Issues

List available devices first:
```bash
sonido devices
```

Then specify the exact device name:
```bash
sonido-gui --input "USB Audio Interface" --output "USB Audio Interface"
```

### Application Won't Start

Check logs for errors:
```bash
RUST_LOG=debug sonido-gui
```

Common issues:
- Missing audio drivers (install ALSA on Linux, check CoreAudio on macOS)
- Permission issues with audio devices
- Conflicting sample rates (try `--sample-rate 44100`)

## Keyboard Shortcuts

Currently, the GUI is primarily mouse-driven. Keyboard shortcuts may be added in future versions.

## See Also

- [Getting Started](GETTING_STARTED.md) - Quick start guide
- [CLI Guide](CLI_GUIDE.md) - Command-line interface documentation
- [Effects Reference](EFFECTS_REFERENCE.md) - Detailed effect documentation
- [Architecture](ARCHITECTURE.md) - System architecture overview
