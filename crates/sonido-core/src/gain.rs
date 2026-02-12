//! Gain staging helpers for the universal output level contract.
//!
//! Every effect in sonido exposes an output level parameter as the *last* index
//! in its [`ParameterInfo`](crate::ParameterInfo) implementation. This module
//! provides the shared vocabulary for that contract: a standard
//! [`SmoothedParam`] constructor, dB get/set helpers,
//! and a canonical [`ParamDescriptor`].
//!
//! # Usage
//!
//! ```rust
//! use sonido_core::gain;
//!
//! let mut output = gain::output_level_param(48000.0);
//! gain::set_output_level_db(&mut output, -3.0);
//! assert!((gain::output_level_db(&output) - (-3.0)).abs() < 0.1);
//! ```
//!
//! # Design
//!
//! The output level operates in linear gain internally (for multiplication in
//! the audio path) but exposes a dB interface for parameter get/set. The range
//! is [`OUTPUT_MIN_DB`] to [`OUTPUT_MAX_DB`] (−20 to +20 dB), clamped on set.

use crate::{ParamDescriptor, ParamUnit, SmoothedParam, db_to_linear, linear_to_db};

/// Minimum output level in dB.
pub const OUTPUT_MIN_DB: f32 = -20.0;

/// Maximum output level in dB.
pub const OUTPUT_MAX_DB: f32 = 20.0;

/// Create a [`SmoothedParam`] for the standard output level (0 dB, 10 ms smoothing).
///
/// The param stores linear gain internally. At 0 dB the value is 1.0 (unity).
pub fn output_level_param(sample_rate: f32) -> SmoothedParam {
    SmoothedParam::standard(1.0, sample_rate)
}

/// Set output level from dB, clamped to [`OUTPUT_MIN_DB`]..=[`OUTPUT_MAX_DB`].
#[inline]
pub fn set_output_level_db(param: &mut SmoothedParam, db: f32) {
    param.set_target(db_to_linear(db.clamp(OUTPUT_MIN_DB, OUTPUT_MAX_DB)));
}

/// Read output level target as dB.
#[inline]
pub fn output_level_db(param: &SmoothedParam) -> f32 {
    linear_to_db(param.target())
}

/// Standard [`ParamDescriptor`] for output level.
///
/// Use as the **last** `ParameterInfo` index for every effect.
pub fn output_param_descriptor() -> ParamDescriptor {
    ParamDescriptor {
        name: "Output",
        short_name: "Out",
        unit: ParamUnit::Decibels,
        min: OUTPUT_MIN_DB,
        max: OUTPUT_MAX_DB,
        default: 0.0,
        step: 0.5,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_level_roundtrip() {
        let mut param = output_level_param(48000.0);
        set_output_level_db(&mut param, -6.0);
        let db = output_level_db(&param);
        assert!((db - (-6.0)).abs() < 0.01, "Expected -6.0, got {db}");
    }

    #[test]
    fn output_level_clamping() {
        let mut param = output_level_param(48000.0);

        set_output_level_db(&mut param, -50.0);
        assert!(
            (output_level_db(&param) - OUTPUT_MIN_DB).abs() < 0.01,
            "Should clamp to min"
        );

        set_output_level_db(&mut param, 50.0);
        assert!(
            (output_level_db(&param) - OUTPUT_MAX_DB).abs() < 0.01,
            "Should clamp to max"
        );
    }

    #[test]
    fn output_descriptor_defaults() {
        let desc = output_param_descriptor();
        assert_eq!(desc.name, "Output");
        assert_eq!(desc.default, 0.0);
        assert_eq!(desc.unit, ParamUnit::Decibels);
    }
}
