//! Chorus effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// UI panel for the chorus effect.
pub struct ChorusPanel;

impl ChorusPanel {
    /// Create a new chorus panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the chorus effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = mix (%).
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
                // Rate (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.1, 10.0, 1.0), |d| (d.min, d.max, d.default));
                let mut rate = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut rate, min, max, "RATE")
                            .default(default)
                            .format(|v| format!("{v:.2} Hz")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), rate);
                }

                ui.add_space(16.0);

                // Depth (param 1) — percent (0–100)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut depth = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut depth, min, max, "DEPTH")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), depth);
                }

                ui.add_space(16.0);

                // Mix (param 2) — percent (0–100)
                let desc = bridge.param_descriptor(slot, ParamIndex(2));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut mix = bridge.get(slot, ParamIndex(2));
                if ui
                    .add(
                        Knob::new(&mut mix, min, max, "MIX")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(2), mix);
                }
            });
        });
    }
}

impl Default for ChorusPanel {
    fn default() -> Self {
        Self::new()
    }
}
