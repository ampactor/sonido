# Effects Reference

Complete parameter reference for all Sonido effects.

## Effect Aliases

Some effects have alternate names for convenience:

| Primary Name | Alias | Notes |
|-------------|-------|-------|
| `filter` | `lowpass` | Resonant lowpass filter |
| `multivibrato` | `vibrato` | 10-unit tape wow/flutter |
| `gate` | `noisegate` | Noise gate |
| `wah` | `autowah` | Auto-wah/manual wah |
| `eq` | `parametriceq`, `peq` | 3-band parametric EQ |

Both names work interchangeably in the CLI:

```bash
sonido process in.wav out.wav --effect filter --param cutoff=2000
sonido process in.wav out.wav --effect lowpass --param cutoff=2000  # Same effect
```

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

Resonant lowpass filter (2-pole). Also available as `lowpass`.

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

10-unit tape wow/flutter vibrato. Also available as `vibrato`.

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

## tremolo

Amplitude modulation with multiple waveforms.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `rate` | LFO rate in Hz | 5.0 | 0.5-20 |
| `depth` | Modulation depth (0-1) | 0.5 | 0-1 |
| `waveform` | Waveform type | sine | sine, triangle, square, samplehold |

### Waveform Types

- **sine**: Smooth, classic tremolo sound
- **triangle**: Slightly more aggressive with linear ramps
- **square**: Choppy, on/off effect (helicopter)
- **samplehold**: Random stepped levels, creates rhythmic variations

### Tips

- **Subtle tremolo**: rate=4, depth=0.3 - Gentle pulsing
- **Classic tremolo**: rate=6, depth=0.5 - Vintage amp sound
- **Choppy tremolo**: rate=8, depth=0.8, waveform=square - Rhythmic gating

### Example

```bash
sonido process in.wav out.wav --effect tremolo \
    --param rate=6 --param depth=0.5 --param waveform=sine
```

---

## gate

Noise gate with threshold and hold.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `threshold` | Threshold in dB | -40.0 | -80 to 0 |
| `attack` | Attack time in ms | 1.0 | 0.1-50 |
| `release` | Release time in ms | 100.0 | 10-1000 |
| `hold` | Hold time in ms | 50.0 | 0-500 |

### Tips

- **Quiet signals**: Set threshold just above the noise floor
- **Fast attack** (0.5-2ms): Preserves transients
- **Hold time**: Prevents rapid gate flutter on sustained notes
- **Release**: Longer release (200-500ms) for smoother fade-out

### Example

```bash
sonido process in.wav out.wav --effect gate \
    --param threshold=-40 --param attack=1 --param release=100 --param hold=50
```

---

## flanger

Classic flanger with modulated short delay.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `rate` | LFO rate in Hz | 0.5 | 0.05-5 |
| `depth` | Modulation depth (0-1) | 0.5 | 0-1 |
| `feedback` | Feedback amount (0-1) | 0.5 | 0-0.95 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |

### Tips

- **Subtle flanger**: rate=0.2, depth=0.3, feedback=0.3
- **Classic flanger**: rate=0.5, depth=0.5, feedback=0.5
- **Jet flanger**: rate=0.1, depth=0.8, feedback=0.8 - Slow, dramatic sweep
- **Metallic**: high feedback (0.7+) creates resonant metallic tones

### Example

```bash
sonido process in.wav out.wav --effect flanger \
    --param rate=0.5 --param depth=0.6 --param feedback=0.5 --param mix=0.5
```

---

## phaser

Multi-stage allpass phaser with LFO modulation.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `rate` | LFO rate in Hz | 0.3 | 0.05-5 |
| `depth` | Frequency sweep range (0-1) | 0.5 | 0-1 |
| `stages` | Number of allpass stages | 6 | 2-12 |
| `feedback` | Feedback/resonance (0-1) | 0.5 | 0-0.95 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |

### Tips

- **Subtle phaser**: stages=4, depth=0.4, feedback=0.3
- **Classic phaser**: stages=6, depth=0.5, feedback=0.5
- **Deep phaser**: stages=8-12, depth=0.7, feedback=0.6
- **More stages**: Creates more notches, thicker effect
- **High feedback**: Resonant, almost filter-like

