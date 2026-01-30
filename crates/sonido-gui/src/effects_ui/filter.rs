//! Filter effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// UI panel for the low-pass filter effect.
pub struct FilterPanel;

impl FilterPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.filter.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.filter.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Cutoff knob (logarithmic would be ideal, but linear works)
                let mut cutoff = params.filter_cutoff.get();
                if ui
                    .add(
                        Knob::new(&mut cutoff, 20.0, 20000.0, "CUTOFF")
                            .default(5000.0)
                            .format_hz()
                            .sensitivity(0.008), // Higher sensitivity for wider range
                    )
                    .changed()
                {
                    params.filter_cutoff.set(cutoff);
                }

                ui.add_space(16.0);

                // Resonance knob
                let mut resonance = params.filter_resonance.get();
                if ui
                    .add(
                        Knob::new(&mut resonance, 0.1, 10.0, "RESO")
                            .default(0.7)
                            .format(|v| format!("{:.1}", v)),
                    )
                    .changed()
                {
                    params.filter_resonance.set(resonance);
                }
            });
        });
    }
}

impl Default for FilterPanel {
    fn default() -> Self {
        Self::new()
    }
}
