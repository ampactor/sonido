//! Tape saturation effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the tape saturation effect.
pub struct TapePanel;

impl TapePanel {
    /// Create a new tape saturation panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the tape saturation controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.tape.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.tape.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Drive knob
                let mut drive = params.tape_drive.get();
                if ui
                    .add(
                        Knob::new(&mut drive, 0.0, 24.0, "DRIVE")
                            .default(6.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.tape_drive.set(drive);
                }

                ui.add_space(16.0);

                // Saturation knob
                let mut saturation = params.tape_saturation.get();
                if ui
                    .add(
                        Knob::new(&mut saturation, 0.0, 1.0, "SAT")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.tape_saturation.set(saturation);
                }
            });
        });
    }
}

impl Default for TapePanel {
    fn default() -> Self {
        Self::new()
    }
}
