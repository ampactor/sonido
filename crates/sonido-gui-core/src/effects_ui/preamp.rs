//! Preamp effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the clean preamp effect.
pub struct PreampPanel;

impl PreampPanel {
    /// Create a new preamp panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the preamp controls.
    ///
    /// Param indices: 0 = gain (dB).
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "GAIN");
            });
        });
    }
}

impl Default for PreampPanel {
    fn default() -> Self {
        Self::new()
    }
}
