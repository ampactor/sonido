//! Chorus effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;
use sonido_core::DIVISION_LABELS;

/// Sync toggle labels.
const SYNC_LABELS: &[&str] = &["Off", "On"];

/// UI panel for the chorus effect.
pub struct ChorusPanel;

impl ChorusPanel {
    /// Create a new chorus panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the chorus effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = mix (%),
    /// 3 = voices, 4 = feedback (%), 5 = base delay (ms),
    /// 6 = sync (on/off), 7 = division (note value), 8 = output (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                ui.label("Sync:");
                bridged_combo(ui, bridge, slot, ParamIndex(6), "chorus_sync", SYNC_LABELS);

                ui.add_space(8.0);

                ui.label("Div:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(7),
                    "chorus_division",
                    DIVISION_LABELS,
                );
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "RATE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "MIX");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(4), "FDBK");
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(3), "VOICES");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(5), "B.DLY");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(8), "OUTPUT");
            });
        });
    }
}

impl Default for ChorusPanel {
    fn default() -> Self {
        Self::new()
    }
}
