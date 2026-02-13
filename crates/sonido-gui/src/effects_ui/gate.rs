//! Gate effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::ParamBridge;

/// UI panel for the noise gate effect.
pub struct GatePanel;

impl GatePanel {
    /// Create a new gate panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the noise gate controls.
    ///
    /// Param indices: 0 = threshold (dB), 1 = attack (ms), 2 = release (ms), 3 = hold (ms).
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
                    .map_or((-80.0, 0.0, -40.0), |d| (d.min, d.max, d.default));
                let mut threshold = bridge.get(slot, 0);
                if ui
                    .add(
                        Knob::new(&mut threshold, min, max, "THRESH")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, 0, threshold);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, 1);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.1, 50.0, 1.0), |d| (d.min, d.max, d.default));
                let mut attack = bridge.get(slot, 1);
                if ui
                    .add(
                        Knob::new(&mut attack, min, max, "ATTACK")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, 1, attack);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, 2);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((10.0, 1000.0, 100.0), |d| (d.min, d.max, d.default));
                let mut release = bridge.get(slot, 2);
                if ui
                    .add(
                        Knob::new(&mut release, min, max, "RELEASE")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, 2, release);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, 3);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 500.0, 50.0), |d| (d.min, d.max, d.default));
                let mut hold = bridge.get(slot, 3);
                if ui
                    .add(
                        Knob::new(&mut hold, min, max, "HOLD")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, 3, hold);
                }
            });
        });
    }
}

impl Default for GatePanel {
    fn default() -> Self {
        Self::new()
    }
}
