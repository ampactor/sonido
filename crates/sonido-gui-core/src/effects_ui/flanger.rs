//! Flanger effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the flanger effect.
pub struct FlangerPanel;

impl FlangerPanel {
    /// Create a new flanger panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the flanger effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = feedback (%), 3 = mix (%).
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "RATE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "FDBK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(3), "MIX");
            });
        });
    }
}

impl Default for FlangerPanel {
    fn default() -> Self {
        Self::new()
    }
}
