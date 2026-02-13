//! Audio-specific GUI widgets.
//!
//! Reusable widgets for building audio effect interfaces:
//! - [`Knob`] — Rotary control with drag, fine control, and double-click reset
//! - [`LevelMeter`] — VU-style peak/RMS meter (vertical or horizontal)
//! - [`GainReductionMeter`] — Compressor gain reduction display
//! - [`BypassToggle`] — Small bypass indicator for effect panels
//! - [`FootswitchToggle`] — Large pedal-style toggle for the chain view

mod knob;
mod meter;
mod toggle;

pub use knob::Knob;
pub use meter::{GainReductionMeter, LevelMeter};
pub use toggle::{BypassToggle, FootswitchToggle};
