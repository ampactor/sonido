//! Compressor effect UI panel.

use crate::widgets::{BypassToggle, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the compressor effect.
pub struct CompressorPanel;

impl CompressorPanel {
    /// Create a new compressor panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the compressor effect controls.
    ///
    /// Param indices: 0 = threshold (dB), 1 = ratio, 2 = attack (ms),
    /// 3 = release (ms), 4 = makeup (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }
            });

            ui.add_space(12.0);

            // First row: Threshold, Ratio, Makeup
            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "THRESH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "RATIO");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "MAKEUP");
            });

            ui.add_space(8.0);

            // Second row: Attack, Release, Mix
            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(2), "ATTACK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(3), "RELEASE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(10), "MIX");
            });
        });
    }
}

impl Default for CompressorPanel {
    fn default() -> Self {
        Self::new()
    }
}
