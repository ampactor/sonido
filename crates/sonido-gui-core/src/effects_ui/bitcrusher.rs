//! Bitcrusher effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the bitcrusher effect.
pub struct BitcrusherPanel;

impl BitcrusherPanel {
    /// Create a new bitcrusher panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the bitcrusher controls.
    ///
    /// Param indices: 0 = bit depth, 1 = downsample, 2 = jitter, 3 = mix, 4 = output.
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
                bridged_knob(ui, bridge, slot, ParamIndex(0), "BITS");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DOWN");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "JITTER");
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(3), "MIX");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "OUTPUT");
            });
        });
    }
}

impl Default for BitcrusherPanel {
    fn default() -> Self {
        Self::new()
    }
}
