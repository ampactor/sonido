# Hothouse Hardware Reference

Quick reference for DSP control design on the Cleveland Music Co. Hothouse platform.

## Physical Controls

| Control | Type | ADC/GPIO | Values | Notes |
|---------|------|----------|--------|-------|
| KNOB_1 | 10K pot | ADC | 0.0–1.0 float | Top left |
| KNOB_2 | 10K pot | ADC | 0.0–1.0 float | Top center |
| KNOB_3 | 10K pot | ADC | 0.0–1.0 float | Top right |
| KNOB_4 | 10K pot | ADC | 0.0–1.0 float | Bottom left |
| KNOB_5 | 10K pot | ADC | 0.0–1.0 float | Bottom center |
| KNOB_6 | 10K pot | ADC | 0.0–1.0 float | Bottom right |
| TOGGLE_1 | 3-way (ON-OFF-ON) | GPIO | UP / MIDDLE / DOWN | Left toggle |
| TOGGLE_2 | 3-way (ON-OFF-ON) | GPIO | UP / MIDDLE / DOWN | Center toggle |
| TOGGLE_3 | 3-way (ON-OFF-ON) | GPIO | UP / MIDDLE / DOWN | Right toggle |
| FOOTSWITCH_1 | Momentary | GPIO | pressed / released | Left footswitch |
| FOOTSWITCH_2 | Momentary | GPIO | pressed / released | Right footswitch |
| LED_1 | Status LED | GPIO | on / off | Left LED |
| LED_2 | Status LED | GPIO | on / off | Right LED |

## Audio I/O

| Port | Type | Channels | Level |
|------|------|----------|-------|
| INPUT | 1/4" TRS | Stereo (tip=L, ring=R) | Instrument level |
| OUTPUT | 1/4" TRS | Stereo (tip=L, ring=R) | Instrument level |

**Audio modes** (software-defined):
- Mono in → Mono out
- Mono in → Stereo out
- Stereo in → Stereo out
- Mono in → Dual mono out

## Control Combinatorics

| Controls | States | Use case |
|----------|--------|----------|
| 3 toggles (3-way each) | 27 combinations | Effect/bank selection |
| 6 knobs | Continuous | Per-effect parameters |
| 2 footswitches | 4 combinations | Bypass, tap, preset |

**Example mapping for multi-effect:**
```
TOGGLE_1: Effect category (delay / reverb / mod)
TOGGLE_2: Algorithm variant (3 per category = 9 total)
TOGGLE_3: Routing mode (series / parallel / bypass)

Result: 27 distinct effect configurations
```

## Signal Level Limitations

**Designed for:** Instrument level (100mV – 1V peak-to-peak)

| Source | Level | Hothouse compatibility |
|--------|-------|------------------------|
| Guitar (passive) | ~200mV p-p | ✅ Optimal |
| Guitar (active) | ~500mV p-p | ✅ Fine |
| Bass | ~300mV p-p | ✅ Fine |
| Synth line out | ~2.8V p-p | ⚠️ Too hot, turn down or pad |
| Eurorack | 5–10V p-p | ❌ Will clip hard |

**For hot signals:** Use external attenuator, reamp box, or turn down source volume. The Hothouse input buffer is not designed for line/modular level.

**Impedance:** 1MΩ input (guitar pickup optimized), may affect tone from low-Z sources.

## Pin Mapping (Daisy Seed)

```
Knobs (ADC):
  KNOB_1 = PIN_21 (A0)
  KNOB_2 = PIN_22 (A1)
  KNOB_3 = PIN_23 (A2)
  KNOB_4 = PIN_24 (A3)
  KNOB_5 = PIN_25 (A4)
  KNOB_6 = PIN_28 (A5)

Toggles (GPIO):
  TOGGLE_1_UP   = PIN_5
  TOGGLE_1_DOWN = PIN_6
  TOGGLE_2_UP   = PIN_7
  TOGGLE_2_DOWN = PIN_8
  TOGGLE_3_UP   = PIN_9
  TOGGLE_3_DOWN = PIN_10

Footswitches (GPIO):
  FOOTSWITCH_1 = PIN_27
  FOOTSWITCH_2 = PIN_14

LEDs (GPIO):
  LED_1 = PIN_4
  LED_2 = PIN_3

Audio (Codec):
  AUDIO_IN_L  = Seed audio in L
  AUDIO_IN_R  = Seed audio in R
  AUDIO_OUT_L = Seed audio out L
  AUDIO_OUT_R = Seed audio out R
```

## Free Pins for Expansion

Available for OLED, MIDI, or other additions:

| Pin | Function | Suggested use |
|-----|----------|---------------|
| D11 | I2C SCL | OLED display |
| D12 | I2C SDA | OLED display |
| D13 | UART TX | MIDI out |
| D14 | UART RX | MIDI in |

## Software Considerations

**Debouncing:** Toggles and footswitches need software debounce (~20-50ms)

**Knob smoothing:** ADC values jitter; apply exponential smoothing or hysteresis

**Toggle reading:** Each 3-way toggle uses 2 GPIO pins:
```rust
match (up_pin, down_pin) {
    (true, false)  => Position::Up,
    (false, false) => Position::Middle,
    (false, true)  => Position::Down,
    _ => unreachable!(), // both true = hardware fault
}
```

**Footswitch modes:**
- Momentary: Read state directly
- Latching (software): Toggle internal state on press
- Long-press: Detect hold duration for secondary function

## Design Patterns

**Bank + Preset system:**
```
TOGGLE_1 = Bank (A / B / C)
TOGGLE_2 = Preset within bank (1 / 2 / 3)
TOGGLE_3 = Modifier (normal / alt / extended)

Total: 27 presets accessible without menus
```

**Parameter pages (no display):**
```
FOOTSWITCH_2 long-press = cycle parameter page
LED_2 blink pattern = indicate current page
KNOB_1-6 = different params per page
```

**Tap tempo:**
```
FOOTSWITCH_1 tap = record interval
FOOTSWITCH_1 hold = reset to default
LED_1 blink = tempo indicator
```

---

## For Lionel's "Hot + Clean" Requirement

The Hothouse as-is won't handle line level or hotter without clipping. Options:

1. **External pad** — $20 passive attenuator before input
2. **Turn down source** — Works for synths with volume knobs  
3. **Phase 2 custom build** — Design platform with variable input gain

For initial DSP development: Hothouse is fine with guitar. Worry about hot signals when the algorithms are proven.