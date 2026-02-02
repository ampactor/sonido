# Sonido Dream Dev Box: Universal Biosignal + Audio DSP Platform

## The Vision
A modular hardware platform for:
- EEG / neural signals (µV, high-impedance, isolated)
- Electric fish (mV, kHz bandwidth, multi-electrode)
- Slime mold / plants (DC-coupled, ultra-slow oscillations)
- Audio effects / synthesis (instrument + line level)
- Cross-Frequency Coupling analysis (NOW model theta-gamma nesting)
- Bidirectional: biosignal → DSP → audio/synthesis

---

## Path 1: Quick Start (~$300-500)

Get running fast with off-the-shelf modules. Proves the concept before custom boards.

### Core Processing
| Component | Price | Notes |
|-----------|-------|-------|
| **Daisy Seed** | $30 | You know it, proven audio, 480MHz Cortex-M7 |
| OR **Teensy 4.1** | $32 | More I/O, built-in SD, 600MHz, better for data logging |

### Biosignal Acquisition (modular)

**Option A: OpenBCI Cyton** (~$250)
- 8 channels, 24-bit ADS1299
- SPI interface, well-documented
- Designed for EEG, works for other biosignals
- Can interface with Daisy/Teensy via SPI

**Option B: DIY ADS1299 Breakout** (~$80)
- ProtoCentral ADS1299 breakout: $65
- Add isolation: ADUM4160 + isolated DC-DC: $15
- Requires more integration work but cheaper

**Option C: For Slime Mold Specifically**
- INA128 instrumentation amp breakout: $15
- ADS1115 16-bit ADC module: $10
- This is DC-coupled and slower, perfect for Physarum

### Audio I/O
- Daisy Seed has built-in codec (good enough)
- Or add PCM5102A DAC module for better output: $8

