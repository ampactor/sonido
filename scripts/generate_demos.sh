#!/usr/bin/env bash
# generate_demos.sh -- Generate demo WAV files showcasing Sonido's DSP capabilities.
#
# Usage: ./scripts/generate_demos.sh
#
# Requires: cargo build -p sonido-cli --release

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SONIDO="$PROJECT_DIR/target/release/sonido"
DEMOS="$PROJECT_DIR/demos"

# Build if needed
if [ ! -f "$SONIDO" ]; then
    echo "Building sonido-cli (release)..."
    cargo build -p sonido-cli --release --manifest-path "$PROJECT_DIR/Cargo.toml"
fi

mkdir -p "$DEMOS"

echo "=== Generating source signals ==="

# 1. Sine tone at 440 Hz (2 seconds)
echo "  [1/5] Sine tone 440 Hz..."
"$SONIDO" generate tone "$DEMOS/src_sine_440.wav" \
    --freq 440.0 --duration 2.0 --amplitude 0.8

# 2. Saw chord pad (C major: MIDI 60,64,67)
echo "  [2/5] Saw chord pad (C major)..."
"$SONIDO" generate chord "$DEMOS/src_saw_chord.wav" \
    --notes "60,64,67" --waveform saw --duration 3.0 --amplitude 0.5 \
    --filter-cutoff 3000.0 --attack 50.0 --release 800.0

# 3. Short percussion ADSR envelope
echo "  [3/5] Percussion ADSR..."
"$SONIDO" generate adsr "$DEMOS/src_perc_adsr.wav" \
    --attack 5.0 --decay 80.0 --sustain 0.1 --release 150.0 \
    --freq 220.0 --gate-duration 0.3 --amplitude 0.9

# 4. Frequency sweep 20-20kHz
echo "  [4/5] Frequency sweep..."
"$SONIDO" generate sweep "$DEMOS/src_sweep.wav" \
    --start 20.0 --end 20000.0 --duration 3.0 --amplitude 0.7

# 5. White noise
echo "  [5/5] White noise..."
"$SONIDO" generate noise "$DEMOS/src_noise.wav" \
    --duration 2.0 --amplitude 0.5

echo ""
echo "=== Processing through effects ==="

# Distortion -- soft clip
echo "  [1/18] Distortion (soft clip)..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_distortion_soft.wav" \
    --effect distortion --param drive=12 --param waveshape=softclip --param tone=4000 --param level=-6

# Distortion -- hard clip
echo "  [2/18] Distortion (hard clip)..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_distortion_hard.wav" \
    --effect distortion --param drive=20 --param waveshape=hardclip --param tone=3000 --param level=-8

# Compressor
echo "  [3/18] Compressor..."
"$SONIDO" process "$DEMOS/src_perc_adsr.wav" "$DEMOS/fx_compressor.wav" \
    --effect compressor --param threshold=-18 --param ratio=4 --param attack=10 --param release=100 --param makeup=6

# Noise Gate
echo "  [4/18] Noise Gate..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_gate.wav" \
    --effect gate --param threshold=-20 --param attack=1 --param release=50 --param hold=20

# Chorus
echo "  [5/18] Chorus..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_chorus.wav" \
    --effect chorus --param rate=1.5 --param depth=0.6 --param mix=0.5

# Flanger
echo "  [6/18] Flanger..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_flanger.wav" \
    --effect flanger --param rate=0.4 --param depth=0.7 --param feedback=0.6 --param mix=0.5

# Phaser
echo "  [7/18] Phaser..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_phaser.wav" \
    --effect phaser --param rate=0.3 --param depth=0.8 --param stages=6 --param feedback=0.7 --param mix=0.5

# Tremolo
echo "  [8/18] Tremolo..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_tremolo.wav" \
    --effect tremolo --param rate=6 --param depth=0.8 --param waveform=sine

# Multi Vibrato
echo "  [9/18] Multi Vibrato..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_multivibrato.wav" \
    --effect multivibrato --param depth=0.6

# Low Pass Filter
echo "  [10/18] Low Pass Filter..."
"$SONIDO" process "$DEMOS/src_sweep.wav" "$DEMOS/fx_filter_lowpass.wav" \
    --effect filter --param cutoff=2000 --param resonance=4

# Auto-Wah
echo "  [11/18] Auto-Wah..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_wah.wav" \
    --effect wah --param frequency=800 --param resonance=5 --param sensitivity=0.7 --param mode=auto

# Parametric EQ
echo "  [12/18] Parametric EQ..."
"$SONIDO" process "$DEMOS/src_sweep.wav" "$DEMOS/fx_parametric_eq.wav" \
    --effect eq --param mid_freq=1000 --param mid_gain=8 --param mid_q=2

# Reverb (room)
echo "  [13/18] Reverb (room)..."
"$SONIDO" process "$DEMOS/src_perc_adsr.wav" "$DEMOS/fx_reverb_room.wav" \
    --effect reverb --param type=room --param decay=0.4 --param damping=0.5 --param mix=0.4

# Reverb (hall)
echo "  [14/18] Reverb (hall)..."
"$SONIDO" process "$DEMOS/src_perc_adsr.wav" "$DEMOS/fx_reverb_hall.wav" \
    --effect reverb --param type=hall --param decay=0.8 --param damping=0.3 --param mix=0.5

# Delay
echo "  [15/18] Delay..."
"$SONIDO" process "$DEMOS/src_perc_adsr.wav" "$DEMOS/fx_delay.wav" \
    --effect delay --param time=375 --param feedback=0.5 --param mix=0.4

# Tape Saturation
echo "  [16/18] Tape Saturation..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_tape_saturation.wav" \
    --effect tape --param drive=8 --param saturation=0.6

# Clean Preamp
echo "  [17/18] Clean Preamp..."
"$SONIDO" process "$DEMOS/src_sine_440.wav" "$DEMOS/fx_preamp.wav" \
    --effect preamp --param gain=8

# Full effect chain: preamp -> distortion -> chorus -> delay -> reverb
echo "  [18/18] Full effect chain..."
"$SONIDO" process "$DEMOS/src_saw_chord.wav" "$DEMOS/fx_full_chain.wav" \
    --chain "preamp:gain=6|distortion:drive=10,waveshape=softclip,level=-6|chorus:rate=1,depth=0.4,mix=0.3|delay:time=300,feedback=0.3,mix=0.25|reverb:type=hall,decay=0.6,mix=0.3"

echo ""
echo "=== Done ==="
echo "Generated $(ls -1 "$DEMOS"/*.wav 2>/dev/null | wc -l) WAV files in $DEMOS/"
ls -lh "$DEMOS"/*.wav
