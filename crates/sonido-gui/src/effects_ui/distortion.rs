//! Distortion effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Waveshape types matching sonido_effects::WaveShape.
const WAVESHAPES: &[&str] = &["Soft Clip", "Hard Clip", "Foldback", "Asymmetric"];

/// UI panel for the distortion effect.
pub struct DistortionPanel;

impl DistortionPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.distortion.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.distortion.store(!active, Ordering::Relaxed);
                }

                ui.add_space(20.0);

                // Waveshape selector
                ui.label("Type:");
                let current = params.dist_waveshape.load(Ordering::Relaxed) as usize;
                let selected = WAVESHAPES.get(current).unwrap_or(&"Soft Clip");
                egui::ComboBox::from_id_salt("waveshape")
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAVESHAPES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                params.dist_waveshape.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Drive knob
                let mut drive = params.dist_drive.get();
                if ui
                    .add(
                        Knob::new(&mut drive, 0.0, 40.0, "DRIVE")
                            .default(0.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.dist_drive.set(drive);
                }

                ui.add_space(16.0);

                // Tone knob
                let mut tone = params.dist_tone.get();
                if ui
                    .add(
                        Knob::new(&mut tone, 500.0, 10000.0, "TONE")
                            .default(8000.0)
                            .format_hz(),
                    )
                    .changed()
                {
                    params.dist_tone.set(tone);
                }

                ui.add_space(16.0);

                // Level knob
                let mut level = params.dist_level.get();
                if ui
                    .add(
                        Knob::new(&mut level, -20.0, 0.0, "LEVEL")
                            .default(0.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.dist_level.set(level);
                }
            });
        });
    }
}

impl Default for DistortionPanel {
    fn default() -> Self {
        Self::new()
    }
}
