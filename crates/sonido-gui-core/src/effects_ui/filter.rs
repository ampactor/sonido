//! Filter effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the low-pass filter effect.
pub struct FilterPanel;

impl FilterPanel {
    /// Create a new filter panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the filter effect controls.
    ///
    /// Param indices: 0 = cutoff (Hz), 1 = resonance.
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
                // Cutoff (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((20.0, 20000.0, 5000.0), |d| (d.min, d.max, d.default));
                let mut cutoff = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut cutoff, min, max, "CUTOFF")
                            .default(default)
                            .format_hz()
                            .sensitivity(0.008),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), cutoff);
                }

                ui.add_space(16.0);

                // Resonance (param 1)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.1, 10.0, 0.7), |d| (d.min, d.max, d.default));
                let mut resonance = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut resonance, min, max, "RESO")
                            .default(default)
                            .format(|v| format!("{v:.1}")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), resonance);
                }
            });
        });
    }
}

impl Default for FilterPanel {
    fn default() -> Self {
        Self::new()
    }
}
