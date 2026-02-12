//! Preamp effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the clean preamp effect.
pub struct PreampPanel;

impl PreampPanel {
    /// Create a new preamp panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the preamp controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.preamp.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.preamp.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                let mut gain = params.preamp_gain.get();
                if ui
                    .add(
                        Knob::new(&mut gain, -20.0, 20.0, "GAIN")
                            .default(0.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.preamp_gain.set(gain);
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
