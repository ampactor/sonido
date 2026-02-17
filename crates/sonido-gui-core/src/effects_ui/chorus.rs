//! Chorus effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the chorus effect.
pub struct ChorusPanel;

impl ChorusPanel {
    /// Create a new chorus panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the chorus effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = mix (%).
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
                bridged_knob(ui, bridge, slot, ParamIndex(2), "MIX");
            });
        });
    }
}

impl Default for ChorusPanel {
    fn default() -> Self {
        Self::new()
    }
}
