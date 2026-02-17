//! Audio-specific GUI widgets.
//!
//! Reusable widgets for building audio effect interfaces:
//! - [`Knob`] — Rotary control with drag, fine control, and double-click reset
//! - [`bridged_knob`] — Bridge-aware knob with auto-format and gesture protocol
//! - [`bridged_knob_fmt`] — Bridge-aware knob with custom formatter
//! - [`bridged_combo`] — Bridge-aware combo box for enum parameters
//! - [`gesture_wrap`] — Gesture protocol helper for custom widget layouts
//! - [`LevelMeter`] — VU-style peak/RMS meter (vertical or horizontal)
//! - [`GainReductionMeter`] — Compressor gain reduction display
//! - [`BypassToggle`] — Small bypass indicator for effect panels
//! - [`FootswitchToggle`] — Large pedal-style toggle for the chain view

mod bridged_knob;
mod knob;
mod meter;
mod toggle;

pub use bridged_knob::{bridged_combo, bridged_knob, bridged_knob_fmt, gesture_wrap};
pub use knob::Knob;
pub use meter::{GainReductionMeter, LevelMeter};
pub use toggle::{BypassToggle, FootswitchToggle};
