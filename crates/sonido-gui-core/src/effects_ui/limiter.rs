//! Limiter effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the brickwall limiter effect.
pub struct LimiterPanel;

impl LimiterPanel {
    /// Create a new limiter panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the limiter controls.
    ///
    /// Param indices: 0 = threshold, 1 = ceiling, 2 = release, 3 = lookahead, 4 = output.
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
                bridged_knob(ui, bridge, slot, ParamIndex(1), "CEIL");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "REL");
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(3), "LOOK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "OUTPUT");
            });
        });
    }
}

impl Default for LimiterPanel {
    fn default() -> Self {
        Self::new()
    }
}
