# DSP Platform Reference

## Document Purpose

This document provides context for Claude Code sessions working on the Rust DSP library. It defines hardware targets, use cases, architectural constraints, and development roadmap. All code decisions should align with this context.

**Key Insight**: This is not just a guitar pedal project. The DSP library serves three interconnected use cases—multi-effect pedal, synthesizer, and biosignal research platform—unified by a common architectural core.

---

## Use Cases

### 1. Multi-Effect Pedal

Standard guitar/instrument processor with chainable effects.

**Requirements**:
- Stereo audio I/O (4-channel eventually)
- 6+ knobs, encoder, footswitches
- OLED/TFT for menu navigation and parameter display
- MIDI I/O for external control
- Latency <5ms round-trip
- Preset save/load

**DSP Components**: Filters (LP/HP/BP/notch, parametric EQ), delays (simple, multi-tap, ping-pong), reverbs (algorithmic), modulation (chorus, flanger, phaser, tremolo), dynamics (compressor, limiter, envelope follower), distortion/overdrive, pitch shifting.

### 2. Synthesizer

Sound generator where audio inputs become modulation sources.

**Requirements**:
- Audio input → parameter mapping (envelope follower → filter cutoff, pitch tracker → oscillator freq)
- Multiple oscillator types (analog-style, wavetable, FM)
- Filter with multiple modes
- Modulation matrix (LFOs, envelopes, input-derived signals)
- MIDI for note/CC
- Ideally: multiple inputs for different modulation sources

**Key Concept**: Input-to-parameter mapping. The pedal's audio input becomes a control signal. Envelope follower extracts amplitude, pitch tracker extracts frequency, spectral analysis extracts timbral features—all become modulation sources.

### 3. Biosignal Research Platform

Scientific instrument for measuring cross-frequency coupling (CFC) in biological systems.

**Requirements**:
- DC-coupled inputs (0 Hz capable) for slow biosignals
- Multiple electrode channels (4-12)
- AC-coupled audio inputs for faster signals
- Real-time spectral analysis
- Phase-amplitude coupling computation
- Sonification output
- Long-duration recording (SD card)

**Target Organisms**:
| Organism | Frequency Range | Recording Method |
|----------|-----------------|------------------|
| Physarum polycephalum | 0.001 - 0.1 Hz | Surface electrodes, DC-coupled |
| Electric fish (Gnathonemus) | 50 - 600 Hz | Water electrodes, AC audio OK |

