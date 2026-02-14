//! Compressor effect UI panel.

use crate::widgets::{BypassToggle, Knob};
use egui::Ui;
use sonido_gui_core::{ParamBridge, ParamIndex, SlotIndex};

/// UI panel for the compressor effect.
pub struct CompressorPanel;

impl CompressorPanel {
    /// Create a new compressor panel.
    pub fn new() -> Self {
        Self
    }

    /// Render the compressor effect controls.
    ///
    /// Param indices: 0 = threshold (dB), 1 = ratio, 2 = attack (ms),
    /// 3 = release (ms), 4 = makeup (dB).
    pub fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                let mut active = !bridge.is_bypassed(slot);
                if ui.add(BypassToggle::new(&mut active, "Active")).changed() {
                    bridge.set_bypassed(slot, !active);
                }
            });

            ui.add_space(12.0);

            // First row: Threshold, Ratio, Makeup
            ui.horizontal(|ui| {
                let desc = bridge.param_descriptor(slot, ParamIndex(0));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((-40.0, 0.0, -20.0), |d| (d.min, d.max, d.default));
                let mut threshold = bridge.get(slot, ParamIndex(0));
                if ui
                    .add(
                        Knob::new(&mut threshold, min, max, "THRESH")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(0), threshold);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(1));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((1.0, 20.0, 4.0), |d| (d.min, d.max, d.default));
                let mut ratio = bridge.get(slot, ParamIndex(1));
                if ui
                    .add(
                        Knob::new(&mut ratio, min, max, "RATIO")
                            .default(default)
                            .format_ratio(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(1), ratio);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(4));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.0, 20.0, 0.0), |d| (d.min, d.max, d.default));
                let mut makeup = bridge.get(slot, ParamIndex(4));
                if ui
                    .add(
                        Knob::new(&mut makeup, min, max, "MAKEUP")
                            .default(default)
                            .format_db(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(4), makeup);
                }
            });

            ui.add_space(8.0);

            // Second row: Attack, Release
            ui.horizontal(|ui| {
                let desc = bridge.param_descriptor(slot, ParamIndex(2));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((0.1, 100.0, 10.0), |d| (d.min, d.max, d.default));
                let mut attack = bridge.get(slot, ParamIndex(2));
                if ui
                    .add(
                        Knob::new(&mut attack, min, max, "ATTACK")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(2), attack);
                }

                ui.add_space(16.0);

                let desc = bridge.param_descriptor(slot, ParamIndex(3));
                let (min, max, default) = desc
                    .as_ref()
                    .map_or((10.0, 1000.0, 100.0), |d| (d.min, d.max, d.default));
                let mut release = bridge.get(slot, ParamIndex(3));
                if ui
                    .add(
                        Knob::new(&mut release, min, max, "RELEASE")
                            .default(default)
                            .format_ms(),
                    )
                    .changed()
                {
                    bridge.set(slot, ParamIndex(3), release);
                }
            });
        });
    }
}

impl Default for CompressorPanel {
    fn default() -> Self {
        Self::new()
    }
}
