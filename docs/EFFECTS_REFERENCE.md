# Effects Reference

Complete parameter reference for all Sonido effects.

## distortion

Waveshaping distortion with multiple modes.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `drive` | Drive amount in dB | 12.0 | 0-40 |
| `tone` | Tone frequency in Hz | 4000.0 | 500-10000 |
| `level` | Output level in dB | -6.0 | -20 to 0 |
| `waveshape` | Waveshape type | softclip | softclip, hardclip, foldback, asymmetric |

### Waveshape Types

- **softclip**: Gentle saturation, natural tube-like overdrive
- **hardclip**: Aggressive clipping, more "transistor" sound
- **foldback**: Signal folds back on itself, creates harmonics
- **asymmetric**: Different positive/negative clipping, odd harmonics

### Example

```bash
sonido process in.wav out.wav --effect distortion \
    --param drive=15 --param tone=3500 --param waveshape=softclip
```

---

## compressor

Dynamics compressor with soft knee.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `threshold` | Threshold in dB | -18.0 | -40 to 0 |
| `ratio` | Compression ratio | 4.0 | 1-20 |
| `attack` | Attack time in ms | 10.0 | 0.1-100 |
| `release` | Release time in ms | 100.0 | 10-1000 |
| `makeup` | Makeup gain in dB | 0.0 | 0-20 |

### Tips

- **Fast attack** (1-5ms): Catches transients, can sound "squashed"
- **Slow attack** (20-50ms): Lets transients through, more natural
- **Fast release** (50-100ms): Pumping effect, good for drums
- **Slow release** (200-500ms): Smooth, transparent compression

### Example

```bash
sonido process in.wav out.wav --effect compressor \
    --param threshold=-20 --param ratio=4 --param attack=10 --param release=100
```

---

## chorus

Dual-voice modulated delay chorus.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `rate` | LFO rate in Hz | 1.0 | 0.1-10 |
| `depth` | Modulation depth (0-1) | 0.5 | 0-1 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |

### Tips

- **Subtle chorus**: rate=0.5, depth=0.3, mix=0.3
- **Classic chorus**: rate=1.0, depth=0.5, mix=0.5
- **Thick chorus**: rate=2.0, depth=0.7, mix=0.6

### Example

```bash
sonido process in.wav out.wav --effect chorus \
    --param rate=1.5 --param depth=0.6 --param mix=0.5
```

---

## delay

Tape-style feedback delay.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `time` | Delay time in ms | 300.0 | 1-2000 |
| `feedback` | Feedback amount (0-1) | 0.4 | 0-0.95 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |

### Tips

- **Slapback**: time=80-120ms, feedback=0.2, mix=0.4
- **Quarter note** (120 BPM): time=500ms
- **Dotted eighth** (120 BPM): time=375ms
- **Self-oscillation**: feedback > 0.9 (be careful with volume!)

### Example

```bash
sonido process in.wav out.wav --effect delay \
    --param time=375 --param feedback=0.5 --param mix=0.4
```

---

## filter

Resonant lowpass filter (2-pole).

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `cutoff` | Cutoff frequency in Hz | 1000.0 | 20-20000 |
| `resonance` | Resonance (Q factor) | 0.707 | 0.1-10 |

### Tips

- **Q = 0.707**: Butterworth response, flattest passband
- **Q > 1**: Resonant peak at cutoff
- **Q > 5**: Strong resonance, almost self-oscillating

### Example

```bash
sonido process in.wav out.wav --effect filter \
    --param cutoff=2000 --param resonance=2.0
```

---

## multivibrato

10-unit tape wow/flutter vibrato.

Simulates the complex pitch modulation of analog tape machines with 10 independent modulation units at different rates.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `depth` | Overall depth (0-1) | 0.5 | 0-1 |

### Tips

- **Subtle**: depth=0.2 - Adds gentle movement
- **Medium**: depth=0.5 - Classic tape sound
- **Heavy**: depth=0.8 - Obvious warble effect

### Example

```bash
sonido process in.wav out.wav --effect multivibrato --param depth=0.4
```

---

## tape

Tape saturation with HF rolloff.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `drive` | Drive amount in dB | 6.0 | 0-24 |
| `saturation` | Saturation amount (0-1) | 0.5 | 0-1 |

### Tips

- **Subtle warmth**: drive=3, saturation=0.3
- **Tape compression**: drive=6, saturation=0.5
- **Saturated tape**: drive=12, saturation=0.7

### Example

```bash
sonido process in.wav out.wav --effect tape \
    --param drive=6 --param saturation=0.5
```

---

## preamp

Clean preamp/gain stage.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `gain` | Gain in dB | 0.0 | -20 to 20 |

### Tips

Use before other effects to boost quiet signals, or after to control output level.

### Example

```bash
sonido process in.wav out.wav --effect preamp --param gain=6
```

---

## reverb

Freeverb-style algorithmic reverb with 8 parallel comb filters and 4 series allpass filters.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `room_size` | Room size (affects early reflection density) | 0.5 | 0-1 |
| `decay` | Decay time (reverb tail length) | 0.5 | 0-1 |
| `damping` | HF damping (0=bright, 1=dark) | 0.5 | 0-1 |
| `predelay` | Pre-delay time in ms | 10.0 | 0-100 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |
| `type` | Reverb type preset | room | room, hall |

### Reverb Types

- **room**: Small room with short decay (room_size=0.4, decay=0.5, damping=0.5, predelay=10ms)
- **hall**: Large hall with long decay (room_size=0.8, decay=0.8, damping=0.3, predelay=25ms)

### Tips

- **Small room**: room_size=0.3, decay=0.4, damping=0.6 - Tight, intimate sound
- **Medium room**: room_size=0.5, decay=0.6, damping=0.4 - Balanced, natural
- **Large hall**: room_size=0.8, decay=0.85, damping=0.25 - Spacious, epic
- **Dark reverb**: damping=0.7-0.9 - Muffled, vintage sound
- **Bright reverb**: damping=0.1-0.3 - Shimmery, modern sound
- **Pre-delay**: 10-30ms for clarity, keeps source separate from reverb

### Example

```bash
# Room reverb
sonido process in.wav out.wav --effect reverb \
    --param room_size=0.5 --param decay=0.6 --param mix=0.4

# Hall reverb preset
sonido process in.wav out.wav --effect reverb \
    --param type=hall --param mix=0.5

# Custom dark hall
sonido process in.wav out.wav --effect reverb \
    --param room_size=0.8 --param decay=0.9 --param damping=0.7 --param predelay=25
```

---

## Effect Chains

### Chain Syntax

```
effect1:param1=value1,param2=value2|effect2:param=value
```

### Common Chains

**Guitar Crunch**
```bash
--chain "preamp:gain=6|distortion:drive=12,tone=4000"
```

**Tape Echo**
```bash
--chain "tape:drive=4,saturation=0.4|delay:time=350,feedback=0.45"
```

**Lush Chorus**
```bash
--chain "compressor:threshold=-18,ratio=3|chorus:rate=1,depth=0.5|delay:time=30,mix=0.2"
```

**Lo-Fi**
```bash
--chain "tape:drive=8,saturation=0.6|filter:cutoff=4000|multivibrato:depth=0.3"
```

**Ambient Wash**
```bash
--chain "delay:time=400,feedback=0.5,mix=0.3|reverb:decay=0.9,room_size=0.8,mix=0.6"
```

**Guitar Hall**
```bash
--chain "distortion:drive=10|compressor:threshold=-18|reverb:type=hall,mix=0.4"
```
