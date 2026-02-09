//! Chorus effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the chorus effect.
pub struct ChorusPanel;

impl ChorusPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.chorus.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.chorus.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Rate knob
                let mut rate = params.chorus_rate.get();
                if ui
                    .add(
                        Knob::new(&mut rate, 0.1, 10.0, "RATE")
                            .default(1.0)
                            .format(|v| format!("{:.2} Hz", v)),
                    )
                    .changed()
                {
                    params.chorus_rate.set(rate);
                }

                ui.add_space(16.0);

                // Depth knob
                let mut depth = params.chorus_depth.get();
                if ui
                    .add(
                        Knob::new(&mut depth, 0.0, 1.0, "DEPTH")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.chorus_depth.set(depth);
                }

                ui.add_space(16.0);

                // Mix knob
                let mut mix = params.chorus_mix.get();
                if ui
                    .add(
                        Knob::new(&mut mix, 0.0, 1.0, "MIX")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.chorus_mix.set(mix);
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
