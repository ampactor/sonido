//! Delay effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the delay effect.
pub struct DelayPanel;

impl DelayPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.delay.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.delay.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Time knob
                let mut time = params.delay_time.get();
                if ui
                    .add(
                        Knob::new(&mut time, 1.0, 2000.0, "TIME")
                            .default(300.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.delay_time.set(time);
                }

                ui.add_space(16.0);

                // Feedback knob
                let mut feedback = params.delay_feedback.get();
                if ui
                    .add(
                        Knob::new(&mut feedback, 0.0, 0.95, "FEEDBACK")
                            .default(0.4)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.delay_feedback.set(feedback);
                }

                ui.add_space(16.0);

                // Mix knob
                let mut mix = params.delay_mix.get();
                if ui
                    .add(
                        Knob::new(&mut mix, 0.0, 1.0, "MIX")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.delay_mix.set(mix);
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
