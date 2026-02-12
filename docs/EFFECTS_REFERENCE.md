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
sonido process in.wav --effect filter --param cutoff=2000
sonido process in.wav --effect lowpass --param cutoff=2000  # Same effect
```

## distortion

Waveshaping distortion with multiple modes.

**Signal flow** (`crates/sonido-effects/src/distortion.rs`):
```
Input -> Drive (gain) -> Waveshaper -> Tone Filter -> Output Level
```

The distortion applies a static nonlinear transfer function (waveshaper) to the input signal, preceded by a gain stage (drive) to push the signal into the nonlinear region. All parameters use `SmoothedParam` with 5ms smoothing to prevent zipper noise during adjustment.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `drive` | Drive amount in dB | 12.0 | 0-40 |
| `tone` | Tone frequency in Hz | 4000.0 | 500-10000 |
| `level` | Output level in dB | -6.0 | -20 to 0 |
| `waveshape` | Waveshape type | softclip | softclip, hardclip, foldback, asymmetric |

### Waveshape Types and Their Harmonic Character

- **softclip** (`tanh`): Hyperbolic tangent soft clipping. Produces primarily odd harmonics (3rd, 5th, 7th...) with a smooth rolloff. The gentle compression curve means the signal transitions gradually into saturation, mimicking tube amplifier behavior. Odd harmonics are musically consonant (octave + fifth, etc.).
- **hardclip**: Flat clipping at +/-1.0. Produces a dense series of odd harmonics with slower rolloff than tanh, giving a harsher, more aggressive character. Equivalent to transistor clipping stages.
- **foldback**: When the signal exceeds a threshold (default 0.8), it folds back toward zero rather than clipping. This creates both even and odd harmonics with a complex, sometimes unpredictable spectral signature. The resulting timbres are buzzy and metallic -- useful for synth processing and experimental sound design.
- **asymmetric**: Different clipping curves for positive and negative half-cycles. Asymmetric nonlinearities generate even harmonics (2nd, 4th...) in addition to odd harmonics. Even harmonics add warmth and are characteristic of single-ended tube amplifier stages (Class A).

### Tone Filter

The tone control is a one-pole lowpass filter (`distortion.rs:174-176`) placed after the waveshaper. The coefficient is computed as `1 - exp(-2*pi*freq/sample_rate)`. This tames the harsh high-frequency harmonics created by waveshaping, which is essential because nonlinear processing can generate significant energy above the original signal's bandwidth.

**Stereo processing**: In stereo mode (`process_stereo`), each channel has its own independent tone filter state (`tone_filter_state` for left, `tone_filter_state_r` for right). This ensures proper dual-mono behavior -- each channel's filtering history is independent, preventing cross-channel artifacts that would occur if a single filter state were shared between channels.

**Aliasing note**: For critical applications, wrap the distortion in `Oversampled<4, Distortion>` to suppress harmonic aliasing from the nonlinear waveshaper. At 48 kHz base rate, 4x oversampling processes at 192 kHz, keeping generated harmonics below the effective Nyquist.

### Example

```bash
sonido process in.wav --effect distortion \
    --param drive=15 --param tone=3500 --param waveshape=softclip
```

---

## compressor

Dynamics compressor with soft knee.

**Architecture** (`crates/sonido-effects/src/compressor.rs`): Feed-forward design with envelope follower, gain computer, and smoothed makeup gain.

```
Input -> Envelope Follower -> Gain Computer -> Gain Reduction -> Output
                                    |
                              Makeup Gain
```

The gain computer implements a soft-knee transfer curve (`compressor.rs:66-79`). Below the knee region, no gain reduction occurs. Within the knee (default 6 dB wide), compression increases quadratically -- this smooth transition avoids the audible "threshold artifact" of hard-knee compressors. Above the knee, full ratio compression applies.

**Stereo linking** (`compressor.rs:187-199`): In stereo mode, the envelope is derived from the sum of both channels `(L+R)/2`, and identical gain reduction is applied to both. This preserves the stereo image -- independent per-channel compression would cause phantom center shifts when one channel is louder than the other.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `threshold` | Threshold in dB | -18.0 | -60 to 0 |
| `ratio` | Compression ratio | 4.0 | 1-20 |
| `attack` | Attack time in ms | 10.0 | 0.1-100 |
| `release` | Release time in ms | 100.0 | 10-1000 |
| `makeup` | Makeup gain in dB | 0.0 | 0-24 |
| `knee` | Knee width in dB | 6.0 | 0-12 |

### Tips

- **Fast attack** (1-5ms): Catches transients, can sound "squashed"
- **Slow attack** (20-50ms): Lets transients through, more natural
- **Fast release** (50-100ms): Pumping effect, good for drums
- **Slow release** (200-500ms): Smooth, transparent compression

### Example

```bash
sonido process in.wav --effect compressor \
    --param threshold=-20 --param ratio=4 --param attack=10 --param release=100
