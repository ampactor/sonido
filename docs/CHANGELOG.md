# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

#### sonido-synth (NEW CRATE)

Full synthesis engine for building synthesizers with `no_std` support.

**Oscillators:**
- `Oscillator` with PolyBLEP anti-aliasing for alias-free waveforms
- `OscillatorWaveform`: Sine, Triangle, Saw, Square, Pulse (with width), Noise
- Frequency and phase modulation support

**Envelopes:**
- `AdsrEnvelope` with Attack, Decay, Sustain, Release stages
- Configurable curves and times in milliseconds
- Gate-based triggering

**Voice Management:**
- `Voice` combining oscillators, filter, and envelopes
- `VoiceManager` for polyphonic voice allocation
- `VoiceAllocationMode`: Oldest, Newest, Quietest, HighestNote, LowestNote stealing strategies

**Modulation:**
- `ModulationMatrix` for flexible modulation routing
- `ModulationRoute` with depth, curve, and inversion
- `ModSourceId` and `ModDestination` for source/dest identification
- `AudioModSource` to use audio input as modulation
- `AudioGate` to convert audio amplitude to gate signals

**Complete Synths:**
- `MonophonicSynth` with portamento/glide
- `PolyphonicSynth<N>` with configurable voice count
- Dual oscillators with detune, filter with envelope, amp envelope

#### sonido-config (NEW CRATE)

CLI-first configuration and preset management.

- `Preset` struct for loading/saving effect chain presets from TOML
- `EffectConfig` for individual effect configuration with parameters
- `EffectChain` runtime builder for effect chains
- Parameter validation against registry schemas
- Platform-specific paths (`user_presets_dir()`, `system_presets_dir()`)
- Factory presets bundled with the library
- `find_preset()` for searching preset directories

#### sonido-core

**Tempo System:**
- `TempoManager` for tempo tracking and musical timing
- `NoteDivision` enum: Whole, Half, Quarter, Eighth, Sixteenth, ThirtySecond
- Dotted notes: DottedHalf, DottedQuarter, DottedEighth
- Triplets: TripletQuarter, TripletEighth, TripletSixteenth
- `division_to_hz()`, `division_to_ms()`, `division_to_samples()` conversions
- `TransportState` (Playing, Stopped) with position tracking
- Beat/bar position and phase methods

**Modulation System:**
- `ModulationSource` trait for unified modulation interface
- Implemented for `Lfo` (bipolar) and `EnvelopeFollower` (unipolar)
- `mod_advance()`, `mod_value()`, `mod_reset()` methods
- `mod_advance_unipolar()` and `mod_advance_bipolar()` conversions
- `ModulationAmount` struct for depth and inversion control

#### sonido-analysis

**Cross-Frequency Coupling (CFC) Analysis:**
- `FilterBank` for multi-band frequency extraction with 4th-order Butterworth filters
- `FrequencyBand` specification with center frequency and bandwidth
- `eeg_bands` module with standard EEG bands (Delta, Theta, Alpha, Beta, Low Gamma, High Gamma)
- `HilbertTransform` using FFT method for analytic signal computation
- Instantaneous phase, amplitude, and frequency extraction
- Phase unwrapping and frequency estimation

**Phase-Amplitude Coupling:**
- `PacAnalyzer` for analyzing coupling between frequency bands
- `PacMethod`: Mean Vector Length (Canolty et al., 2006) and Kullback-Leibler (Tort et al., 2010)
- `PacResult` with modulation index, preferred phase, and phase-amplitude histogram
- 18-bin phase histogram (20 degrees per bin)
- Significance threshold checking

**Comodulogram:**
- `Comodulogram::compute()` for multi-frequency PAC analysis
- Configurable phase and amplitude frequency ranges with step sizes
- `peak_coupling()` to find strongest coupling pair
- `to_csv()` export for visualization
- `get_coupling()` for specific frequency pair lookup

#### sonido-cli

