//! Effect UI panels for each effect type.
//!
//! Each panel renders controls for one effect, reading and writing parameter
//! values through a [`ParamBridge`]. Parameter
//! indices match the effect's `ParameterInfo`
//! implementation — panels never access DSP state directly.

mod chorus;
mod compressor;
mod delay;
mod distortion;
mod eq;
mod filter;
mod flanger;
mod gate;
mod multivibrato;
mod phaser;
mod preamp;
mod reverb;
mod tape;
mod tremolo;
mod wah;

pub use chorus::ChorusPanel;
pub use compressor::CompressorPanel;
pub use delay::DelayPanel;
pub use distortion::DistortionPanel;
pub use eq::ParametricEqPanel;
pub use filter::FilterPanel;
pub use flanger::FlangerPanel;
pub use gate::GatePanel;
pub use multivibrato::MultiVibratoPanel;
pub use phaser::PhaserPanel;
pub use preamp::PreampPanel;
pub use reverb::ReverbPanel;
pub use tape::TapePanel;
pub use tremolo::TremoloPanel;
pub use wah::WahPanel;

use crate::{ParamBridge, SlotIndex};
use egui::Ui;

/// Trait for effect UI panels.
///
/// Panels render controls for a specific effect type, using the
/// [`ParamBridge`] for all parameter access. The `slot` argument
/// identifies which effect in the chain this panel controls.
pub trait EffectPanel {
    /// The display name of the effect.
    fn name(&self) -> &'static str;

    /// Short name for chain view.
    fn short_name(&self) -> &'static str;

    /// Render the effect's controls.
    fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex);
}

/// Implement [`EffectPanel`] for a panel struct that already has an inherent
/// `ui(&mut self, &mut Ui, &dyn ParamBridge, SlotIndex)` method.
macro_rules! impl_effect_panel {
    ($panel:ty, $name:expr, $short:expr) => {
        impl EffectPanel for $panel {
            fn name(&self) -> &'static str {
                $name
            }
            fn short_name(&self) -> &'static str {
                $short
            }
            fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: SlotIndex) {
                <$panel>::ui(self, ui, bridge, slot);
            }
        }
    };
}

impl_effect_panel!(PreampPanel, "Preamp", "Pre");
impl_effect_panel!(DistortionPanel, "Distortion", "Dist");
impl_effect_panel!(CompressorPanel, "Compressor", "Comp");
impl_effect_panel!(GatePanel, "Gate", "Gate");
impl_effect_panel!(ParametricEqPanel, "Parametric EQ", "EQ");
impl_effect_panel!(WahPanel, "Wah", "Wah");
impl_effect_panel!(ChorusPanel, "Chorus", "Chor");
impl_effect_panel!(FlangerPanel, "Flanger", "Flgr");
impl_effect_panel!(PhaserPanel, "Phaser", "Phsr");
impl_effect_panel!(TremoloPanel, "Tremolo", "Trem");
impl_effect_panel!(DelayPanel, "Delay", "Dly");
impl_effect_panel!(FilterPanel, "Filter", "Flt");
impl_effect_panel!(MultiVibratoPanel, "Vibrato", "Vib");
impl_effect_panel!(TapePanel, "Tape", "Tape");
impl_effect_panel!(ReverbPanel, "Reverb", "Rev");

/// Create an effect panel for the given registry effect ID.
///
/// Returns `None` if `effect_id` doesn't map to a known panel type.
pub fn create_panel(effect_id: &str) -> Option<Box<dyn EffectPanel>> {
    match effect_id {
        "preamp" => Some(Box::new(PreampPanel::new())),
        "distortion" => Some(Box::new(DistortionPanel::new())),
        "compressor" => Some(Box::new(CompressorPanel::new())),
        "gate" => Some(Box::new(GatePanel::new())),
        "eq" => Some(Box::new(ParametricEqPanel::new())),
        "wah" => Some(Box::new(WahPanel::new())),
        "chorus" => Some(Box::new(ChorusPanel::new())),
        "flanger" => Some(Box::new(FlangerPanel::new())),
        "phaser" => Some(Box::new(PhaserPanel::new())),
        "tremolo" => Some(Box::new(TremoloPanel::new())),
        "delay" => Some(Box::new(DelayPanel::new())),
        "filter" => Some(Box::new(FilterPanel::new())),
        "multivibrato" => Some(Box::new(MultiVibratoPanel::new())),
        "tape" => Some(Box::new(TapePanel::new())),
        "reverb" => Some(Box::new(ReverbPanel::new())),
        _ => None,
    }
}
