//! Reverb effect UI panel.

use crate::audio_bridge::SharedParams;
use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Reverb types matching sonido_effects::ReverbType.
const REVERB_TYPES: &[&str] = &["Room", "Hall"];

/// UI panel for the reverb effect.
pub struct ReverbPanel;

impl ReverbPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !params.bypass.reverb.load(Ordering::Relaxed);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    params.bypass.reverb.store(!active, Ordering::Relaxed);
                }

                ui.add_space(20.0);

                // Reverb type selector
                ui.label("Type:");
                let current = params.reverb_type.load(Ordering::Relaxed) as usize;
                let selected = REVERB_TYPES.get(current).unwrap_or(&"Room");
                egui::ComboBox::from_id_salt("reverb_type")
                    .selected_text(*selected)
                    .show_ui(ui, |ui| {
                        for (i, name) in REVERB_TYPES.iter().enumerate() {
                            if ui.selectable_label(i == current, *name).clicked() {
                                params.reverb_type.store(i as u32, Ordering::Relaxed);
                            }
                        }
                    });
            });

            ui.add_space(12.0);

            // First row: Room Size, Decay, Damping
            ui.horizontal(|ui| {
                let mut room_size = params.reverb_room_size.get();
                if ui
                    .add(
                        Knob::new(&mut room_size, 0.0, 1.0, "SIZE")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.reverb_room_size.set(room_size);
                }

                ui.add_space(16.0);

                let mut decay = params.reverb_decay.get();
                if ui
                    .add(
                        Knob::new(&mut decay, 0.0, 1.0, "DECAY")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.reverb_decay.set(decay);
                }

                ui.add_space(16.0);

                let mut damping = params.reverb_damping.get();
                if ui
                    .add(
                        Knob::new(&mut damping, 0.0, 1.0, "DAMP")
                            .default(0.5)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.reverb_damping.set(damping);
                }
            });

            ui.add_space(8.0);

            // Second row: Predelay, Mix
            ui.horizontal(|ui| {
                let mut predelay = params.reverb_predelay.get();
                if ui
                    .add(
                        Knob::new(&mut predelay, 0.0, 100.0, "PREDLY")
                            .default(10.0)
                            .format_ms(),
                    )
                    .changed()
                {
                    params.reverb_predelay.set(predelay);
                }

                ui.add_space(16.0);

                let mut mix = params.reverb_mix.get();
                if ui
                    .add(
                        Knob::new(&mut mix, 0.0, 1.0, "MIX")
                            .default(0.3)
                            .format_percent(),
                    )
                    .changed()
                {
                    params.reverb_mix.set(mix);
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
