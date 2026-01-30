# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Root README.md with project overview
- Documentation in `docs/` directory
- Preset files in `presets/` directory
- Makefile for common tasks
- Demo script at `scripts/demo.sh`
- LICENSE-MIT and LICENSE-APACHE files

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
