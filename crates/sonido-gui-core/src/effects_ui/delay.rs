//! Delay effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the delay effect.
pub struct DelayPanel;

impl DelayPanel {
    /// Create a new delay panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the delay effect controls.
    ///
    /// Param indices: 0 = time (ms), 1 = feedback (%), 2 = mix (%).
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "TIME");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "FEEDBACK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "MIX");
            });
        });
    }
}

impl Default for DelayPanel {
    fn default() -> Self {
        Self::new()
    }
}
