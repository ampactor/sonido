//! Preamp effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::ParamBridge;

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
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: usize) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let desc = bridge.param_descriptor(slot, 0);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((-20.0, 20.0, 0.0), |d| (d.min, d.max, d.default));
                let mut gain = bridge.get(slot, 0);
                if ui
                    .add(
                        Knob::new(&mut gain, min, max, "GAIN")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, 0, gain);
                }
            });
        });
    }
}

impl Default for PreampPanel {
    fn default() -> Self {
        Self::new()
    }
}
