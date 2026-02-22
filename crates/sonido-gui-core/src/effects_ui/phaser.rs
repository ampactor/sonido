//! Phaser effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;
use sonido_core::DIVISION_LABELS;

/// Sync toggle labels.
const SYNC_LABELS: &[&str] = &["Off", "On"];

/// UI panel for the phaser effect.
pub struct PhaserPanel;

impl PhaserPanel {
    /// Create a new phaser panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the phaser effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = stages (enum),
    /// 3 = feedback (%), 4 = mix (%), 5 = min freq (Hz), 6 = max freq (Hz),
    /// 7 = sync (on/off), 8 = division (note value), 9 = output (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Stages selector (param 2) — non-sequential values, manual gesture wrap
                ui.label("Stages:");
                let current_stages = bridge.get(slot, ParamIndex(2)) as usize;
                egui::ComboBox::from_id_salt(("phaser_stages", slot.0))
                    .selected_text(format!("{current_stages}"))
                    .show_ui(ui, |ui| {
                        for stages in [2, 4, 6, 8, 10, 12] {
                            if ui
                                .selectable_label(stages == current_stages, format!("{stages}"))
                                .clicked()
                            {
                                bridge.begin_set(slot, ParamIndex(2));
                                bridge.set(slot, ParamIndex(2), stages as f32);
                                bridge.end_set(slot, ParamIndex(2));
                            }
                        }
                    });

                ui.add_space(12.0);

                ui.label("Sync:");
                bridged_combo(ui, bridge, slot, ParamIndex(7), "phaser_sync", SYNC_LABELS);

                ui.add_space(8.0);

                ui.label("Div:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(8),
                    "phaser_division",
                    DIVISION_LABELS,
                );
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "RATE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(3), "FDBK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "MIX");
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(5), "MIN F");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(6), "MAX F");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(9), "OUTPUT");
            });
        });
    }
}

impl Default for PhaserPanel {
    fn default() -> Self {
        Self::new()
    }
}