**New Analyze Subcommands:**
- `analyze pac` - Phase-Amplitude Coupling analysis
  - Configurable phase and amplitude bands
  - MVL and KL methods
  - Surrogate shuffling for significance testing with p-value
  - JSON output with phase histogram
- `analyze comodulogram` - Multi-frequency PAC matrix
  - Configurable frequency ranges and steps
  - Bandwidth ratio control
  - CSV output for heatmap visualization
- `analyze bandpass` - Frequency band extraction
  - Configurable low/high cutoff frequencies
  - Filter order selection (2, 4, 6)
  - WAV output
- `analyze hilbert` - Hilbert transform analysis
  - Instantaneous phase output (normalized -1 to 1)
  - Amplitude envelope output (normalized 0 to 1)
  - Optional pre-filtering with bandpass

**Other CLI Additions:**
- `tui` command for interactive terminal UI
- `presets` command for preset management

---

#### Build Infrastructure

- GitHub Actions CI workflow (`.github/workflows/ci.yml`)
  - Multi-platform testing (Linux, macOS, Windows)
  - `no_std` compatibility checks for core crates
  - Clippy linting and rustfmt checking
  - Cargo caching for faster builds

- GitHub Actions release workflow (`.github/workflows/release.yml`)
  - Triggered on version tags (`v*`)
  - Builds for Linux x64, macOS x64, macOS ARM64, Windows x64
  - Packages CLI and GUI binaries with factory presets
  - Creates GitHub releases with artifacts

#### CLI Audio Device UX

- Device selection by index: `sonido realtime --input 0 --output 0`
- Fuzzy device name matching: `sonido realtime --input "USB" --output "USB"`
- Device list now shows indices for easy reference
- Loopback device detection with `--include-virtual` flag
- Platform-specific guidance for virtual audio setup (VB-Audio, BlackHole, PulseAudio)

#### CLI Preset Management

- `sonido presets export-factory <DIR>` - Export all factory presets as TOML files

#### sonido-io

- `find_device_fuzzy()` - Find devices by partial name match
- `find_device_by_index()` - Find devices by numeric index

### Changed

- `sonido realtime` device options renamed from `--input-device`/`--output-device` to `-i/--input` and `-o/--output` (old names still work as aliases)

---

#### Phase 2: New Guitar Effects

Six new effects expanding the modulation, dynamics, and filter categories:

**Modulation Effects**
- `Tremolo`: Amplitude modulation with sine, triangle, square, and sample-hold waveforms (rate 0.5-20 Hz, depth 0-100%)
- `Flanger`: Classic flanger with modulated short delay (1-10ms range), feedback control, and LFO modulation
- `Phaser`: Multi-stage allpass phaser with 2-12 configurable stages, LFO modulation, and resonance control

**Dynamics Effects**
- `Gate`: Noise gate with threshold, attack, release, and hold time parameters for clean signal gating

**Filter Effects**
- `Wah`: Auto-wah (envelope follower) and manual wah modes using StateVariableFilter in bandpass mode with high Q for classic wah tone
- `ParametricEq`: 3-band parametric equalizer with independent frequency, gain, and Q controls per band using RBJ cookbook peaking EQ coefficients

#### sonido-core
- `peaking_eq_coefficients()`: RBJ Audio EQ Cookbook peaking EQ filter coefficients for parametric equalizers

#### sonido-registry
- Registered all 6 new effects with descriptors and factory functions
- Updated effect count to 15 total effects

#### sonido-cli
- CLI support for all 6 new effects with full parameter parsing
- Wah mode parsing (auto/manual)
- Tremolo waveform parsing (sine, triangle, square, samplehold)
- Short parameter aliases for EQ (lf/lg/lq, mf/mg/mq, hf/hg/hq)

#### Documentation
- Updated EFFECTS_REFERENCE.md with parameter tables for all 6 new effects
- Added example effect chains using new Phase 2 effects

---

