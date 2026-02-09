//! Tremolo effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Waveform types for tremolo.
const WAVEFORMS: &[&str] = &["Sine", "Triangle", "Square", "S&H"];

/// UI panel for the tremolo effect.
pub struct TremoloPanel;

impl TremoloPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.tremolo.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.tremolo.store(!active, Ordering::Relaxed);
                }

                ui.add_space(20.0);

                // Waveform selector
                ui.label("Wave:");
                let current = params.tremolo_waveform.load(Ordering::Relaxed) as usize;
                let selected = WAVEFORMS.get(current).unwrap_or(&"Sine");
                egui::ComboBox::from_id_salt("tremolo_waveform")
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAVEFORMS.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                params.tremolo_waveform.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Rate knob (0.5-20 Hz)
                let mut rate = params.tremolo_rate.get();
                if ui
                    .add(
                        Knob::new(&mut rate, 0.5, 20.0, "RATE")
                            .default(5.0)
                            .format_hz(),
                    )
                    .changed()
                {
                    params.tremolo_rate.set(rate);
                }

                ui.add_space(16.0);

                // Depth knob (0-1)
                let mut depth = params.tremolo_depth.get();
                if ui
                    .add(
                        Knob::new(&mut depth, 0.0, 1.0, "DEPTH")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.tremolo_depth.set(depth);
                }
            });
        });
    }
}

impl Default for TremoloPanel {
    fn default() -> Self {
        Self::new()
    }
}
