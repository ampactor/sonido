//! Scale-aware ADC-to-parameter conversion — re-exported from `sonido-platform`.
//!
//! The implementation lives in [`sonido_platform::param_map`] so it can be
//! tested on the host and shared across platform crates.

pub use sonido_platform::param_map::{adc_to_param, adc_to_param_biased};
