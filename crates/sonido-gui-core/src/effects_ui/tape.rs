//! Tape saturation effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the tape saturation effect.
pub struct TapePanel;

impl TapePanel {
    /// Create a new tape saturation panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the tape saturation controls.
    ///
    /// Param indices: 0 = drive (dB), 1 = saturation (%).
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "DRIVE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "SAT");
            });
        });
    }
}

impl Default for TapePanel {
    fn default() -> Self {
        Self::new()
    }
}
