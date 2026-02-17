//! Bridge-aware parameter knobs with gesture protocol.
//!
//! [`bridged_knob`] connects a rotary [`Knob`] to a [`ParamBridge`] slot,
//! handling descriptor lookup, auto-formatting based on [`ParamUnit`],
//! and VST3/CLAP gesture events (`begin_set` on drag start, `end_set` on drag stop).
//!
//! # Functions
//!
//! - [`bridged_knob`] — auto-formatted knob from `ParamUnit`
//! - [`bridged_knob_fmt`] — knob with custom value formatter
//! - [`bridged_combo`] — combo box for enum parameters
//! - [`gesture_wrap`] — low-level gesture protocol for custom widgets

use super::Knob;
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::{Response, Ui};
use sonido_core::ParamUnit;

/// Apply the gesture protocol to a widget response.
///
/// Wraps `begin_set`/`end_set` around drag and double-click interactions.
/// Use this with raw [`Knob`] widgets that need custom properties
/// (e.g., `.diameter()`, `.sensitivity()`) but still want gesture support.
///
/// For double-click resets, a complete `begin_set → set(default) → end_set`
/// sequence is emitted. Regular drags emit `begin_set` on drag start,
/// `set(value)` on each change, and `end_set` on drag stop.
pub fn gesture_wrap(
    response: &Response,
    bridge: &dyn ParamBridge,
    slot: SlotIndex,
    param: ParamIndex,
    value: f32,
    default: f32,
) {
    if response.double_clicked() {
        bridge.begin_set(slot, param);
        bridge.set(slot, param, default);
        bridge.end_set(slot, param);
    } else {
        if response.drag_started() {
            bridge.begin_set(slot, param);
        }
        if response.changed() {
            bridge.set(slot, param, value);
        }
        if response.drag_stopped() {
            bridge.end_set(slot, param);
        }
    }
}

/// Render a parameter knob bound to a [`ParamBridge`] slot.
///
/// Handles descriptor lookup (min/max/default), auto-formatting based on
/// [`ParamUnit`], and the gesture protocol (`begin_set`/`end_set`).
/// Double-click resets to the parameter's default value.
///
/// # Auto-format mapping
///
/// | `ParamUnit`      | Display format              |
/// |------------------|-----------------------------|
/// | `Decibels`       | `"-3.5 dB"`                 |
/// | `Hertz`          | `"1.2 kHz"` / `"440 Hz"`   |
/// | `Milliseconds`   | `"1.50 s"` / `"100.0 ms"`  |
/// | `Percent`        | `"50%"` (value is 0–100)    |
/// | `Ratio`          | `"4.0:1"`                   |
/// | `None` / unknown | `"0.50"` (2 decimal places) |
pub fn bridged_knob(
    ui: &mut Ui,
    bridge: &dyn ParamBridge,
    slot: SlotIndex,
    param: ParamIndex,
    label: &str,
) -> Response {
    let desc = bridge.param_descriptor(slot, param);
    let (min, max, default) = desc
        .as_ref()
        .map_or((0.0, 1.0, 0.5), |d| (d.min, d.max, d.default));

    let mut value = bridge.get(slot, param);
    let knob = Knob::new(&mut value, min, max, label).default(default);

    let knob = match desc.as_ref().map(|d| d.unit) {
        Some(ParamUnit::Decibels) => knob.format_db(),
        Some(ParamUnit::Hertz) => knob.format_hz(),
        Some(ParamUnit::Milliseconds) => knob.format_ms(),
        Some(ParamUnit::Percent) => knob.format(|v| format!("{v:.0}%")),
        Some(ParamUnit::Ratio) => knob.format_ratio(),
        _ => knob,
    };

    let response = ui.add(knob);
    gesture_wrap(&response, bridge, slot, param, value, default);
    response
}

/// Like [`bridged_knob`] but with a custom value formatter.
///
/// Use when the auto-format from [`ParamUnit`] doesn't match the desired
/// display (e.g., custom precision, special suffix, or no unit suffix).
pub fn bridged_knob_fmt(
    ui: &mut Ui,
    bridge: &dyn ParamBridge,
    slot: SlotIndex,
    param: ParamIndex,
    label: &str,
    format: impl Fn(f32) -> String + 'static,
) -> Response {
    let desc = bridge.param_descriptor(slot, param);
    let (min, max, default) = desc
        .as_ref()
        .map_or((0.0, 1.0, 0.5), |d| (d.min, d.max, d.default));

    let mut value = bridge.get(slot, param);
    let knob = Knob::new(&mut value, min, max, label)
        .default(default)
        .format(format);

    let response = ui.add(knob);
    gesture_wrap(&response, bridge, slot, param, value, default);
    response
}

/// Render a combo box for an enum parameter bound to a [`ParamBridge`] slot.
///
/// The parameter value is stored as a float index (`0.0`, `1.0`, `2.0`, …).
/// Selection changes are wrapped in gesture protocol events.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]
pub fn bridged_combo(
    ui: &mut Ui,
    bridge: &dyn ParamBridge,
    slot: SlotIndex,
    param: ParamIndex,
    id_salt: &str,
    labels: &[&str],
) -> Response {
    let current = bridge.get(slot, param) as u32 as usize;
    let selected = labels
        .get(current)
        .copied()
        .unwrap_or(labels.first().copied().unwrap_or("?"));

    let response = egui::ComboBox::from_id_salt((id_salt, slot.0))
        .selected_text(selected)
        .show_ui(ui, |ui| {
            for (i, name) in labels.iter().enumerate() {
                if ui.selectable_label(i == current, *name).clicked() {
                    bridge.begin_set(slot, param);
                    bridge.set(slot, param, i as f32);
                    bridge.end_set(slot, param);
                }
            }
        });

    response.response
}