```

---

## chorus

Dual-voice modulated delay chorus.

**How chorus works**: A chorus effect creates the illusion of multiple instruments playing in unison by mixing the dry signal with copies that have slightly varying pitch. The pitch variation is achieved by modulating a short delay time with an LFO. When a delay time changes over time, it effectively time-stretches or compresses the signal, producing a Doppler-like pitch shift.

**Implementation** (`crates/sonido-effects/src/chorus.rs`): Two `InterpolatedDelay` lines with independent LFOs provide two modulated voices. The base delay is 15 ms with up to 5 ms of LFO modulation, sweeping the total delay between 10-20 ms. The two LFOs are phase-offset by 90 degrees (`lfo2.set_phase(0.25)` at line 59) so the voices move independently, creating a richer effect.

**Stereo processing** (`chorus.rs:118-148`): In stereo mode, voice 1 is panned 80% left / 20% right and voice 2 is 20% left / 80% right. This creates a wide stereo image from a mono source -- a classic technique for thickening synth pads and guitar tracks.

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
sonido process in.wav --effect chorus \
    --param rate=1.5 --param depth=0.6 --param mix=0.5
```

---

## delay

Feedback delay with optional ping-pong stereo mode.

**Architecture** (`crates/sonido-effects/src/delay.rs`): Two `InterpolatedDelay` lines (left/right) with feedback and smoothed parameter control. The delay time parameter uses 50 ms smoothing to prevent audible pitch artifacts when changing delay time during playback.

**Ping-pong mode** (`delay.rs:129-135`): When enabled, the feedback path crosses channels -- the left delay line's output feeds back into the right delay line, and vice versa. This creates alternating left-right echoes that "bounce" across the stereo field. The effect reports `is_true_stereo() -> true` only when ping-pong is active.

**Feedback stability**: Feedback is clamped to 0.95 maximum to prevent runaway oscillation. At feedback=0.95, each echo is 95% of the previous, so the signal decays by ~0.45 dB per repeat. Complete decay below -60 dB takes approximately 130 repeats.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `time` | Delay time in ms | 300.0 | 1-2000 |
| `feedback` | Feedback amount (0-1) | 0.4 | 0-0.95 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |
| `ping_pong` | Ping-pong stereo mode (0=off, 1=on) | 0.0 | 0-1 |

### Tips

- **Slapback**: time=80-120ms, feedback=0.2, mix=0.4
- **Quarter note** (120 BPM): time=500ms
- **Dotted eighth** (120 BPM): time=375ms
- **Self-oscillation**: feedback > 0.9 (be careful with volume!)

### Example

```bash
sonido process in.wav --effect delay \
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
sonido process in.wav --effect filter \
    --param cutoff=2000 --param resonance=2.0
```

---

## multivibrato

10-unit tape wow/flutter vibrato. Also available as `vibrato`.

Simulates the complex pitch modulation of analog tape machines with 10 independent modulation units at different rates.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `depth` | Overall depth (0-1) | 0.5 | 0-1 |
| `mix` | Wet/dry mix (0-100%) | 100.0 | 0-100 |

### Tips

- **Subtle**: depth=0.2 - Adds gentle movement
- **Medium**: depth=0.5 - Classic tape sound
- **Heavy**: depth=0.8 - Obvious warble effect

### Example

```bash
sonido process in.wav --effect multivibrato --param depth=0.4
```

---

## tape

