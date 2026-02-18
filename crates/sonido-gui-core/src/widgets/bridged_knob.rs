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
use sonido_core::{ParamDescriptor, ParamUnit};

/// Normalize a plain value to \[0, 1\] using the descriptor's scale, or linear fallback.
fn normalize(desc: Option<&ParamDescriptor>, value: f32, min: f32, max: f32) -> f32 {
    desc.map_or_else(
        || {
            if (max - min).abs() < f32::EPSILON {
                0.0
            } else {
                (value - min) / (max - min)
            }
        },
        |d| d.normalize(value),
    )
}

/// Denormalize a \[0, 1\] value back to plain using the descriptor's scale, or linear fallback.
fn denormalize(desc: Option<&ParamDescriptor>, normalized: f32, min: f32, max: f32) -> f32 {
    desc.map_or_else(
        || min + normalized * (max - min),
        |d| d.denormalize(normalized),
    )
}

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
/// The knob internally operates in normalized \[0, 1\] space, mapped through
/// the parameter's [`ParamScale`](sonido_core::ParamScale). This ensures
/// logarithmic parameters (e.g., filter cutoff 20–20 kHz) have their visual
/// midpoint at the geometric mean (√(min×max)), not the arithmetic mean.
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
    let (min, max, default) = desc.map_or((0.0, 1.0, 0.5), |d| (d.min, d.max, d.default));

    let plain_value = bridge.get(slot, param);

    // Normalize to [0, 1] using the parameter's scale curve (log, power, linear)
    let mut normalized = normalize(desc.as_ref(), plain_value, min, max);
    let norm_default = normalize(desc.as_ref(), default, min, max);

    let knob = Knob::new(&mut normalized, 0.0, 1.0, label).default(norm_default);

    // Format: denormalize back to plain value, then apply unit formatting
    let knob = if let Some(d) = desc {
        match d.unit {
            ParamUnit::Decibels => knob.format(move |n| {
                let v = d.denormalize(n);
                format!("{v:.1} dB")
            }),
            ParamUnit::Hertz => knob.format(move |n| {
                let v = d.denormalize(n);
                if v >= 1000.0 {
                    format!("{:.1} kHz", v / 1000.0)
                } else {
                    format!("{v:.0} Hz")
                }
            }),
            ParamUnit::Milliseconds => knob.format(move |n| {
                let v = d.denormalize(n);
                if v >= 1000.0 {
                    format!("{:.2} s", v / 1000.0)
                } else {
                    format!("{v:.1} ms")
                }
            }),
            ParamUnit::Percent => knob.format(move |n| {
                let v = d.denormalize(n);
                format!("{v:.0}%")
            }),
            ParamUnit::Ratio => knob.format(move |n| {
                let v = d.denormalize(n);
                format!("{v:.1}:1")
            }),
            ParamUnit::None => knob.format(move |n| {
                let v = d.denormalize(n);
                format!("{v:.2}")
            }),
        }
    } else {
        knob
    };

    let response = ui.add(knob);

    // Denormalize back to plain value for the bridge
    let plain_out = denormalize(desc.as_ref(), normalized, min, max);
    gesture_wrap(&response, bridge, slot, param, plain_out, default);
    response
}

/// Like [`bridged_knob`] but with a custom value formatter.
///
/// Use when the auto-format from [`ParamUnit`] doesn't match the desired
/// display (e.g., custom precision, special suffix, or no unit suffix).
///
/// The `format` callback receives the **plain** parameter value (Hz, dB, etc.),
/// not the normalized knob position. Scale-aware normalization is handled
/// internally, identical to [`bridged_knob`].
pub fn bridged_knob_fmt(
    ui: &mut Ui,
    bridge: &dyn ParamBridge,
    slot: SlotIndex,
    param: ParamIndex,
    label: &str,
    format: impl Fn(f32) -> String + 'static,
) -> Response {
    let desc = bridge.param_descriptor(slot, param);
    let (min, max, default) = desc.map_or((0.0, 1.0, 0.5), |d| (d.min, d.max, d.default));

    let plain_value = bridge.get(slot, param);

    let mut normalized = normalize(desc.as_ref(), plain_value, min, max);
    let norm_default = normalize(desc.as_ref(), default, min, max);

    // Wrap user's format fn: denormalize [0,1] → plain before formatting
    let knob = Knob::new(&mut normalized, 0.0, 1.0, label)
        .default(norm_default)
        .format(move |n| {
            let plain = denormalize(desc.as_ref(), n, min, max);
            format(plain)
        });

    let response = ui.add(knob);

    let plain_out = denormalize(desc.as_ref(), normalized, min, max);
    gesture_wrap(&response, bridge, slot, param, plain_out, default);
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
