//! Wah effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// Wah mode names.
const WAH_MODES: &[&str] = &["Auto", "Manual"];

/// UI panel for the wah effect.
pub struct WahPanel;

impl WahPanel {
    /// Create a new wah panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the wah effect controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.wah.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.wah.store(!active, Ordering::Relaxed);
                }

                ui.add_space(20.0);

                // Mode selector
                ui.label("Mode:");
                let current = params.wah_mode.load(Ordering::Relaxed) as usize;
                let selected = WAH_MODES.get(current).unwrap_or(&"Auto");
                egui::ComboBox::from_id_salt("wah_mode")
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAH_MODES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                params.wah_mode.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Frequency knob (200-2000 Hz)
                let mut freq = params.wah_frequency.get();
                if ui
                    .add(
                        Knob::new(&mut freq, 200.0, 2000.0, "FREQ")
                            .default(800.0)
                            .format_hz(),
                    )
                    .changed()
                {
                    params.wah_frequency.set(freq);
                }

                ui.add_space(16.0);

                // Resonance knob (1-10)
                let mut resonance = params.wah_resonance.get();
                if ui
                    .add(
                        Knob::new(&mut resonance, 1.0, 10.0, "RESO")
                            .default(5.0)
                            .format(|v| format!("{:.1}", v)),
                    )
                    .changed()
                {
                    params.wah_resonance.set(resonance);
                }

                ui.add_space(16.0);

                // Sensitivity knob (0-1)
                let mut sensitivity = params.wah_sensitivity.get();
                if ui
                    .add(
                        Knob::new(&mut sensitivity, 0.0, 1.0, "SENS")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.wah_sensitivity.set(sensitivity);
                }
            });
        });
    }
}

impl Default for WahPanel {
    fn default() -> Self {
        Self::new()
    }
}