Tape saturation with HF rolloff.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `drive` | Drive amount in dB | 6.0 | 0-24 |
| `saturation` | Saturation amount (0-1) | 0.5 | 0-1 |
| `output` | Output level in dB | 0.0 | -12 to 12 |
| `hf_rolloff` | HF rolloff frequency in Hz | 12000.0 | 1000-20000 |
| `bias` | Tape bias offset | 0.0 | -0.2 to 0.2 |

### Tips

- **Subtle warmth**: drive=3, saturation=0.3
- **Tape compression**: drive=6, saturation=0.5
- **Saturated tape**: drive=12, saturation=0.7
- **Dark tape**: hf_rolloff=4000 — rolls off high frequencies earlier
- **Bias offset**: Small bias values add asymmetric harmonic content

### Example

```bash
sonido process in.wav --effect tape \
    --param drive=6 --param saturation=0.5
```

---

## preamp

Clean preamp/gain stage.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `gain` | Gain in dB | 0.0 | -20 to 20 |
| `output` | Output level in dB | 0.0 | -20 to 20 |
| `headroom` | Headroom in dB | 20.0 | 6 to 40 |

### Tips

Use before other effects to boost quiet signals, or after to control output level. The headroom parameter sets the clipping ceiling -- lower values produce softer compression at high gain.

### Example

```bash
sonido process in.wav --effect preamp --param gain=6
```

---

## reverb

Freeverb-style algorithmic reverb with 8 parallel comb filters and 4 series allpass filters.

**Freeverb topology** (`crates/sonido-effects/src/reverb.rs`): The Freeverb algorithm, originally by Jezar at Dreampoint, is one of the most widely used algorithmic reverb designs. The signal flow is:

```
Input -> Pre-delay -> [8 parallel comb filters] -> sum -> [4 series allpass filters] -> Output
```

The 8 comb filters run in parallel and their outputs are summed. Each comb filter has a different delay length, chosen to be mutually prime to avoid reinforcing resonances. The delay lengths are specified at 44.1 kHz (`reverb.rs:14`) and scaled to the actual sample rate:

```
Left:  1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617 samples (at 44.1 kHz)
Right: 1139, 1211, 1300, 1379, 1445, 1514, 1580, 1640 samples
```

The right channel uses slightly offset tunings (`reverb.rs:17`) for stereo decorrelation. This means the left and right reverb tails evolve independently, creating a wide stereo image without artificial panning.

The 4 series allpass filters provide diffusion -- they smear the distinct echoes from the comb filters into a dense, smooth reverb tail. The allpass feedback coefficient is fixed at 0.5 (`reverb.rs:139`).

**Comb filter feedback** (`reverb.rs:292-295`): The feedback coefficient combines room_size and decay parameters:
```
scaled_room = 0.28 + room_size * 0.7    (range: 0.28 to 0.98)
feedback = scaled_room + decay * (0.98 - scaled_room)
```
This mapping ensures the feedback stays below 1.0 (stable) while providing a wide range of decay times. The damping parameter controls a one-pole lowpass filter inside each comb, simulating the frequency-dependent absorption of real room surfaces -- higher damping means more high-frequency absorption per reflection, producing a darker reverb tail.

**Stereo width** (`reverb.rs:396-402`): A mid/side matrix controls stereo width. At width=0, both channels receive the average (mono). At width=1, channels are fully independent.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `room_size` | Room size (affects early reflection density) | 0.5 | 0-1 |
| `decay` | Decay time (reverb tail length) | 0.5 | 0-1 |
| `damping` | HF damping (0=bright, 1=dark) | 0.5 | 0-1 |
| `predelay` | Pre-delay time in ms | 10.0 | 0-100 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |
| `stereo_width` | Stereo width (0-100%) | 100.0 | 0-100 |
| `reverb_type` | Reverb type (0=room, 1=hall) | 0.0 | 0-1 |

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
sonido process in.wav --effect reverb \
    --param room_size=0.5 --param decay=0.6 --param mix=0.4

# Hall reverb preset
sonido process in.wav --effect reverb \
    --param reverb_type=1 --param mix=0.5

# Custom dark hall
sonido process in.wav --effect reverb \
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
sonido process in.wav --effect tremolo \
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
sonido process in.wav --effect gate \
    --param threshold=-40 --param attack=1 --param release=100 --param hold=50
