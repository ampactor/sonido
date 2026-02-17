//! Tremolo effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Waveform types for tremolo.
const WAVEFORMS: &[&str] = &["Sine", "Triangle", "Square", "S&H"];

/// UI panel for the tremolo effect.
pub struct TremoloPanel;

impl TremoloPanel {
    /// Create a new tremolo panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the tremolo effect controls.
    ///
    /// Param indices: 0 = rate (Hz), 1 = depth (%), 2 = waveform (enum).
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
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "RATE");
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(1), "DEPTH");
            });
        });
    }
}

impl Default for TremoloPanel {
    fn default() -> Self {
        Self::new()
    }
}
