//! Phaser effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the phaser effect.
pub struct PhaserPanel;

impl PhaserPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.phaser.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.phaser.store(!active, Ordering::Relaxed);
                }

                ui.add_space(20.0);

                // Stages selector
                ui.label("Stages:");
                let current_stages = params.phaser_stages.load(Ordering::Relaxed) as usize;
                egui::ComboBox::from_id_salt("phaser_stages")
                    .selected_text(format!("{}", current_stages))
                    .show_ui(ui, |ui| {
                        for stages in [2, 4, 6, 8, 10, 12] {
                            if ui
                                .selectable_label(stages == current_stages, format!("{}", stages))
                                .clicked()
                            {
                                params.phaser_stages.store(stages as u32, Ordering::Relaxed);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            // First row: Rate, Depth, Feedback
            ui.horizontal(|ui| {
                // Rate knob (0.05-5 Hz)
                let mut rate = params.phaser_rate.get();
                if ui
                    .add(
                        Knob::new(&mut rate, 0.05, 5.0, "RATE")
                            .default(0.3)
                            .format_hz(),
                    )
                    .changed()
                {
                    params.phaser_rate.set(rate);
                }

                ui.add_space(16.0);

                // Depth knob (0-1)
                let mut depth = params.phaser_depth.get();
                if ui
                    .add(
                        Knob::new(&mut depth, 0.0, 1.0, "DEPTH")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.phaser_depth.set(depth);
                }

                ui.add_space(16.0);

                // Feedback knob (0-0.95)
                let mut feedback = params.phaser_feedback.get();
                if ui
                    .add(
                        Knob::new(&mut feedback, 0.0, 0.95, "FDBK")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.phaser_feedback.set(feedback);
                }

                ui.add_space(16.0);

                // Mix knob (0-1)
                let mut mix = params.phaser_mix.get();
                if ui
                    .add(
                        Knob::new(&mut mix, 0.0, 1.0, "MIX")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.phaser_mix.set(mix);
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
