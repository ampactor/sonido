//! Flanger effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the flanger effect.
pub struct FlangerPanel;

impl FlangerPanel {
    /// Create a new flanger panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the flanger effect controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.flanger.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.flanger.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Rate knob (0.05-5 Hz)
                let mut rate = params.flanger_rate.get();
                if ui
                    .add(
                        Knob::new(&mut rate, 0.05, 5.0, "RATE")
                            .default(0.5)
                            .format_hz(),
                    )
                    .changed()
                {
                    params.flanger_rate.set(rate);
                }

                ui.add_space(16.0);

                // Depth knob (0-1)
                let mut depth = params.flanger_depth.get();
                if ui
                    .add(
                        Knob::new(&mut depth, 0.0, 1.0, "DEPTH")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.flanger_depth.set(depth);
                }

                ui.add_space(16.0);

                // Feedback knob (0-0.95)
                let mut feedback = params.flanger_feedback.get();
                if ui
                    .add(
                        Knob::new(&mut feedback, 0.0, 0.95, "FDBK")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.flanger_feedback.set(feedback);
                }

                ui.add_space(16.0);

                // Mix knob (0-1)
                let mut mix = params.flanger_mix.get();
                if ui
                    .add(
                        Knob::new(&mut mix, 0.0, 1.0, "MIX")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.flanger_mix.set(mix);
                }
            });
        });
    }
}

impl Default for FlangerPanel {
    fn default() -> Self {
        Self::new()
    }
}
