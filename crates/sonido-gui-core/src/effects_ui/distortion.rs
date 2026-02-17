//! Distortion effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Waveshape types matching `sonido_effects::WaveShape`.
const WAVESHAPES: &[&str] = &["Soft Clip", "Hard Clip", "Foldback", "Asymmetric"];

/// UI panel for the distortion effect.
pub struct DistortionPanel;

impl DistortionPanel {
    /// Create a new distortion panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the distortion effect controls.
    ///
    /// Param indices: 0 = drive (dB), 1 = tone (Hz), 2 = level (dB), 3 = waveshape (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Waveshape selector (param 3)
                ui.label("Type:");
                bridged_combo(ui, bridge, slot, ParamIndex(3), "waveshape", WAVESHAPES);
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "DRIVE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "TONE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "LEVEL");
            });
        });
    }
}

impl Default for DistortionPanel {
    fn default() -> Self {
        Self::new()
    }
}