#### sonido-registry (NEW CRATE)
- Central effect registry for discovering and instantiating effects by name
- `EffectRegistry` with factory pattern for runtime effect creation
- `EffectDescriptor` with metadata (id, name, description, category, param_count)
- `EffectCategory` enum for organizing effects (Dynamics, Distortion, Modulation, TimeBased, Filter, Utility)
- `EffectWithParams` helper trait for accessing ParameterInfo through boxed effects
- `no_std` compatible with optional `std` feature

#### sonido-core
- `ParameterInfo` trait for runtime parameter introspection
- `ParamDescriptor` struct with name, short_name, unit, min, max, default, step
- `ParamUnit` enum (Decibels, Hertz, Milliseconds, Percent, Ratio, None)
- Helper methods: `clamp()`, `normalize()`, `denormalize()` on ParamDescriptor

#### sonido-effects
- All 9 effects now implement `ParameterInfo` trait
- `CleanPreamp`: Added SmoothedParam for gain and output parameters
- `TapeSaturation`: Added SmoothedParam for drive, saturation, and output_gain
- `MultiVibrato`: Added SmoothedParam for depth_scale parameter

#### sonido-cli
- Shared `preset` module consolidating preset handling across commands

### Changed
- `CleanPreamp::new()` now requires sample_rate parameter
- CLI effects.rs updated to use new effect constructors

### Fixed
- no_std compatibility: replaced `f32::ceil()`, `f32::round()`, `f32::ln()` with libm functions in chorus, delay, reverb, and tape_saturation
- Test modules in gate.rs and wah.rs now correctly import Vec from alloc in no_std mode

### Previously Added
- Root README.md with project overview
- Documentation in `docs/` directory
- Preset files in `presets/` directory
- Makefile for common tasks
- Demo script at `scripts/demo.sh`
- LICENSE-MIT and LICENSE-APACHE files

#### sonido-gui
- Professional egui-based DSP effect processor GUI
- Real-time waveform visualization (input/output)
- Effect chain management with drag-and-drop
- Preset system with save/load functionality
- Audio device selection with hot-swap support
- CPU usage monitoring and performance metrics

#### sonido-effects
- `Reverb` effect: Freeverb-style algorithmic reverb with 8 parallel comb filters and 4 series allpass filters
- `CombFilter` primitive for building reverbs and delays
- `AllpassFilter` primitive for diffusion networks

#### sonido-core
- `CombFilter` delay-based comb filter
- `AllpassFilter` for reverb diffusion

### Changed
- Renamed `next()` to `advance()` in SmoothedParam, LinearSmoothedParam, and Lfo to avoid clippy warnings about iterator naming

### Fixed
- Removed unused imports and dead code warnings

## [0.1.0] - 2024-XX-XX

Initial release.

### Added

#### sonido-core
- `Effect` trait for all audio effects
- `SmoothedParam` for zipper-free parameter changes
- `LinearSmoothedParam` for linear interpolation
- `DelayLine` with fractional delay support
- `Biquad` filter for EQ and filtering
- `Lfo` with sine, triangle, saw, square, sample-and-hold waveforms
- `Oversampling` for 2x/4x oversampling
- Full `no_std` support

#### sonido-effects
- `Distortion` with soft clip, hard clip, foldback, asymmetric modes
- `Compressor` with soft knee and makeup gain
- `Chorus` dual-voice modulated delay
- `Delay` tape-style feedback delay
- `LowPassFilter` resonant 2-pole filter
- `MultiVibrato` 10-unit tape wow/flutter
- `TapeSaturation` with HF rolloff
- `CleanPreamp` gain stage
- Full `no_std` support

#### sonido-analysis
- `Fft` wrapper around rustfft
- `Window` functions (Hamming, Blackman, Hann, Rectangular)
- `TransferFunction` measurement
- `SineSweep` generation

#### sonido-io
- WAV file reading and writing via hound
- Real-time audio streaming via cpal
- `ProcessingEngine` for block-based effect chains

#### sonido-cli
- `process` command for file processing
- `realtime` command for live audio
- `generate` command for test signals
- `analyze` command for spectral analysis
- `compare` command for A/B comparison
- `devices` command for device listing
- `effects` command for effect listing
- TOML preset file support
