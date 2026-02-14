//! Tape saturation effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::{ParamBridge, ParamIndex, SlotIndex};

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
                // Drive (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 24.0, 6.0), |d| (d.min, d.max, d.default));
                let mut drive = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut drive, min, max, "DRIVE")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), drive);
                }

                ui.add_space(16.0);

                // Saturation (param 1) — percent (0–100)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut saturation = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut saturation, min, max, "SAT")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), saturation);
                }
            });
        });
    }
}

impl Default for TapePanel {
    fn default() -> Self {
        Self::new()
    }
}
