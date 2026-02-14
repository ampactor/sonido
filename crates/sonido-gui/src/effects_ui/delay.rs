//! Delay effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::{ParamBridge, ParamIndex, SlotIndex};

/// UI panel for the delay effect.
pub struct DelayPanel;

impl DelayPanel {
    /// Create a new delay panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the delay effect controls.
    ///
    /// Param indices: 0 = time (ms), 1 = feedback (%), 2 = mix (%).
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
                // Time (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((1.0, 2000.0, 300.0), |d| (d.min, d.max, d.default));
                let mut time = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut time, min, max, "TIME")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), time);
                }

                ui.add_space(16.0);

                // Feedback (param 1) — percent (0–95)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 95.0, 50.0), |d| (d.min, d.max, d.default));
                let mut feedback = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut feedback, min, max, "FEEDBACK")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), feedback);
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

impl Default for DelayPanel {
    fn default() -> Self {
        Self::new()
    }
}
