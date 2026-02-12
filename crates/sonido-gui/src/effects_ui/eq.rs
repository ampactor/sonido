//! Parametric EQ effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the 3-band parametric EQ effect.
pub struct ParametricEqPanel;

impl ParametricEqPanel {
    /// Create a new parametric EQ panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the parametric EQ controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.eq.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.eq.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            // Low band
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("LOW")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .small(),
                );
                ui.add_space(8.0);

                // Low frequency knob (20-500 Hz)
                let mut low_freq = params.eq_low_freq.get();
                if ui
                    .add(
                        Knob::new(&mut low_freq, 20.0, 500.0, "FREQ")
                            .default(100.0)
                            .format_hz()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_low_freq.set(low_freq);
                }

                ui.add_space(8.0);

                // Low gain knob (-12 to +12 dB)
                let mut low_gain = params.eq_low_gain.get();
                if ui
                    .add(
                        Knob::new(&mut low_gain, -12.0, 12.0, "GAIN")
                            .default(0.0)
                            .format_db()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_low_gain.set(low_gain);
                }

                ui.add_space(8.0);

                // Low Q knob (0.5-5)
                let mut low_q = params.eq_low_q.get();
                if ui
                    .add(
                        Knob::new(&mut low_q, 0.5, 5.0, "Q")
                            .default(1.0)
                            .format(|v| format!("{:.1}", v))
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_low_q.set(low_q);
                }
            });

            ui.add_space(4.0);

            // Mid band
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("MID")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .small(),
                );
                ui.add_space(8.0);

                // Mid frequency knob (200-5000 Hz)
                let mut mid_freq = params.eq_mid_freq.get();
                if ui
                    .add(
                        Knob::new(&mut mid_freq, 200.0, 5000.0, "FREQ")
                            .default(1000.0)
                            .format_hz()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_mid_freq.set(mid_freq);
                }

                ui.add_space(8.0);

                // Mid gain knob (-12 to +12 dB)
                let mut mid_gain = params.eq_mid_gain.get();
                if ui
                    .add(
                        Knob::new(&mut mid_gain, -12.0, 12.0, "GAIN")
                            .default(0.0)
                            .format_db()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_mid_gain.set(mid_gain);
                }

                ui.add_space(8.0);

                // Mid Q knob (0.5-5)
                let mut mid_q = params.eq_mid_q.get();
                if ui
                    .add(
                        Knob::new(&mut mid_q, 0.5, 5.0, "Q")
                            .default(1.0)
                            .format(|v| format!("{:.1}", v))
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_mid_q.set(mid_q);
                }
            });

            ui.add_space(4.0);

            // High band
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("HIGH")
                        .color(egui::Color32::from_rgb(150, 150, 160))
                        .small(),
                );
                ui.add_space(4.0);

                // High frequency knob (1000-15000 Hz)
                let mut high_freq = params.eq_high_freq.get();
                if ui
                    .add(
                        Knob::new(&mut high_freq, 1000.0, 15000.0, "FREQ")
                            .default(5000.0)
                            .format_hz()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_high_freq.set(high_freq);
                }

                ui.add_space(8.0);

                // High gain knob (-12 to +12 dB)
                let mut high_gain = params.eq_high_gain.get();
                if ui
                    .add(
                        Knob::new(&mut high_gain, -12.0, 12.0, "GAIN")
                            .default(0.0)
                            .format_db()
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_high_gain.set(high_gain);
                }

                ui.add_space(8.0);

                // High Q knob (0.5-5)
                let mut high_q = params.eq_high_q.get();
                if ui
                    .add(
                        Knob::new(&mut high_q, 0.5, 5.0, "Q")
                            .default(1.0)
                            .format(|v| format!("{:.1}", v))
                            .diameter(50.0),
                    )
                    .changed()
                {
                    params.eq_high_q.set(high_q);
                }
            });
        });
    }
}

impl Default for ParametricEqPanel {
    fn default() -> Self {
        Self::new()
    }
}
