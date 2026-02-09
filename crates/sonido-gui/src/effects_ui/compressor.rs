//! Compressor effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the compressor effect.
pub struct CompressorPanel;

impl CompressorPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.compressor.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.compressor.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            // First row: Threshold, Ratio
            ui.horizontal(|ui| {
                let mut threshold = params.comp_threshold.get();
                if ui
                    .add(
                        Knob::new(&mut threshold, -40.0, 0.0, "THRESH")
                            .default(-20.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.comp_threshold.set(threshold);
                }

                ui.add_space(16.0);

                let mut ratio = params.comp_ratio.get();
                if ui
                    .add(
                        Knob::new(&mut ratio, 1.0, 20.0, "RATIO")
                            .default(4.0)
                            .format_ratio(),
                    )
                    .changed()
                {
                    params.comp_ratio.set(ratio);
                }

                ui.add_space(16.0);

                let mut makeup = params.comp_makeup.get();
                if ui
                    .add(
                        Knob::new(&mut makeup, 0.0, 20.0, "MAKEUP")
                            .default(0.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.comp_makeup.set(makeup);
                }
            });

            ui.add_space(8.0);

            // Second row: Attack, Release
            ui.horizontal(|ui| {
                let mut attack = params.comp_attack.get();
                if ui
                    .add(
                        Knob::new(&mut attack, 0.1, 100.0, "ATTACK")
                            .default(10.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.comp_attack.set(attack);
                }

                ui.add_space(16.0);

                let mut release = params.comp_release.get();
                if ui
                    .add(
                        Knob::new(&mut release, 10.0, 1000.0, "RELEASE")
                            .default(100.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.comp_release.set(release);
                }
            });
        });
    }
}

impl Default for CompressorPanel {
    fn default() -> Self {
        Self::new()
    }
}
