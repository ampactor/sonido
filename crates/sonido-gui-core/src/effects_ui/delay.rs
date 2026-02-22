//! Delay effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;
use sonido_core::DIVISION_LABELS;

/// Sync toggle labels.
const SYNC_LABELS: &[&str] = &["Off", "On"];

/// Ping-pong toggle labels.
const PING_PONG_LABELS: &[&str] = &["Off", "On"];

/// UI panel for the delay effect.
pub struct DelayPanel;

impl DelayPanel {
    /// Create a new delay panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the delay effect controls.
    ///
    /// Param indices: 0 = time (ms), 1 = feedback (%), 2 = mix (%),
    /// 3 = ping pong (on/off), 4 = feedback LP (Hz), 5 = feedback HP (Hz),
    /// 6 = diffusion (%), 7 = sync (on/off), 8 = division (note value),
    /// 9 = output (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                ui.label("Ping Pong:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(3),
                    "delay_ping_pong",
                    PING_PONG_LABELS,
                );

                ui.add_space(12.0);

                ui.label("Sync:");
                bridged_combo(ui, bridge, slot, ParamIndex(7), "delay_sync", SYNC_LABELS);

                ui.add_space(8.0);

                ui.label("Div:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(8),
                    "delay_division",
                    DIVISION_LABELS,
                );
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "TIME");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "FDBK");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "MIX");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(6), "DIFF");
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(4), "FB LP");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(5), "FB HP");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(9), "OUTPUT");
            });
        });
    }
}

impl Default for DelayPanel {
    fn default() -> Self {
        Self::new()
    }
}
