//! Filter effect UI panel.

use crate::theme::SonidoTheme;
use crate::widgets::{bridged_combo, bridged_fader};
use crate::{ParamBridge, ParamIndex, SlotIndex};
use egui::Ui;

/// Filter type names matching `FilterParams` filter_type indices.
const FILTER_TYPES: &[&str] = &["LPF", "HPF", "BPF", "Notch"];

/// UI panel for the multimode filter effect.
pub struct FilterPanel;

impl FilterPanel {
    /// Create a new filter panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the filter effect controls.
    ///
    /// Param indices: 0 = cutoff (Hz), 1 = resonance, 2 = output (dB),
    /// 3 = filter type (enum: LPF/HPF/BPF/Notch).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        let theme = SonidoTheme::get(ui.ctx());
        let fader_indices: &[usize] = &[0, 1, 2];
        let param_count = fader_indices.len();
        let avail_w = ui.available_width();
        let fader_w = theme.layout.fader_width(avail_w, param_count);
        let fader_h = theme.layout.fader_height(ui.available_height().min(200.0));

        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                // Filter type selector (param 3)
                ui.label("Type:");
                bridged_combo(ui, bridge, slot, ParamIndex(3), "filter_type", FILTER_TYPES);
            });

            ui.add_space(12.0);

            ui.horizontal_wrapped(|ui| {
                for &i in fader_indices {
                    bridged_fader(ui, bridge, slot, ParamIndex(i), fader_w, fader_h);
                }
            });
        });
    }
}

impl Default for FilterPanel {
    fn default() -> Self {
        Self::new()
    }
}
