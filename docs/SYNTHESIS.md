# Synthesis Guide

Complete guide to the `sonido-synth` crate for building synthesizers.

## Overview

The `sonido-synth` crate provides audio synthesis building blocks including:

- **Oscillators** - PolyBLEP anti-aliased waveforms (sine, saw, square, triangle, pulse, noise)
- **Envelopes** - ADSR envelope generators with exponential curves
- **Voice Management** - Polyphonic voice allocation with multiple stealing strategies
- **Modulation Matrix** - Flexible routing from sources to destinations
- **Complete Synths** - Ready-to-use monophonic and polyphonic synthesizers

All components are `no_std` compatible for embedded use.

## Quick Start

```rust
use sonido_synth::{PolyphonicSynth, OscillatorWaveform, VoiceAllocationMode};

// Create an 8-voice synth at 48kHz
let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(48000.0);

// Configure the sound
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_osc2_waveform(OscillatorWaveform::Saw);
synth.set_osc2_detune(7.0);  // 7 cents for thickness
synth.set_filter_cutoff(2000.0);
synth.set_filter_resonance(2.0);
synth.set_amp_attack(10.0);
synth.set_amp_release(500.0);

// Play a chord
synth.note_on(60, 100);  // C4
synth.note_on(64, 100);  // E4
synth.note_on(67, 100);  // G4

// Generate audio
let mut buffer = vec![0.0; 1024];
for sample in buffer.iter_mut() {
    *sample = synth.process();
}
```

---

## Oscillators

The `Oscillator` type generates audio-rate waveforms with PolyBLEP (Polynomial Band-Limited Step) anti-aliasing to reduce aliasing artifacts on non-sinusoidal waveforms.

### Waveform Types

| Waveform | Description | Harmonics |
|----------|-------------|-----------|
| `Sine` | Pure tone | Fundamental only |
| `Triangle` | Soft, flute-like | Odd harmonics, -12dB/octave |
| `Saw` | Bright, brassy | All harmonics, -6dB/octave |
| `Square` | Hollow, clarinet-like | Odd harmonics, -6dB/octave |
| `Pulse(duty)` | Variable duty cycle | Depends on duty cycle |
| `Noise` | White noise | Broadband |

### Basic Usage

```rust
use sonido_synth::{Oscillator, OscillatorWaveform};

let mut osc = Oscillator::new(48000.0);
osc.set_frequency(440.0);  // A4
osc.set_waveform(OscillatorWaveform::Saw);

// Generate samples
let sample = osc.advance();
```

### Phase Modulation (FM Synthesis)

```rust
// Create carrier and modulator
let mut carrier = Oscillator::new(48000.0);
let mut modulator = Oscillator::new(48000.0);

carrier.set_frequency(440.0);
modulator.set_frequency(880.0);  // 2:1 ratio

// Generate FM synthesis
let mod_depth = 2.0;  // Modulation index
let mod_signal = modulator.advance() * mod_depth;
let output = carrier.advance_with_pm(mod_signal);
```

### Hard Sync

```rust
let mut master = Oscillator::new(48000.0);
let mut slave = Oscillator::new(48000.0);

master.set_frequency(100.0);
slave.set_frequency(300.0);

// In audio loop:
let master_sample = master.advance();
if master.phase() < master_phase_inc {  // Detect cycle completion
    slave.sync();  // Reset slave phase
}
let slave_sample = slave.advance();
```

### Pulse Width Modulation

```rust
let mut osc = Oscillator::new(48000.0);
osc.set_frequency(440.0);

// Animate duty cycle for PWM effect
let duty = 0.25 + lfo_value * 0.24;  // Range 0.01 - 0.49
osc.set_waveform(OscillatorWaveform::Pulse(duty));
```

---

## ADSR Envelopes

The `AdsrEnvelope` generates attack-decay-sustain-release envelopes with exponential curves for natural-sounding modulation.

### Parameters

| Parameter | Description | Default | Range |
|-----------|-------------|---------|-------|
| `attack_ms` | Time to reach peak | 10.0 | 0.1+ |
| `decay_ms` | Time to reach sustain | 100.0 | 0.1+ |
| `sustain` | Sustain level | 0.7 | 0.0-1.0 |
| `release_ms` | Time to reach zero | 200.0 | 0.1+ |

### Basic Usage

```rust
use sonido_synth::{AdsrEnvelope, EnvelopeState};

let mut env = AdsrEnvelope::new(48000.0);
env.set_attack_ms(10.0);
env.set_decay_ms(100.0);
env.set_sustain(0.7);
env.set_release_ms(200.0);

// Trigger
env.gate_on();

// Process samples
for _ in 0..1000 {
    let level = env.advance();
    // Use level to modulate amplitude, filter, etc.
}

// Release
env.gate_off();
```

