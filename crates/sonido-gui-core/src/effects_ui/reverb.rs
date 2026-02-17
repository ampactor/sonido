//! Reverb effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Reverb types matching `sonido_effects::ReverbType`.
const REVERB_TYPES: &[&str] = &["Room", "Hall"];

/// UI panel for the reverb effect.
pub struct ReverbPanel;

impl ReverbPanel {
    /// Create a new reverb panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the reverb effect controls.
    ///
    /// Param indices: 0 = room_size (%), 1 = decay (%), 2 = damping (%),
    /// 3 = predelay (ms), 4 = mix (%), 5 = stereo_width (%), 6 = type (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Reverb type selector (param 6)
                ui.label("Type:");
                bridged_combo(ui, bridge, slot, ParamIndex(6), "reverb_type", REVERB_TYPES);
            });

            ui.add_space(12.0);

            // First row: Room Size, Decay, Damping
            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "SIZE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DECAY");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "DAMP");
            });

            ui.add_space(8.0);

            // Second row: Predelay, Mix
            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(3), "PREDLY");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "MIX");
            });
        });
    }
}

impl Default for ReverbPanel {
    fn default() -> Self {
        Self::new()
    }
}