**Scientific Context**: CFC describes how slow oscillation phase modulates fast oscillation amplitude. Central to theories of hierarchical consciousness (Riddle's Nested Observer Windows model). Research question: Does CFC appear in non-neural biological systems?

---

## Hardware Strategy

### Phase 1: bkshepherd 125B (Current Target)

**Purpose**: Get DSP library on real hardware fast. Validate architecture. Ship working effects.

**Timeline**: Now → 3 months

**Repo**: https://github.com/bkshepherd/DaisySeedProjects

#### Specifications

| Component | Details |
|-----------|---------|
| MCU | STM32H750 (Cortex-M7 @ 480MHz) via Daisy Seed |
| Memory | 64MB SDRAM, 8MB flash |
| Audio | Stereo in/out (WM8731 codec, 24-bit, 48kHz) |
| Knobs | 6x potentiometers (ADC) |
| Encoder | Rotary with push button |
| Display | 128x64 OLED (SSD1306, I2C) |
| Footswitches | 2x momentary |
| LEDs | 2x |
| MIDI | In + Out (TRS) |
| Bypass | Relay-based true bypass |
| Power | 9V center-negative |

#### Critical I/O Details

**TRS Stereo Jacks**: The 125B has 2 physical jacks, not 4:
- 1x TRS input: Tip = Left, Ring = Right
- 1x TRS output: Tip = Left, Ring = Right

Standard TS (mono) guitar cable connects only to Tip (Left channel). For true stereo, need TRS cables or Y-adapters.

**AC Coupling**: Audio path has capacitors blocking DC and infraslow frequencies:

| Frequency | Signal Passed |
|-----------|---------------|
| 20 Hz | ~70% |
| 5 Hz | ~20% |
| 1 Hz | ~5% |
| 0.1 Hz | <1% |

**Implication**: Physarum oscillations (0.01-0.1 Hz) cannot pass through the audio codec. Biosignal work requires separate DC-coupled ADC.

#### Build Cost

| Item | Source | Cost |
|------|--------|------|
| JLCPCB PCB + SMD assembly | JLCPCB | ~$40 |
| Through-hole parts | Tayda | ~$18 |
| Enclosure (drilled, coated) | Tayda | ~$12 |
| Daisy Seed 65MB | Electrosmith | $29 |
| ST-LINK V2 clone | Amazon | ~$12 |
| **Total** | | **~$111** |

#### Use Case Coverage

- Multi-FX pedal: ✓ Full support
- Synthesizer: ✓ Basic (single stereo input for modulation)
- Biosignal: ✗ Requires external ADC hack

### Phase 1.5: Biosignal Expansion (Optional)

Add DC-coupled inputs via I2C ADC for early biosignal experiments:

```
I2C Bus (shared with OLED)
    ├── OLED (0x3C)
    └── ADS1115 (0x48)
            ├── Ch0: Physarum electrode A
            ├── Ch1: Physarum electrode B
            ├── Ch2: Spare
            └── Ch3: Spare
```

**ADS1115 specs**: 16-bit, DC-coupled, 860 SPS max, ~$5/module, stack up to 4 on one bus (16 channels).

**Cost**: +$15 for 2x ADS1115 + instrumentation amp (INA128) + passives.

This is a hack, not a proper solution—but sufficient for proof-of-concept CFC measurements while Phase 2 hardware is designed.

### Phase 2: Custom Patch SM Carrier (Future)

**Purpose**: Purpose-built platform with full I/O for all use cases.

**Timeline**: Design after Phase 1 learnings (3-6 months out)

**Core Module**: Daisy Patch SM (~$69)
- Same STM32H750 as Daisy Seed
- 12x 16-bit ADC inputs (DC-coupled, bipolar CV ready)
- 2x 12-bit DAC outputs
- 2x gate in, 2x gate out
- 12x GPIO

**Target Specifications**:

| Feature | Spec | Rationale |
|---------|------|-----------|
| Audio In | 4 channels | Dual stereo OR stereo + modulation sources |
| Audio Out | 4 channels | Stereo wet + dry, or dual stereo |
| Jack Config | TRS stereo OR dual TS mono | Studio-standard flexibility |
| DC ADC | 8+ channels | Biosignal electrodes, CV input |
| CV/Gate Out | 2-4 channels | Stimulation, external gear |
| Display | 2.4"+ TFT (SPI) | Rich UI, waveforms, CFC matrices |
| MIDI | In + Out + Thru | Full integration |
| SD Card | Yes | Presets, IRs, data recording |

**Key Addition**: External codec (PCM3168A: 6-in/8-out, ~$15) for 4x4 audio. Daisy Seed's built-in codec is stereo only.

**Estimated BOM**: $150-200

**Use Case Coverage**:
- Multi-FX pedal: ✓ Full support with 4x4 audio
- Synthesizer: ✓ Full support with multiple inputs
- Biosignal: ✓ Full support with DC-coupled ADCs

---

## Architecture

### Portability Requirements

The DSP core must run on:
- Phase 1 hardware (125B: 2-in/2-out, 48kHz, 128x64 OLED)
- Phase 2 hardware (Custom: 4-in/4-out, 96kHz possible, TFT)
- Desktop (development, testing)
- Future unknown platforms

**Consequence**: No hardware assumptions in DSP algorithms. All platform specifics behind traits.

### Layer Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                    Application Layer                            │
│         (Effect chains, synth patches, presets, UI)            │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                     DSP Core (portable)                         │
│                                                                 │
│   Processors:  Filters, delays, reverbs, modulation, dynamics  │
│   Analyzers:   FFT, envelope follower, pitch tracker, CFC      │
│   Modulators:  LFO, ADSR, input-derived                        │
│   Utilities:   Interpolation, windowing, coefficients          │
│                                                                 │
│   Constraints: no_std, no heap in audio path, deterministic    │
│                                                                 │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│                  Platform Abstraction Layer                     │
│                                                                 │
│   trait AudioIO { fn process(...); }                           │
│   trait ParamSource { fn read(&self, id) -> f32; }             │
│   trait Display { fn draw(...); }                              │
│   trait Storage { fn load/save(...); }                         │
│                                                                 │
└─────────────────────────────┬───────────────────────────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          │                   │                   │
┌─────────▼─────────┐ ┌───────▼───────┐ ┌────────▼────────┐
│   125B HAL        │ │ Patch SM HAL  │ │  Desktop (CPAL) │
└───────────────────┘ └───────────────┘ └─────────────────┘
```

### Core Traits

```rust
/// Audio processor that transforms samples
pub trait Processor: Send {
    /// Process audio block. Channels interleaved: [L0, R0, L1, R1, ...]
    fn process(&mut self, input: &[f32], output: &mut [f32]);
    
    /// Set parameter value (0.0-1.0 normalized)
    fn set_param(&mut self, id: ParamId, value: f32);
    
    /// Parameter metadata for UI
    fn params(&self) -> &[ParamInfo];
    
    /// Reset internal state
    fn reset(&mut self);
}

/// Feature extractor (doesn't modify audio)
pub trait Analyzer: Send {
    type Output;
    fn analyze(&mut self, input: &[f32]) -> Self::Output;
    fn reset(&mut self);
}

/// Modulation source
pub trait Modulator: Send {
    fn next(&mut self) -> f32;
    fn set_rate(&mut self, hz: f32);
    fn trigger(&mut self);
    fn reset(&mut self);
}
```

### Effect Chaining

```rust
pub struct Chain {
    processors: Vec<Box<dyn Processor>>,
    buffer: Vec<f32>,
}

impl Processor for Chain {
    fn process(&mut self, input: &[f32], output: &mut [f32]) {
        if self.processors.is_empty() {
            output.copy_from_slice(input);
            return;
        }
        
        self.processors[0].process(input, &mut self.buffer);
        for proc in &mut self.processors[1..] {
            let tmp = self.buffer.clone(); // TODO: double buffer
            proc.process(&tmp, &mut self.buffer);
        }
        output.copy_from_slice(&self.buffer);
    }
    // ...
}
```

### Input → Parameter Mapping (Synth Mode)

```rust
pub enum MappingSource {
    Knob(u8),
    MidiCC(u8),
    EnvelopeFollower,
    PitchTracker,
    SpectralCentroid,
    LFO(u8),
    ExternalCV(u8),  // Phase 2 only
}

pub struct ParamMapping {
    source: MappingSource,
    target: ParamId,
    amount: f32,      // Modulation depth
    offset: f32,      // Base value
}

pub struct MappingEngine {
    mappings: Vec<ParamMapping>,
    envelope_follower: EnvelopeFollower,
    pitch_tracker: PitchTracker,
    lfos: [LFO; 4],
}
```

### CFC Analysis (Biosignal)

```rust
/// Phase-Amplitude Coupling analyzer
pub struct PACAnalyzer {
    phase_filter: Bandpass,      // Extract slow rhythm
    amplitude_filter: Bandpass,  // Extract fast rhythm
    hilbert: HilbertTransform,   // Phase/envelope extraction
    phase_bins: [f32; 18],       // 20° bins for MI calculation
}

impl PACAnalyzer {
    /// Compute modulation index (0 = no coupling, 1 = perfect)
    pub fn modulation_index(&mut self, signal: &[f32]) -> f32 {
        // 1. Bandpass for slow rhythm → extract phase via Hilbert
        // 2. Bandpass for fast rhythm → extract envelope via Hilbert
        // 3. Bin fast amplitudes by slow phase
        // 4. MI = (H_max - H_observed) / H_max
        todo!()
    }
}
```

---

## Development Constraints

### Real-Time Safety

The audio callback must be deterministic:
- No heap allocation
- No blocking operations
- No unbounded loops
- Predictable worst-case execution time

```rust
// BAD: allocation in audio path
fn process(&mut self, input: &[f32], output: &mut [f32]) {
    let buffer = vec![0.0; input.len()]; // HEAP ALLOCATION
}

// GOOD: pre-allocated buffers
fn process(&mut self, input: &[f32], output: &mut [f32]) {
    // self.buffer allocated at construction time
    self.buffer[..input.len()].copy_from_slice(input);
}
```

### Memory Model

- **SDRAM (64MB)**: Large buffers—delay lines, reverb networks, IR convolution, recording buffers
- **SRAM (512KB)**: Hot data—filter states, current parameters, small buffers
- **Flash (8MB)**: Presets, wavetables, IRs

Delay lines and reverbs can be large. A 2-second stereo delay at 48kHz = 384KB. Plan accordingly.

### Parameter Smoothing

Raw ADC values jitter. Parameters need smoothing:

```rust
pub struct SmoothedParam {
    current: f32,
    target: f32,
    coeff: f32,  // Smoothing coefficient (0.99 = slow, 0.9 = fast)
}

impl SmoothedParam {
    pub fn set(&mut self, value: f32) {
        self.target = value;
    }
    
    pub fn next(&mut self) -> f32 {
        self.current = self.current * self.coeff + self.target * (1.0 - self.coeff);
        self.current
    }
}
```

Apply per-sample in audio callback, or per-block with interpolation.

---

## 3-Month Development Timeline

**Goal**: Working multi-FX pedal demonstrating DSP library capabilities, ready to show collaborator.

### Month 1: Hardware + Foundation

| Week | Deliverable |
|------|-------------|
| 1 | Order parts (JLCPCB, Tayda, Electrosmith). Start physarum culture. |
| 2 | Set up Rust toolchain for `thumbv7em-none-eabihf`. Write DSP core abstractions testable on desktop. |
| 3 | Assemble 125B. Flash test firmware. Verify audio passthrough. |
| 4 | Port first effect (delay or filter). Debug real-time issues. |

### Month 2: DSP Library + UI

| Week | Deliverable |
|------|-------------|
| 5 | Implement 3-4 core effects (reverb, delay, filter, drive). |
| 6 | Menu system on OLED. Effect selection, parameter pages. |
| 7 | Effect chaining. Preset save/load. |
| 8 | Synth mode: oscillator + input→param mapping. |

### Month 3: Polish + Demo

| Week | Deliverable |
|------|-------------|
| 9 | UI polish. Smooth transitions. Visual feedback. |
| 10 | Add 2-3 more effects. Performance presets. |
| 11 | (If time) Basic physarum recording. Sonification proof-of-concept. |
| 12 | Demo prep. Documentation. |

### Minimum Viable Demo

If blockers arise, the absolute minimum for credibility:
- [ ] 125B hardware working
- [ ] 2-3 effects with menu selection
- [ ] Knob control of parameters
- [ ] Clean audio quality

---

## Open Questions

### Architecture

1. **Block size**: Fixed 64? Configurable? How does this affect latency claims?
2. **Channel count**: Should `Processor::process()` assume stereo, or be generic over N channels?
3. **Sample rate independence**: Bake in 48kHz? Or pass rate at construction and recalculate coefficients?
4. **Parameter IDs**: Enum per effect? Global registry? How do chains reference child params?

### Hardware

5. **Expression pedal**: Worth adding to 125B via spare ADC?
6. **USB audio**: Possible on Daisy but complex. Worth the effort?
7. **Phase 2 codec**: PCM3168A vs alternatives? I2S configuration complexity?

### Biosignal

8. **Real-time CFC**: Feasible at audio rates, or only on downsampled data?
9. **Sonification latency**: How much delay acceptable for biosignal→sound to feel "live"?

---

## Resources

### Hardware
- **bkshepherd 125B**: https://github.com/bkshepherd/DaisySeedProjects
- **Daisy Seed**: https://daisy.audio/hardware/Seed/
- **Patch SM**: https://daisy.audio/hardware/PatchSM/
- **Rust daisy crate**: https://crates.io/crates/daisy

### Tooling
- **probe-rs**: https://probe.rs
- **defmt**: https://defmt.ferrous-systems.com
- **Embassy**: https://embassy.dev

### Ordering
- **JLCPCB**: https://jlcpcb.com
- **Tayda**: https://taydaelectronics.com
- **Tayda drill tool**: https://drill.taydakits.com

### Research
- **Riddle NOW model**: "Hierarchical consciousness: the Nested Observer Windows model" (2024), Neuroscience of Consciousness
- **Riddle Lab**: https://www.theriddlelab.org

---

## Collaboration Context

**Lionel Williams (Vinyl Williams)**: LA-based multimedia artist, has Meow Wolf installation. Visiting Salt Lake City for Kilby Court show (date TBD). Potential collaborator on interactive biosignal installation. Strategy: Have working pedal + physarum demo ready for in-person conversation.

**Justin Riddle**: NOW model author, FSU Psychology. Contact after preliminary CFC data from physarum. Lead with data, not pitch.

**DigiTech**: Local target (Murray, UT) for potential employment. 125B pedal demonstrates relevant embedded DSP skills.

---

*This document evolves. Update as decisions are made and context shifts.*
