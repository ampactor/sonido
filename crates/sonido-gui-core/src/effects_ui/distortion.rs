//! Distortion effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Waveshape types matching `sonido_effects::WaveShape`.
const WAVESHAPES: &[&str] = &["Soft Clip", "Hard Clip", "Foldback", "Asymmetric"];

/// UI panel for the distortion effect.
pub struct DistortionPanel;

impl DistortionPanel {
    /// Create a new distortion panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the distortion effect controls.
    ///
    /// Param indices: 0 = drive (dB), 1 = tone (Hz), 2 = level (dB), 3 = waveshape (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Waveshape selector (param 3)
                ui.label("Type:");
                let current = bridge.get(slot, ParamIndex(3)) as u32 as usize;
                let selected = WAVESHAPES.get(current).unwrap_or(&"Soft Clip");
                egui::ComboBox::from_id_salt(("waveshape", slot.0))
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in WAVESHAPES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                bridge.set(slot, ParamIndex(3), i as f32);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                // Drive (param 0)
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 40.0, 0.0), |d| (d.min, d.max, d.default));
                let mut drive = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut drive, min, max, "DRIVE")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), drive);
                }

                ui.add_space(16.0);

                // Tone (param 1)
                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((500.0, 10000.0, 8000.0), |d| (d.min, d.max, d.default));
                let mut tone = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut tone, min, max, "TONE")
                            .default(default)
                            .format_hz(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), tone);
                }

                ui.add_space(16.0);

                // Level (param 2)
                let desc = bridge.param_descriptor(slot, ParamIndex(2));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((-20.0, 0.0, 0.0), |d| (d.min, d.max, d.default));
                let mut level = bridge.get(slot, ParamIndex(2));
                if ui
                    .add(
                        Knob::new(&mut level, min, max, "LEVEL")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(2), level);
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
