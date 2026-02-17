//! Gate effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the noise gate effect.
pub struct GatePanel;

impl GatePanel {
    /// Create a new gate panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the noise gate controls.
    ///
    /// Param indices: 0 = threshold (dB), 1 = attack (ms), 2 = release (ms), 3 = hold (ms).
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "THRESH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "ATTACK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "RELEASE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(3), "HOLD");
            });
        });
    }
}

impl Default for GatePanel {
    fn default() -> Self {
        Self::new()
    }
}