```

---

## flanger

Classic flanger with modulated short delay.

**How flanging works**: Flanging is a comb filtering effect created by mixing a signal with a short, time-varying delayed copy. The delay sweeps between ~1-10 ms, producing a series of notches in the frequency spectrum at multiples of 1/delay_time. As the delay changes, the notches sweep through the spectrum, creating the characteristic "jet" or "whoosh" sound.

**Implementation** (`crates/sonido-effects/src/flanger.rs`): Base delay of 5 ms with up to 5 ms of LFO modulation (total range 1-10 ms). The feedback path feeds the delayed output back into the delay input, which deepens the comb filter notches and creates a more resonant, metallic character. At high feedback values, the comb filter approaches self-oscillation, producing pitched metallic tones.

**Stereo** (`flanger.rs:178-220`): The right channel LFO is phase-offset by 90 degrees from the left. This means the comb filter notches sweep at different times in each channel, creating a spatial motion effect. Each channel has its own delay line and feedback state.

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `rate` | LFO rate in Hz | 0.5 | 0.05-5 |
| `depth` | Modulation depth (0-1) | 0.5 | 0-1 |
| `feedback` | Feedback amount (0-1) | 0.5 | 0-0.95 |
| `mix` | Wet/dry mix (0-1) | 0.5 | 0-1 |

### Flanger vs. Chorus vs. Phaser

All three are modulation effects, but they differ in mechanism:

| Effect | Delay Range | Creates | Characteristic |
|--------|-------------|---------|----------------|
| Flanger | 1-10 ms | Comb filter (evenly-spaced notches) | Metallic, jet sweep |
| Chorus | 10-25 ms | Pitch detuning (Doppler) | Thickening, ensemble |
| Phaser | N/A (allpass) | Unevenly-spaced notches | Organic, swooshing |

### Tips

- **Subtle flanger**: rate=0.2, depth=0.3, feedback=0.3
- **Classic flanger**: rate=0.5, depth=0.5, feedback=0.5
- **Jet flanger**: rate=0.1, depth=0.8, feedback=0.8 - Slow, dramatic sweep
- **Metallic**: high feedback (0.7+) creates resonant metallic tones

### Example

```bash
sonido process in.wav --effect flanger \
    --param rate=0.5 --param depth=0.6 --param feedback=0.5 --param mix=0.5
```

---

## phaser

Multi-stage allpass phaser with LFO modulation.

**How phasing works**: A phaser creates notches in the frequency spectrum by mixing the input with a phase-shifted copy of itself. Unlike flanging (which uses comb filters with evenly-spaced notches), phasing uses cascaded first-order allpass filters whose notch positions are unevenly spaced. This produces a more organic, less metallic sound.

**Allpass filter theory** (`crates/sonido-effects/src/phaser.rs:112-135`): Each first-order allpass filter shifts the phase of the signal by up to 180 degrees, with the transition centered at a specific frequency. The coefficient is computed as:
```
a = (tan(pi * fc / fs) - 1) / (tan(pi * fc / fs) + 1)
```
When the allpass-shifted signal is mixed with the original, a notch appears at the frequency where the phase difference equals 180 degrees. Each pair of allpass stages produces one notch, so 6 stages (the default) creates 3 notches.

**Frequency sweep** (`phaser.rs:242-245`): The center frequencies use exponential mapping for a natural-sounding sweep: `freq = min_freq * (max_freq/min_freq)^(lfo * depth)`. This ensures equal time is spent per octave rather than per Hz, matching human pitch perception. The default sweep range is 200 Hz to 4000 Hz. Each stage is slightly offset by a factor of `1 + stage_index * 0.1`, spreading the notches for a richer effect.

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
sonido process in.wav --effect phaser \
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
sonido process in.wav --effect wah \
    --param frequency=700 --param resonance=6 --param sensitivity=0.7

# Manual wah (fixed position)
sonido process in.wav --effect wah \
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
sonido process in.wav --effect eq \
    --param low_freq=100 --param low_gain=4 \
    --param high_freq=8000 --param high_gain=3

# Cut mud, add presence
sonido process in.wav --effect eq \
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
--chain "distortion:drive=10|compressor:threshold=-18|reverb:reverb_type=1,mix=0.4"
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
