//! Tremolo effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Waveform types for tremolo.
const WAVEFORMS: &[&str] = &["Sine", "Triangle", "Square", "S&H"];

/// UI panel for the tremolo effect.
pub struct TremoloPanel;

impl TremoloPanel {
    /// Create a new tremolo panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the tremolo effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = waveform (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Waveform selector (param 2)
                ui.label("Wave:");
                let current = bridge.get(slot, ParamIndex(2)) as u32 as usize;
                let selected = WAVEFORMS.get(current).unwrap_or(&"Sine");
                egui::ComboBox::from_id_salt(("tremolo_waveform", slot.0))
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAVEFORMS.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                bridge.set(slot, ParamIndex(2), i as f32);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Rate (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.5, 20.0, 5.0), |d| (d.min, d.max, d.default));
                let mut rate = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut rate, min, max, "RATE")
                            .default(default)
                            .format_hz(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), rate);
                }

                ui.add_space(16.0);

                // Depth (param 1) — percent (0–100)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut depth = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut depth, min, max, "DEPTH")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), depth);
                }
            });
        });
    }
}

impl Default for TremoloPanel {
    fn default() -> Self {
        Self::new()
    }
}
