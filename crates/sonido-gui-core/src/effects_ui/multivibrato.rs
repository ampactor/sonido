//! MultiVibrato effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the MultiVibrato effect.
pub struct MultiVibratoPanel;

impl MultiVibratoPanel {
    /// Create a new multi-vibrato panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the multi-vibrato effect controls.
    ///
    /// Param indices: 0 = depth (%).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Depth (param 0) — percent (0–100)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut depth = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut depth, min, max, "DEPTH")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), depth);
                }
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("6-unit tape wow/flutter simulation")
                    .small()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        });
    }
}

impl Default for MultiVibratoPanel {
    fn default() -> Self {
        Self::new()
    }
}
