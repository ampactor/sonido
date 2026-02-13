//! Phaser effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::ParamBridge;

/// UI panel for the phaser effect.
pub struct PhaserPanel;

impl PhaserPanel {
    /// Create a new phaser panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the phaser effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = stages (enum),
    /// 3 = feedback (%), 4 = mix (%).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: usize) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Stages selector (param 2)
                ui.label("Stages:");
                let current_stages = bridge.get(slot, 2) as usize;
                egui::ComboBox::from_id_salt("phaser_stages")
                    .selected_text(format!("{current_stages}"))
                    .show_ui(ui, |ui| {
                        for stages in [2, 4, 6, 8, 10, 12] {
                            if ui
                                .selectable_label(stages == current_stages, format!("{stages}"))
                                .clicked()
                            {
                                bridge.set(slot, 2, stages as f32);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Rate (param 0)
                let desc = bridge.param_descriptor(slot, 0);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.05, 5.0, 0.3), |d| (d.min, d.max, d.default));
                let mut rate = bridge.get(slot, 0);
                if ui
                    .add(
                        Knob::new(&mut rate, min, max, "RATE")
                            .default(default)
                            .format_hz(),
                    )
                    .changed()
                {
                    bridge.set(slot, 0, rate);
                }

                ui.add_space(16.0);

                // Depth (param 1) — percent (0–100)
                let desc = bridge.param_descriptor(slot, 1);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut depth = bridge.get(slot, 1);
                if ui
                    .add(
                        Knob::new(&mut depth, min, max, "DEPTH")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, 1, depth);
                }

                ui.add_space(16.0);

                // Feedback (param 3) — percent (0–95)
                let desc = bridge.param_descriptor(slot, 3);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 95.0, 50.0), |d| (d.min, d.max, d.default));
                let mut feedback = bridge.get(slot, 3);
                if ui
                    .add(
                        Knob::new(&mut feedback, min, max, "FDBK")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, 3, feedback);
                }

                ui.add_space(16.0);

                // Mix (param 4) — percent (0–100)
                let desc = bridge.param_descriptor(slot, 4);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut mix = bridge.get(slot, 4);
                if ui
                    .add(
                        Knob::new(&mut mix, min, max, "MIX")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, 4, mix);
                }
            });
        });
    }
}

impl Default for PhaserPanel {
    fn default() -> Self {
        Self::new()
    }
}
