//! Tremolo effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;
use sonido_core::DIVISION_LABELS;

/// Waveform types for tremolo.
const WAVEFORMS: &[&str] = &["Sine", "Triangle", "Square", "S&H"];

/// Sync toggle labels.
const SYNC_LABELS: &[&str] = &["Off", "On"];

/// UI panel for the tremolo effect.
pub struct TremoloPanel;

impl TremoloPanel {
    /// Create a new tremolo panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the tremolo effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = waveform (enum),
    /// 3 = spread (%), 4 = sync (on/off), 5 = division (note value),
    /// 6 = output (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Waveform selector (param 2)
                ui.label("Wave:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(2),
                    "tremolo_waveform",
                    WAVEFORMS,
                );

                ui.add_space(12.0);

                ui.label("Sync:");
                bridged_combo(ui, bridge, slot, ParamIndex(4), "tremolo_sync", SYNC_LABELS);

                ui.add_space(8.0);

                ui.label("Div:");
                bridged_combo(
                    ui,
                    bridge,
                    slot,
                    ParamIndex(5),
                    "tremolo_division",
                    DIVISION_LABELS,
                );
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "RATE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(3), "SPREAD");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(6), "OUTPUT");
            });
        });
    }
}

impl Default for TremoloPanel {
    fn default() -> Self {
        Self::new()
    }
}