### Envelope States

```rust
match env.state() {
    EnvelopeState::Idle => { /* Silent */ }
    EnvelopeState::Attack => { /* Rising to peak */ }
    EnvelopeState::Decay => { /* Falling to sustain */ }
    EnvelopeState::Sustain => { /* Holding at sustain level */ }
    EnvelopeState::Release => { /* Fading to zero */ }
}
```

### Retriggering

The envelope supports smooth retriggering - calling `gate_on()` while active preserves the current level to avoid clicks:

```rust
env.gate_on();
// Process some samples...
env.gate_on();  // Retrigger without resetting level
```

---

## Voice Management

### Single Voice

A `Voice` contains all components for one synth voice:

```rust
use sonido_synth::Voice;

let mut voice = Voice::new(48000.0);

// Trigger note (MIDI note, velocity)
voice.note_on(60, 100);

// Generate samples
for _ in 0..1000 {
    let sample = voice.process();
}

// Release
voice.note_off();
```

### Voice Configuration

```rust
// Oscillator settings
voice.osc1.set_waveform(OscillatorWaveform::Saw);
voice.osc2.set_waveform(OscillatorWaveform::Saw);
voice.set_osc2_detune(7.0);  // Cents
voice.set_osc_mix(0.5);      // 0 = osc1 only, 1 = osc2 only

// Filter settings
voice.set_filter_cutoff(2000.0);
voice.filter.set_resonance(2.0);
voice.set_filter_env_amount(4000.0);  // Hz modulation from envelope

// Envelope settings
voice.amp_env.set_attack_ms(10.0);
voice.amp_env.set_release_ms(500.0);
voice.filter_env.set_attack_ms(5.0);
voice.filter_env.set_decay_ms(200.0);
```

### Polyphonic Voice Manager

```rust
use sonido_synth::{VoiceManager, VoiceAllocationMode};

// 8-voice polyphony
let mut manager: VoiceManager<8> = VoiceManager::new(48000.0);
manager.set_allocation_mode(VoiceAllocationMode::OldestNote);

// Play notes
manager.note_on(60, 100);
manager.note_on(64, 100);
manager.note_on(67, 100);

// Process
let sample = manager.process();

// Release
manager.note_off(60);

// Panic - stop all notes
manager.all_notes_off();
```

### Voice Allocation Modes

| Mode | Description |
|------|-------------|
| `RoundRobin` | Cycle through voices in order (default) |
| `OldestNote` | Steal the oldest active note |
| `LowestNote` | Steal the lowest pitch |
| `HighestNote` | Steal the highest pitch |

---

## Modulation Matrix

The modulation matrix routes modulation sources to destinations with configurable amounts.

### Sources

| Source | Description |
|--------|-------------|
| `Lfo1`, `Lfo2` | LFO outputs |
| `AmpEnv`, `FilterEnv`, `ModEnv` | Envelope outputs |
| `Velocity` | Note velocity (0-1) |
| `Aftertouch` | Channel pressure |
| `ModWheel` | CC1 mod wheel |
| `PitchBend` | Pitch bend wheel |
| `AudioIn` | Envelope follower on audio input |
| `KeyTrack` | Note number (centered at C4) |
| `Custom1`, `Custom2` | User-defined sources |

### Destinations

| Destination | Description |
|-------------|-------------|
| `Osc1Pitch`, `Osc2Pitch` | Oscillator pitch (semitones) |
| `Osc1PulseWidth`, `Osc2PulseWidth` | Pulse width |
| `OscMix` | Oscillator balance |
| `FilterCutoff` | Filter cutoff frequency |
| `FilterResonance` | Filter Q |
| `Amplitude` | Output level |
| `Pan` | Stereo position |
| `Lfo1Rate`, `Lfo2Rate` | LFO speed |
| `EffectParam1`, `EffectParam2` | Effect parameters |

### Usage