### Example

```bash
sonido process in.wav out.wav --effect phaser \
    --param rate=0.4 --param depth=0.6 --param stages=6 --param feedback=0.5
```

---

## wah

Auto-wah and manual wah with envelope follower. Also available as `autowah`.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `frequency` | Center frequency in Hz | 800.0 | 200-2000 |
| `resonance` | Filter Q (sharpness) | 5.0 | 1-10 |
| `sensitivity` | Envelope sensitivity (0-1) | 0.5 | 0-1 |
| `mode` | Wah mode | auto | auto, manual |

### Modes

- **auto**: Envelope follower tracks input level, playing dynamics control wah sweep
- **manual**: Frequency parameter directly controls wah position

### Tips

- **Classic auto-wah**: frequency=600, resonance=6, sensitivity=0.7
- **Subtle envelope**: sensitivity=0.3-0.5 for gentle sweep
- **Aggressive**: sensitivity=0.8-1.0 for full range sweep
- **High Q** (8-10): Classic narrow wah tone
- **Low Q** (2-4): Wider, smoother sweep

### Example

```bash
# Auto-wah
sonido process in.wav out.wav --effect wah \
    --param frequency=700 --param resonance=6 --param sensitivity=0.7

# Manual wah (fixed position)
sonido process in.wav out.wav --effect wah \
    --param frequency=1200 --param mode=manual
```

---

## eq

3-band parametric equalizer. Also available as `parametriceq` or `peq`.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `low_freq` | Low band frequency in Hz | 100.0 | 20-500 |
| `low_gain` | Low band gain in dB | 0.0 | -12 to 12 |
| `low_q` | Low band Q | 1.0 | 0.5-5 |
| `mid_freq` | Mid band frequency in Hz | 1000.0 | 200-5000 |
| `mid_gain` | Mid band gain in dB | 0.0 | -12 to 12 |
| `mid_q` | Mid band Q | 1.0 | 0.5-5 |
| `high_freq` | High band frequency in Hz | 5000.0 | 1000-15000 |
| `high_gain` | High band gain in dB | 0.0 | -12 to 12 |
| `high_q` | High band Q | 1.0 | 0.5-5 |

### Tips

- **Wide Q** (0.5-1): Gentle, musical boosts/cuts
- **Narrow Q** (3-5): Surgical, precise adjustments
- **Bass boost**: low_freq=80, low_gain=4, low_q=0.7
- **Presence boost**: mid_freq=3000, mid_gain=3, mid_q=1.5
- **Air/brilliance**: high_freq=10000, high_gain=3, high_q=0.7
- **Mud cut**: mid_freq=300, mid_gain=-4, mid_q=1.5

### Short Parameter Names

For convenience, you can use abbreviated parameter names:
- `lf`, `lg`, `lq` for low band
- `mf`, `mg`, `mq` for mid band
- `hf`, `hg`, `hq` for high band

### Example

```bash
# Boost bass and highs (smiley face EQ)
sonido process in.wav out.wav --effect eq \
    --param low_freq=100 --param low_gain=4 \
    --param high_freq=8000 --param high_gain=3

# Cut mud, add presence
sonido process in.wav out.wav --effect eq \
    --param mf=300 --param mg=-4 --param mq=1.5 \
    --param mf=3000 --param mg=2
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

**Funk Wah**
```bash
--chain "compressor:threshold=-15,ratio=4|wah:sensitivity=0.8,resonance=7"
```

**Clean Chorus with EQ**
```bash
--chain "eq:lf=80,lg=3,hf=6000,hg=2|chorus:rate=1,depth=0.5|reverb:mix=0.3"
```

**Gated Tremolo**
```bash
--chain "gate:threshold=-45|tremolo:rate=6,depth=0.7,waveform=square"
```

**80s Phaser Lead**
```bash
--chain "distortion:drive=8|phaser:rate=0.3,stages=8,feedback=0.6|delay:time=350,feedback=0.4"
```

**Jet Flanger**
```bash
--chain "compressor:threshold=-20|flanger:rate=0.1,depth=0.9,feedback=0.8"
```
