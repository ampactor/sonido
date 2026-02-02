//! Sonido Analysis - Spectral tools for audio DSP reverse engineering
//!
//! This crate provides analysis tools for reverse engineering audio algorithms:
//!
//! - [`fft`] - FFT wrapper with windowing functions
//! - [`spectrum`] - Spectral analysis utilities (including Welch's method)
//! - [`dynamics`] - Dynamics analysis (RMS, crest factor, dynamic range)
//! - [`ir`] - Impulse response capture via sine sweep
//! - [`transfer_fn`] - Transfer function measurement
//! - [`compare`] - A/B comparison tools
//! - [`distortion`] - THD, THD+N, and IMD analysis
//! - [`spectrogram`] - STFT-based time-frequency analysis
//! - [`constant_q`] - Constant-Q transform for pitch-based analysis
//! - [`filterbank`] - Bandpass filter bank for frequency band extraction
//! - [`hilbert`] - Hilbert transform for analytic signals
//! - [`cfc`] - Cross-Frequency Coupling (Phase-Amplitude Coupling) analysis
//!
//! ## Target Use Case
//!
//! This crate is designed to help reverse engineer audio algorithms like those
//! in DigiTech audioDNA pedals (Polara, Obscura, Ventura, etc.).
//!
//! ## Example Workflow
//!
//! ```rust,ignore
//! use sonido_analysis::{SineSweep, TransferFunction, spectrum};
//!
//! // 1. Generate test signal
//! let sweep = SineSweep::new(48000.0, 20.0, 20000.0, 2.0);
//!
//! // 2. Record pedal response (external)
//!
//! // 3. Analyze transfer function
//! let tf = TransferFunction::from_sweep(&input, &output, 48000.0);
//!
//! // 4. Compare with implementation
//! let similarity = spectrum::correlation(&original, &implementation);
//! ```
//!
//! ## Distortion Analysis
//!
//! ```rust,ignore
//! use sonido_analysis::distortion::{ThdAnalyzer, generate_test_tone};
//!
//! // Generate test tone
//! let signal = generate_test_tone(48000.0, 1000.0, 1.0, 0.5);
//!
//! // Analyze THD
//! let analyzer = ThdAnalyzer::new(48000.0, 8192);
//! let result = analyzer.analyze(&signal, 1000.0);
//! println!("THD: {:.2}% ({:.1} dB)", result.thd_ratio * 100.0, result.thd_db);
//! ```
//!
//! ## Spectrogram
//!
//! ```rust,ignore
//! use sonido_analysis::spectrogram::StftAnalyzer;
//! use sonido_analysis::fft::Window;
//!
//! let analyzer = StftAnalyzer::new(48000.0, 2048, 512, Window::Hann);
//! let spectrogram = analyzer.analyze(&signal);
//!
//! // Get peak frequency at frame 10
//! let peak = spectrogram.peak_frequency(10);
//! ```

pub mod fft;
pub mod spectrum;
pub mod dynamics;
pub mod ir;
pub mod transfer_fn;
pub mod compare;
pub mod distortion;
pub mod spectrogram;
pub mod constant_q;
pub mod export;
pub mod filterbank;
pub mod hilbert;
pub mod cfc;

// Re-export main types
pub use fft::{Fft, Window};
pub use spectrum::{magnitude_spectrum, phase_spectrum, spectral_centroid, welch_psd, coherence};
pub use dynamics::{rms, rms_db, peak, peak_db, crest_factor, crest_factor_db, analyze_dynamics, DynamicsAnalysis};
pub use ir::{SineSweep, trim_ir, energy_decay_curve, estimate_rt60, Rt60Estimate};
pub use transfer_fn::{TransferFunction, Resonance, unwrap_phase};
pub use compare::{spectral_correlation, spectral_difference};
pub use distortion::{ThdAnalyzer, ThdResult, ImdAnalyzer, ImdResult, generate_test_tone};
pub use spectrogram::{Spectrogram, StftAnalyzer, MelFilterbank, MelSpectrogram};
pub use constant_q::{ConstantQTransform, CqtResult, CqtSpectrogram, Chromagram};
pub use filterbank::{FilterBank, FrequencyBand, eeg_bands};
pub use hilbert::HilbertTransform;
pub use cfc::{PacAnalyzer, PacResult, PacMethod, Comodulogram};
