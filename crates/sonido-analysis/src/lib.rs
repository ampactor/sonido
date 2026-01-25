//! Sonido Analysis - Spectral tools for audio DSP reverse engineering
//!
//! This crate provides analysis tools for reverse engineering audio algorithms:
//!
//! - [`fft`] - FFT wrapper with windowing functions
//! - [`spectrum`] - Spectral analysis utilities
//! - [`ir`] - Impulse response capture via sine sweep
//! - [`transfer_fn`] - Transfer function measurement
//! - [`compare`] - A/B comparison tools
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

pub mod fft;
pub mod spectrum;
pub mod ir;
pub mod transfer_fn;
pub mod compare;

// Re-export main types
pub use fft::{Fft, Window};
pub use spectrum::{magnitude_spectrum, phase_spectrum, spectral_centroid};
pub use ir::SineSweep;
pub use transfer_fn::TransferFunction;
pub use compare::{spectral_correlation, spectral_difference};
