//! Wah effect UI panel.

use crate::widgets::{BypassToggle, bridged_combo, bridged_knob, bridged_knob_fmt};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

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
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }

                ui.add_space(20.0);

                // Mode selector (param 3)
                ui.label("Mode:");
                bridged_combo(ui, bridge, slot, ParamIndex(3), "wah_mode", WAH_MODES);
            });

            ui.add_space(12.0);

            ui.horizontal(|ui| {
                bridged_knob(ui, bridge, slot, ParamIndex(0), "FREQ");
                ui.add_space(16.0);
                bridged_knob_fmt(ui, bridge, slot, ParamIndex(1), "RESO", |v| {
                    format!("{v:.1}")
                });
                ui.add_space(16.0);
                bridged_knob(ui, bridge, slot, ParamIndex(2), "SENS");
            });
        });
    }
}

impl Default for WahPanel {
    fn default() -> Self {
        Self::new()
    }
}