```rust
use sonido_synth::{ModulationMatrix, ModulationRoute, ModulationValues,
                   ModSourceId, ModDestination};

// Create matrix with 8 routing slots
let mut matrix: ModulationMatrix<8> = ModulationMatrix::new();

// Route LFO1 to filter cutoff (vibrato-style)
matrix.add_route(ModulationRoute::new(
    ModSourceId::Lfo1,
    ModDestination::FilterCutoff,
    0.5,  // Amount (-1 to 1)
));

// Route filter envelope to cutoff (unipolar for standard envelope)
matrix.add_route(ModulationRoute::unipolar(
    ModSourceId::FilterEnv,
    ModDestination::FilterCutoff,
    0.8,
));

// Route velocity to amplitude
matrix.add_route(ModulationRoute::unipolar(
    ModSourceId::Velocity,
    ModDestination::Amplitude,
    1.0,
));

// Compute total modulation
let mut values = ModulationValues::new();
values.lfo1 = lfo.advance();
values.filter_env = filter_env.advance();
values.set_velocity_from_midi(velocity);

let filter_mod = matrix.get_modulation(ModDestination::FilterCutoff, &values);
let cutoff = base_cutoff + filter_mod * mod_range;
```

---

## Complete Synthesizers

### MonophonicSynth

Single-voice synth with glide/portamento, ideal for bass and lead sounds:

```rust
use sonido_synth::{MonophonicSynth, OscillatorWaveform};
use sonido_core::LfoWaveform;

let mut synth = MonophonicSynth::new(48000.0);

// Oscillators
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_osc2_waveform(OscillatorWaveform::Square);
synth.set_osc2_detune(7.0);
synth.set_osc_mix(0.3);

// Filter
synth.set_filter_cutoff(1500.0);
synth.set_filter_resonance(3.0);
synth.set_filter_env_amount(3000.0);
synth.set_filter_key_track(0.5);

// Envelopes
synth.set_amp_attack(5.0);
synth.set_amp_decay(100.0);
synth.set_amp_sustain(0.8);
synth.set_amp_release(200.0);

synth.set_filter_attack(10.0);
synth.set_filter_decay(200.0);
synth.set_filter_sustain(0.3);
synth.set_filter_release(300.0);

// LFOs
synth.set_lfo1_rate(5.0);
synth.set_lfo1_waveform(LfoWaveform::Triangle);
synth.set_lfo1_to_pitch(0.1);    // Subtle vibrato
synth.set_lfo1_to_filter(200.0); // Filter wobble

// Glide
synth.set_glide_time(50.0);  // 50ms portamento

// Play
synth.note_on(48, 100);
```

### PolyphonicSynth

Multi-voice synth for pads and chords:

```rust
use sonido_synth::{PolyphonicSynth, OscillatorWaveform, VoiceAllocationMode};

// 8 voices
let mut synth: PolyphonicSynth<8> = PolyphonicSynth::new(48000.0);
synth.set_allocation_mode(VoiceAllocationMode::OldestNote);

// Sound design
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_osc2_waveform(OscillatorWaveform::Saw);
synth.set_osc2_detune(10.0);
synth.set_filter_cutoff(3000.0);
synth.set_filter_env_amount(2000.0);

synth.set_amp_attack(50.0);
synth.set_amp_decay(200.0);
synth.set_amp_sustain(0.7);
synth.set_amp_release(1000.0);

// LFO modulation
synth.set_lfo1_rate(0.5);
synth.set_lfo1_to_filter(500.0);  // Slow filter sweep

// Play chord
synth.note_on(60, 80);
synth.note_on(64, 80);
synth.note_on(67, 80);
synth.note_on(72, 80);

// Process
for _ in 0..buffer_size {
    let sample = synth.process();
    // or stereo:
    let (left, right) = synth.process_stereo();
}
```

---

## Audio Modulation

Use audio input as a modulation source (sidechain, envelope follower):

```rust
use sonido_synth::AudioModSource;
use sonido_core::ModulationSource;

let mut audio_mod = AudioModSource::new(48000.0);
audio_mod.set_attack_ms(5.0);
audio_mod.set_release_ms(50.0);
audio_mod.set_threshold(0.1);  // Gate threshold

// In audio loop:
audio_mod.process(sidechain_input);
let mod_value = audio_mod.mod_advance();  // -1 to 1

// Use for sidechain ducking
let ducking = 1.0 - (mod_value + 1.0) * 0.5;  // Invert envelope
output *= ducking;
```

### Audio Gate

Trigger envelopes from audio input:

```rust
use sonido_synth::AudioGate;

let mut gate = AudioGate::new(48000.0);
gate.set_open_threshold(0.2);
gate.set_close_threshold(0.1);  // Hysteresis

// In audio loop:
if gate.process(audio_input) {
    if !was_open {
        envelope.gate_on();  // Trigger on gate open
    }
} else {
    if was_open {
        envelope.gate_off();  // Release on gate close
    }
}
```

---

## Tempo Sync

Synchronize LFOs and delays to musical tempo:

