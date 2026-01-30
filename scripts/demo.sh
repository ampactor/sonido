#!/bin/bash
# Generate demo audio files for Sonido

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
DEMO_DIR="$PROJECT_DIR/demos"
PRESET_DIR="$PROJECT_DIR/presets"

# Build release binary if not present
if [ ! -f "$PROJECT_DIR/target/release/sonido" ]; then
    echo "Building sonido CLI..."
    cargo build --release --manifest-path "$PROJECT_DIR/Cargo.toml" -p sonido-cli
fi

SONIDO="$PROJECT_DIR/target/release/sonido"

# Create demos directory
mkdir -p "$DEMO_DIR"

echo "=== Sonido Demo Generator ==="
echo ""

# Generate test signals
echo "Generating test signals..."
$SONIDO generate tone "$DEMO_DIR/tone_440.wav" --freq 440 --duration 2.0
$SONIDO generate sweep "$DEMO_DIR/sweep.wav" --start 20 --end 20000 --duration 3.0
$SONIDO generate noise "$DEMO_DIR/noise.wav" --duration 1.0 --amplitude 0.3

echo ""
echo "Processing through presets..."

# Process through each preset
for preset in "$PRESET_DIR"/*.toml; do
    if [ -f "$preset" ]; then
        name=$(basename "$preset" .toml)
        echo "  Processing with $name..."
        $SONIDO process "$DEMO_DIR/tone_440.wav" "$DEMO_DIR/${name}_tone.wav" --preset "$preset"
        $SONIDO process "$DEMO_DIR/sweep.wav" "$DEMO_DIR/${name}_sweep.wav" --preset "$preset"
    fi
done

echo ""
echo "=== Demo files created in $DEMO_DIR ==="
ls -la "$DEMO_DIR"/*.wav

echo ""
echo "To listen to a demo file:"
echo "  aplay $DEMO_DIR/guitar_crunch_tone.wav  # Linux"
echo "  afplay $DEMO_DIR/guitar_crunch_tone.wav # macOS"
