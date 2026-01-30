//! MultiVibrato effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// UI panel for the MultiVibrato effect.
pub struct MultiVibratoPanel;

impl MultiVibratoPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.multivibrato.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.multivibrato.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Depth knob
                let mut depth = params.vibrato_depth.get();
                if ui
                    .add(
                        Knob::new(&mut depth, 0.0, 1.0, "DEPTH")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.vibrato_depth.set(depth);
                }
            });

            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("10-unit tape wow/flutter simulation")
                    .small()
                    .color(egui::Color32::from_rgb(120, 120, 130)),
            );
        });
    }
}

impl Default for MultiVibratoPanel {
    fn default() -> Self {
        Self::new()
    }
}