```rust
use sonido_core::{TempoManager, NoteDivision, Lfo};

let mut tempo = TempoManager::new(48000.0, 120.0);  // 120 BPM
tempo.play();

// Get tempo-synced LFO rate
let lfo_freq = tempo.division_to_hz(NoteDivision::Eighth);  // 4 Hz at 120 BPM
let mut lfo = Lfo::new(48000.0, lfo_freq);

// Get delay time
let delay_ms = tempo.division_to_ms(NoteDivision::DottedEighth);  // 375ms at 120 BPM
```

### Note Divisions

| Division | Beats | At 120 BPM |
|----------|-------|------------|
| `Whole` | 4 | 2000ms |
| `Half` | 2 | 1000ms |
| `Quarter` | 1 | 500ms |
| `Eighth` | 0.5 | 250ms |
| `Sixteenth` | 0.25 | 125ms |
| `DottedQuarter` | 1.5 | 750ms |
| `DottedEighth` | 0.75 | 375ms |
| `TripletEighth` | 0.333 | 166.7ms |

---

## Utility Functions

### MIDI to Frequency

```rust
use sonido_synth::{midi_to_freq, freq_to_midi, cents_to_ratio};

// A4 = MIDI note 69 = 440 Hz
let freq = midi_to_freq(69);  // 440.0

// Middle C = MIDI note 60 = 261.63 Hz
let freq = midi_to_freq(60);  // 261.63

// Convert back
let note = freq_to_midi(440.0);  // 69.0

// Cents to frequency ratio
let ratio = cents_to_ratio(100.0);  // 1 semitone up
let ratio = cents_to_ratio(1200.0); // 1 octave up = 2.0
```

---

## no_std Usage

For embedded systems:

```toml
[dependencies]
sonido-synth = { version = "0.1", default-features = false }
```

```rust
#![no_std]

use sonido_synth::{Oscillator, OscillatorWaveform, AdsrEnvelope};

// Pre-allocated buffer
static mut BUFFER: [f32; 256] = [0.0; 256];

fn process_audio(osc: &mut Oscillator, env: &mut AdsrEnvelope) {
    unsafe {
        for sample in BUFFER.iter_mut() {
            let osc_out = osc.advance();
            let env_out = env.advance();
            *sample = osc_out * env_out;
        }
    }
}
```

---

## Sound Design Tips

### Classic Subtractive Bass

```rust
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_osc2_waveform(OscillatorWaveform::Square);
synth.set_osc2_detune(5.0);
synth.set_filter_cutoff(500.0);
synth.set_filter_resonance(4.0);
synth.set_filter_env_amount(2000.0);
synth.set_amp_attack(5.0);
synth.set_amp_release(100.0);
synth.set_filter_decay(150.0);
synth.set_filter_sustain(0.2);
```

### Warm Pad

```rust
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_osc2_waveform(OscillatorWaveform::Saw);
synth.set_osc2_detune(12.0);
synth.set_osc_mix(0.5);
synth.set_filter_cutoff(2000.0);
synth.set_filter_resonance(1.5);
synth.set_amp_attack(200.0);
synth.set_amp_decay(500.0);
synth.set_amp_sustain(0.8);
synth.set_amp_release(2000.0);
synth.set_lfo1_rate(0.3);
synth.set_lfo1_to_filter(300.0);
```

### Pluck / Key

```rust
synth.set_osc1_waveform(OscillatorWaveform::Saw);
synth.set_filter_cutoff(4000.0);
synth.set_filter_resonance(2.0);
synth.set_filter_env_amount(6000.0);
synth.set_amp_attack(1.0);
synth.set_amp_decay(300.0);
synth.set_amp_sustain(0.0);
synth.set_filter_attack(1.0);
synth.set_filter_decay(200.0);
synth.set_filter_sustain(0.0);
```

---

## API Reference

For complete API documentation, see the rustdoc:

```bash
cargo doc -p sonido-synth --open
```

### Key Files

| Component | Location |
|-----------|----------|
| Oscillator | `crates/sonido-synth/src/oscillator.rs` |
| ADSR Envelope | `crates/sonido-synth/src/envelope.rs` |
| Voice/VoiceManager | `crates/sonido-synth/src/voice.rs` |
| Modulation Matrix | `crates/sonido-synth/src/mod_matrix.rs` |
| Audio Modulation | `crates/sonido-synth/src/audio_mod.rs` |
| MonophonicSynth | `crates/sonido-synth/src/synth.rs` |
| PolyphonicSynth | `crates/sonido-synth/src/synth.rs` |
| Tempo System | `crates/sonido-core/src/tempo.rs` |
