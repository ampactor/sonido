//! Wah effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::ParamBridge;

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
    ///
    /// Param indices: 0 = frequency (Hz), 1 = resonance, 2 = sensitivity (%), 3 = mode (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: usize) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Mode selector (param 3)
                ui.label("Mode:");
                let current = bridge.get(slot, 3) as u32 as usize;
                let selected = WAH_MODES.get(current).unwrap_or(&"Auto");
                egui::ComboBox::from_id_salt("wah_mode")
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAH_MODES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                bridge.set(slot, 3, i as f32);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Frequency (param 0)
                let desc = bridge.param_descriptor(slot, 0);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((200.0, 2000.0, 800.0), |d| (d.min, d.max, d.default));
                let mut freq = bridge.get(slot, 0);
                if ui
                    .add(
                        Knob::new(&mut freq, min, max, "FREQ")
                            .default(default)
                            .format_hz(),
                    )
                    .changed()
                {
                    bridge.set(slot, 0, freq);
                }

                ui.add_space(16.0);

                // Resonance (param 1)
                let desc = bridge.param_descriptor(slot, 1);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((1.0, 10.0, 5.0), |d| (d.min, d.max, d.default));
                let mut resonance = bridge.get(slot, 1);
                if ui
                    .add(
                        Knob::new(&mut resonance, min, max, "RESO")
                            .default(default)
                            .format(|v| format!("{v:.1}")),
                    )
                    .changed()
                {
                    bridge.set(slot, 1, resonance);
                }

                ui.add_space(16.0);

                // Sensitivity (param 2) — percent in ParameterInfo units (0–100)
                let desc = bridge.param_descriptor(slot, 2);
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut sensitivity = bridge.get(slot, 2);
                if ui
                    .add(
                        Knob::new(&mut sensitivity, min, max, "SENS")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, 2, sensitivity);
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
