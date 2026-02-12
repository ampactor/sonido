//! Gate effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::Arc;
use std::sync::atomic::Ordering;

/// UI panel for the noise gate effect.
pub struct GatePanel;

impl GatePanel {
    /// Create a new gate panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the noise gate controls.
    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.gate.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.gate.store(!active, Ordering::Relaxed);
                }
            });

            ui.add_space(12.0);

            // First row: Threshold, Attack
            ui.horizontal(|ui| {
                // Threshold knob (-80 to 0 dB)
                let mut threshold = params.gate_threshold.get();
                if ui
                    .add(
                        Knob::new(&mut threshold, -80.0, 0.0, "THRESH")
                            .default(-40.0)
                            .format_db(),
                    )
                    .changed()
                {
                    params.gate_threshold.set(threshold);
                }

                ui.add_space(16.0);

                // Attack knob (0.1-50 ms)
                let mut attack = params.gate_attack.get();
                if ui
                    .add(
                        Knob::new(&mut attack, 0.1, 50.0, "ATTACK")
                            .default(1.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.gate_attack.set(attack);
                }

                ui.add_space(16.0);

                // Release knob (10-1000 ms)
                let mut release = params.gate_release.get();
                if ui
                    .add(
                        Knob::new(&mut release, 10.0, 1000.0, "RELEASE")
                            .default(100.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.gate_release.set(release);
                }

                ui.add_space(16.0);

                // Hold knob (0-500 ms)
                let mut hold = params.gate_hold.get();
                if ui
                    .add(
                        Knob::new(&mut hold, 0.0, 500.0, "HOLD")
                            .default(50.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.gate_hold.set(hold);
                }
            });
        });
    }
}

impl Default for GatePanel {
    fn default() -> Self {
        Self::new()
    }
}
