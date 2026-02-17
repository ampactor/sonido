//! Reverb effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Reverb types matching `sonido_effects::ReverbType`.
const REVERB_TYPES: &[&str] = &["Room", "Hall"];

/// UI panel for the reverb effect.
pub struct ReverbPanel;

impl ReverbPanel {
    /// Create a new reverb panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the reverb effect controls.
    ///
    /// Param indices: 0 = room_size (%), 1 = decay (%), 2 = damping (%),
    /// 3 = predelay (ms), 4 = mix (%), 5 = stereo_width (%), 6 = type (enum).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Reverb type selector (param 6)
                ui.label("Type:");
                let current = bridge.get(slot, ParamIndex(6)) as u32 as usize;
                let selected = REVERB_TYPES.get(current).unwrap_or(&"Room");
                egui::ComboBox::from_id_salt(("reverb_type", slot.0))
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in REVERB_TYPES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                bridge.set(slot, ParamIndex(6), i as f32);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            // First row: Room Size, Decay, Damping
            ui.horizontal(|ui| {
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut room_size = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut room_size, min, max, "SIZE")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), room_size);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut decay = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut decay, min, max, "DECAY")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), decay);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(2));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 50.0), |d| (d.min, d.max, d.default));
                let mut damping = bridge.get(slot, ParamIndex(2));
                if ui
                    .add(
                        Knob::new(&mut damping, min, max, "DAMP")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(2), damping);
                }
            });

            ui.add_space(8.0);

            // Second row: Predelay, Mix
            ui.horizontal(|ui| {
                let desc = bridge.param_descriptor(slot, ParamIndex(3));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 10.0), |d| (d.min, d.max, d.default));
                let mut predelay = bridge.get(slot, ParamIndex(3));
                if ui
                    .add(
                        Knob::new(&mut predelay, min, max, "PREDLY")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(3), predelay);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(4));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 100.0, 30.0), |d| (d.min, d.max, d.default));
                let mut mix = bridge.get(slot, ParamIndex(4));
                if ui
                    .add(
                        Knob::new(&mut mix, min, max, "MIX")
                            .default(default)
                            .format(|v| format!("{v:.0}%")),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(4), mix);
                }
            });
        });
    }
}

impl Default for ReverbPanel {
    fn default() -> Self {
        Self::new()
    }
}
