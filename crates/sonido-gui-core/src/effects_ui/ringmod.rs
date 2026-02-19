//! Ring modulator effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Carrier waveform labels for the combo box.
const WAVEFORMS: &[&str] = &["Sine", "Triangle", "Square"];

/// UI panel for the ring modulator effect.
pub struct RingModPanel;

impl RingModPanel {
    /// Create a new ring modulator panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the ring modulator controls.
    ///
    /// Param indices: 0 = frequency, 1 = depth, 2 = waveform, 3 = mix, 4 = output.
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                ui.label("Wave:");
                bridged_combo(ui, bridge, slot, ParamIndex(2), "wave", WAVEFORMS);
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "FREQ");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
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

impl Default for RingModPanel {
    fn default() -> Self {
        Self::new()
    }
}