**For hot signals (Lionel's requirement):**
- DIY resistor divider pad: $2 (4x 10k resistors, simple voltage divider)
- Or: Radial ProAV passive DI (reverse as attenuator): ~$100
- Or: Behringer HD400 Hum Destroyer (has -20dB pad): $25
- The input protection is important - Daisy's codec maxes at ~3.3V p-p, Eurorack is ±10V

### Wiring It Up
```
[Electrodes] → [ADS1299/INA128] → SPI/I2C → [Daisy/Teensy] → USB → [Computer]
                                              ↓
                                        [Audio Out]
```

### Software Path
Your sonido crates already have the DSP. Add:
1. `sonido-platform` crate for embedded HAL abstraction
2. ADS1299 driver (SPI protocol is documented)
3. USB streaming for data to computer
4. Real-time CFC analysis in `sonido-analysis`

---

## Path 2: Dream Platform (Custom Modular Design)

A professional-grade, expandable system. Estimate $800-2000 depending on options.

### Architecture: Stackable Modules

```
┌─────────────────────────────────────────────────┐
│              EXPANSION MODULES                   │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐        │
│  │ Biosig   │ │ Audio    │ │ CV/Mod   │  ...   │
│  │ 8-ch EEG │ │ Hi-Z In  │ │ Eurorack │        │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘        │
│       │            │            │               │
│  ─────┴────────────┴────────────┴───────────── │
│              EXPANSION BUS (SPI/I2S)            │
├─────────────────────────────────────────────────┤
│                 BASE MODULE                      │
│  ┌─────────────────────────────────────────┐   │
│  │  STM32H750 (Daisy-compatible)           │   │
│  │  • 480MHz Cortex-M7 + FPU               │   │
│  │  • 64MB SDRAM, 8MB Flash                │   │
│  │  • Audio codec (stereo in/out)          │   │
│  │  • USB-C (power + data)                 │   │
│  │  • microSD slot                         │   │
│  │  • OLED header (128x64 or 128x32)       │   │
│  │  • 6x knobs, 3x toggles (Hothouse compat)│   │
│  │  • Expansion connector (40-pin)         │   │
│  └─────────────────────────────────────────┘   │
└─────────────────────────────────────────────────┘
```

### Module Specifications

#### Base Module
- **MCU**: STM32H750VBT6 (same as Daisy Seed)
- **Audio**: WM8731 or PCM3060 codec, 24-bit 96kHz
- **Storage**: microSD via SDIO, 8MB QSPI flash
- **Display**: I2C OLED header, optional SPI TFT
- **USB**: USB-C with USB 2.0 HS (480Mbps)
- **Power**: 5V USB or 7-15V barrel jack, onboard regulators
- **Controls**: Hothouse-compatible 6 knobs + 3 toggles (optional, breakout headers if not populated)
- **Expansion**: 40-pin header with SPI, I2S, I2C, UART, ADC, GPIO

#### Biosignal Module (EEG/EMG/ECG/Fish/Mold)
- **AFE**: ADS1299-8 (8 channels, 24-bit, 250-16kSPS)
- **Inputs**: 8 differential channels, 2.4µV resolution
- **Gain**: Programmable 1-24x per channel
- **Filters**: Per-channel configurable HP (DC to 0.5Hz for slime mold mode)
- **Isolation**: Digital isolator (Si8641) + isolated DC-DC (SN6505A)
- **Connectors**: Touch-proof electrodes (DIN 42802) for human use
- **Safety**: Meets IEC 60601-1 isolation requirements
- **Interface**: SPI to base module

Why ADS1299:
- Industry standard for EEG (OpenBCI, Muse internals, research systems)
- Built-in bias drive, lead-off detection
- 8 channels is good balance (expandable by stacking)
- Works for fish/mold with gain adjustment

#### Audio Input Module (Lionel-Approved Hot Signal Handling)
- **Inputs**: 2x 1/4" TRS (stereo or dual mono)
- **Impedance**: Switchable 1MΩ (guitar) / 10kΩ (line) / 1kΩ (Eurorack)
- **Input Range**: -20dBV to +18dBV (handles everything from weak pickups to Eurorack ±10V)
- **Gain Stages**:
  - **Stage 1**: Resistive pad (switchable -20dB for Eurorack/hot synths)
  - **Stage 2**: Variable gain preamp (-10dB to +40dB)
  - **Stage 3**: Soft-clip protection before ADC (no hard clipping ever)
- **Headroom**: 24dB above nominal, rails at ±15V
- **Features**:
  - Per-channel gain knobs with LED clip indicators
  - Soft-clipping circuit (symmetrical diode limiting) - breaks up musically, not digitally
  - Optional transformer input for galvanic isolation
  - Phantom power option for condenser mics
- **ADC**: Uses base module codec or dedicated AK5572 (120dB SNR)
- **Interface**: I2S to base module

**Why this matters**: The Hothouse clips hard on anything over ~1.5V p-p. Synths and Eurorack output 10V p-p. This design handles it clean, OR you can engage soft-clip for musical saturation.

#### CV/Modular Module
- **CV In**: 4x bipolar ±10V, 16-bit ADC
- **CV Out**: 4x 0-10V or ±5V, 16-bit DAC
- **Gate**: 4x in, 4x out (5V logic)
- **Power**: Eurorack power header option
- **Interface**: SPI/I2C to base module

### PCB Design Considerations

**Form Factor Options:**
1. **Eurorack**: 3U height, 20HP base + 8HP per expansion
2. **Desktop**: 125B-sized base (same as Hothouse)
3. **Stacked**: Credit-card modules with standoffs

**For a welder learning to solder:**
- 0805 passives (resistors, caps) - easy to hand solder
- TQFP for MCU (harder but doable with practice)
- Critical chips (ADS1299) in TQFP-64, requires good flux and patience
- Consider ordering base module as PCBA (assembled), hand-solder simpler expansion modules

### Form Factor: Field-Rugged Hackbox

**Requirements**: Field-portable, sits nice on desk, protects sensitive parts, easy to access for hacking.

**Recommended: Hammond 1455 Series Extruded Aluminum**
- Model: 1455N1601 (160 x 103 x 53mm) or 1455N2201 (220mm longer)
- Aluminum body = RF shielding, durable, looks professional
- Slide-off end panels = easy access to boards
- Slot for PCB mounting = boards slide in like cards
- Flat top and bottom = stable on desk
- Optional: Add rubber feet, carry handle
- Price: ~$30-40

**Alternative: Pelican 1060 Micro Case + Custom Panel**
- Waterproof, crushproof, field-proven
- Cut custom acrylic top panel for connectors
- Foam interior holds boards secure
- Ultimate field durability
- Price: ~$25 + panel work

**Hackability Features:**
- Expansion headers exposed on one end panel
- JTAG/SWD debug port accessible
- USB-C on front, electrode jacks on back
- Removable lid with captive thumb screws (not tiny Phillips)
- Internal standoffs for stacking expansion modules
- Consider pogo-pin connectors for quick module swaps

**3D Printed Option:**
- Design parametric enclosure in OpenSCAD/FreeCAD
- PETG or ASA for durability
- Integrated mounting posts, cable routing
- Can iterate design as you hack
- Print locally or use JLCPCB 3D printing service

### BOM Estimate (Dream Platform)

| Module | Key Components | Est. Cost |
|--------|---------------|-----------|
| Base Module PCB+assembly | STM32H750, codec, power, SD | $150-250 |
| Biosignal Module | ADS1299, isolation, connectors | $100-150 |
| Audio Input Module | Preamps, ADC, jacks | $50-80 |
| CV Module | ADC, DAC, protection | $60-100 |
| Enclosure, cables, electrodes | | $100-200 |
| **Total Dream Setup** | | **$500-800** |

---

## Software Architecture (sonido-platform crate)

```
crates/sonido-platform/
├── src/
│   ├── lib.rs
│   ├── hal.rs          # Hardware abstraction traits
│   ├── daisy.rs        # Daisy Seed implementation
│   ├── dreambox.rs     # Dream platform implementation
│   ├── ads1299.rs      # Biosignal ADC driver
│   ├── controls.rs     # Knobs, toggles, footswitches
│   └── display.rs      # OLED/LCD abstraction
```

### Key Traits

```rust
pub trait Platform {
    type AudioIn: AudioInput;
    type AudioOut: AudioOutput;
    type Biosignal: BiosignalInput;
    type Controls: ControlSurface;

    fn init() -> Self;
    fn process_audio(&mut self, callback: impl FnMut(&[f32], &mut [f32]));
    fn read_biosignal(&mut self) -> BiosignalFrame;
}

pub trait BiosignalInput {
    fn channels(&self) -> usize;
    fn sample_rate(&self) -> u32;
    fn read_frame(&mut self) -> &[i32];  // 24-bit samples
    fn set_gain(&mut self, channel: usize, gain: Gain);
    fn set_highpass(&mut self, channel: usize, freq: HighpassFreq);
}
```

---

## Cross-Frequency Coupling Analysis

For the NOW model (Riddle/Schooler), you need:

### Theta-Gamma Coupling
1. **Bandpass filter** theta (4-8 Hz) and gamma (30-100 Hz)
2. **Hilbert transform** to extract instantaneous phase (theta) and amplitude (gamma)
3. **Phase-amplitude coupling metrics**: Modulation Index, Mean Vector Length
4. **Sliding window** for real-time tracking

### sonido-analysis Extensions Needed

```rust
// New module: crates/sonido-analysis/src/cfc.rs

pub struct CrossFrequencyCoupling {
    theta_filter: BandpassFilter,    // 4-8 Hz
    gamma_filter: BandpassFilter,    // 30-100 Hz
    hilbert: HilbertTransform,
    phase_bins: usize,               // typically 18 (20° bins)
    window_samples: usize,
}

impl CrossFrequencyCoupling {
    /// Returns phase-amplitude coupling strength (0-1)
    pub fn modulation_index(&self, signal: &[f32]) -> f32;

    /// Returns coupling over time for visualization
    pub fn coupling_timeseries(&self, signal: &[f32]) -> Vec<f32>;

    /// Phase-amplitude histogram for comodulogram
    pub fn phase_amplitude_histogram(&self, signal: &[f32]) -> [[f32; N_PHASE_BINS]; N_FREQ_BINS];
}
```

---

## Recommended Next Steps (Dream Platform Direct)

### Phase 1: Design (Weeks 1-2)
1. [ ] Install KiCad 8, create project structure
2. [ ] Base module schematic: STM32H750, power, USB-C, SD, audio codec
3. [ ] Biosignal module schematic: ADS1299, isolation, electrode connectors
4. [ ] Audio input schematic: preamp with hot signal handling (Lionel spec)
5. [ ] Review schematics together, catch errors before layout

### Phase 2: Layout & Fab (Weeks 3-4)
1. [ ] PCB layout: 4-layer for base (signal integrity), 2-layer for simpler modules
2. [ ] Design Hammond 1455 end panels (DXF for laser cutting)
3. [ ] Generate gerbers, BOM, pick-and-place files
4. [ ] Order PCBs from JLCPCB (get PCBA for base module, bare boards for expansion)
5. [ ] Order components from DigiKey/Mouser (ADS1299, STM32, passives)
6. [ ] Order enclosure, electrodes, cables

### Phase 3: Assembly (Weeks 5-6)
1. [ ] Practice soldering: get some scrap boards, solder 0805 resistors, SOIC chips
2. [ ] Assemble expansion modules (hand solder)
3. [ ] Test continuity, power rails before powering on
4. [ ] Flash bootloader to STM32 via SWD
5. [ ] Bring up audio path first (you know this works from Hothouse)
6. [ ] Then bring up ADS1299 SPI

### Phase 4: Software (Parallel with hardware)
1. [ ] Create `sonido-platform` crate structure
2. [ ] Port existing Daisy code to new platform HAL
3. [ ] Write ADS1299 driver (SPI, configuration, data streaming)
4. [ ] Add Hilbert transform to `sonido-analysis`
5. [ ] Implement CFC metrics (Modulation Index, phase-amplitude coupling)
6. [ ] Build egui visualization for real-time coupling display

### Phase 5: Integration & Testing
1. [ ] End-to-end test: electrodes → ADS1299 → DSP → audio out
2. [ ] Validate CFC analysis with synthetic test signals
3. [ ] First real experiment: your own EEG, look for theta-gamma during meditation
4. [ ] Field test: take it somewhere, record something weird

---

## Signal-Specific Notes

### Electric Fish (Apteronotus, Eigenmannia, Gymnotus)
- **Wave-type fish** (Apteronotus, Eigenmannia): Pure sine-like EOD, 100Hz-2kHz
- **Pulse-type fish** (Gymnotus): Brief pulses with harmonics, need ~50kHz sample rate
- **Multi-electrode**: Dipole source localization needs 4+ electrodes
- **Tank setup**: Ag/AgCl electrodes in water, differential recording
- **ADS1299 works**: Just run at 16kSPS, HP filter at 1Hz

### Slime Mold (Physarum polycephalum)
- **Oscillation period**: 60-120 seconds (0.008-0.016 Hz!)
- **Signal**: Extracellular potential, ~100µV to few mV
- **Electrodes**: Non-polarizable (Ag/AgCl) on agar substrate
- **Critical**: DC-coupled input! Most biosignal amps have HP filter at 0.1Hz minimum
- **ADS1299**: Can set HP to 0.016Hz or bypass entirely in DC mode
- **Suggestion**: Also monitor impedance (slime mold grows/moves)

### EEG for CFC/NOW Model
- **Channels needed**: Minimum 1 (Fz/Pz for basic theta-gamma), ideally 4-8 (Fz, Cz, Pz, O1, O2, T3, T4)
- **Frontal theta**: 4-8Hz, prominent during attention/meditation
- **Parietal gamma**: 30-100Hz, associated with conscious moments in NOW model
- **Reference**: Linked ears or Cz reference
- **Ground**: Forehead (Fpz)
- **Electrodes**: Dry electrodes OK for dev (worse signal but no gel), wet Ag/AgCl for quality

## Open Questions

1. **Channel count**: 8 channels enough to start? (EEG typically 16-64 for research, but 8 is fine for frontal theta-gamma)
2. **Wireless**: Add ESP32 co-processor for BLE streaming to phone/tablet?
3. **Display**: OLED (128x64, simple), larger TFT (320x240, better visualization), or just USB to computer?
4. **Power**: USB-C only, or also battery option for true field portability?

---

## Resources

- ADS1299 datasheet: TI SBAS499
- OpenBCI hardware designs (open source): github.com/OpenBCI
- Daisy Seed pinout/schematic: electro-smith.com
- Phase-amplitude coupling: Tort et al. 2010 "Measuring phase-amplitude coupling"
- NOW model: Riddle & Schooler papers on theta-gamma nesting

---

## Files to Create/Modify

| File | Action | Purpose |
|------|--------|---------|
| `crates/sonido-platform/` | Create | New crate for hardware abstraction |
| `crates/sonido-platform/src/ads1299.rs` | Create | ADS1299 driver |
| `crates/sonido-analysis/src/cfc.rs` | Create | Cross-frequency coupling analysis |
| `crates/sonido-analysis/src/hilbert.rs` | Create | Hilbert transform |
| `docs/HARDWARE.md` | Update | Add dream platform docs |
| `docs/BIOSIGNAL.md` | Create | Biosignal acquisition guide |

## Verification

1. **ADS1299 driver**: Loopback test with known signal, verify sample rate and bit depth
2. **CFC analysis**: Test with synthetic coupled signal (theta FM of gamma AM)
3. **End-to-end**: Record 1 minute of EEG, visualize theta-gamma coupling over time
4. **Latency**: Measure round-trip latency for audio feedback (target <10ms)
