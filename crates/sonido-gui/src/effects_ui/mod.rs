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

use egui::Ui;
use sonido_gui_core::ParamBridge;

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
    fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: usize);
}

/// Implement [`EffectPanel`] for a panel struct that already has an inherent
/// `ui(&mut self, &mut Ui, &dyn ParamBridge, usize)` method.
macro_rules! impl_effect_panel {
    ($panel:ty, $name:expr, $short:expr) => {
        impl EffectPanel for $panel {
            fn name(&self) -> &'static str {
                $name
            }
            fn short_name(&self) -> &'static str {
                $short
            }
            fn ui(&mut self, ui: &mut Ui, bridge: &dyn ParamBridge, slot: usize) {
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

/// Effect type enumeration for UI purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    /// Preamp / input gain stage.
    Preamp,
    /// Distortion / overdrive.
    Distortion,
    /// Dynamic range compressor.
    Compressor,
    /// Noise gate.
    Gate,
    /// 3-band parametric equalizer.
    ParametricEq,
    /// Wah filter (auto or manual).
    Wah,
    /// Chorus modulation effect.
    Chorus,
    /// Flanger modulation effect.
    Flanger,
    /// Phaser modulation effect.
    Phaser,
    /// Tremolo amplitude modulation.
    Tremolo,
    /// Delay / echo effect.
    Delay,
    /// Low/high-pass filter.
    Filter,
    /// Multi-voice vibrato.
    MultiVibrato,
    /// Tape saturation emulation.
    Tape,
    /// Reverb (room/hall).
    Reverb,
}

impl EffectType {
    /// Get display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Preamp => "Preamp",
            Self::Distortion => "Distortion",
            Self::Compressor => "Compressor",
            Self::Gate => "Gate",
            Self::ParametricEq => "Parametric EQ",
            Self::Wah => "Wah",
            Self::Chorus => "Chorus",
            Self::Flanger => "Flanger",
            Self::Phaser => "Phaser",
            Self::Tremolo => "Tremolo",
            Self::Delay => "Delay",
            Self::Filter => "Filter",
            Self::MultiVibrato => "Vibrato",
            Self::Tape => "Tape",
            Self::Reverb => "Reverb",
        }
    }

    /// Get short name for chain view.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Preamp => "Pre",
            Self::Distortion => "Dist",
            Self::Compressor => "Comp",
            Self::Gate => "Gate",
            Self::ParametricEq => "EQ",
            Self::Wah => "Wah",
            Self::Chorus => "Chor",
            Self::Flanger => "Flgr",
            Self::Phaser => "Phsr",
            Self::Tremolo => "Trem",
            Self::Delay => "Dly",
            Self::Filter => "Flt",
            Self::MultiVibrato => "Vib",
            Self::Tape => "Tape",
            Self::Reverb => "Rev",
        }
    }

    /// Get all effect types in default order.
    pub fn all() -> &'static [EffectType] {
        &[
            Self::Preamp,
            Self::Distortion,
            Self::Compressor,
            Self::Gate,
            Self::ParametricEq,
            Self::Wah,
            Self::Chorus,
            Self::Flanger,
            Self::Phaser,
            Self::Tremolo,
            Self::Delay,
            Self::Filter,
            Self::MultiVibrato,
            Self::Tape,
            Self::Reverb,
        ]
    }

    /// Get index in the default order (= chain slot index).
    pub fn index(&self) -> usize {
        match self {
            Self::Preamp => 0,
            Self::Distortion => 1,
            Self::Compressor => 2,
            Self::Gate => 3,
            Self::ParametricEq => 4,
            Self::Wah => 5,
            Self::Chorus => 6,
            Self::Flanger => 7,
            Self::Phaser => 8,
            Self::Tremolo => 9,
            Self::Delay => 10,
            Self::Filter => 11,
            Self::MultiVibrato => 12,
            Self::Tape => 13,
            Self::Reverb => 14,
        }
    }

    /// Get effect type from index.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::all().get(index).copied()
    }

    /// Look up an [`EffectType`] from a registry effect ID string.
    ///
    /// Returns `None` for unrecognised IDs.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "preamp" => Some(Self::Preamp),
            "distortion" => Some(Self::Distortion),
            "compressor" => Some(Self::Compressor),
            "gate" => Some(Self::Gate),
            "eq" => Some(Self::ParametricEq),
            "wah" => Some(Self::Wah),
            "chorus" => Some(Self::Chorus),
            "flanger" => Some(Self::Flanger),
            "phaser" => Some(Self::Phaser),
            "tremolo" => Some(Self::Tremolo),
            "delay" => Some(Self::Delay),
            "filter" => Some(Self::Filter),
            "multivibrato" => Some(Self::MultiVibrato),
            "tape" => Some(Self::Tape),
            "reverb" => Some(Self::Reverb),
            _ => None,
        }
    }

    /// Returns the registry effect ID string for this effect type.
    ///
    /// This is the inverse of [`from_id`](Self::from_id).
    pub fn effect_id(&self) -> &'static str {
        match self {
            Self::Preamp => "preamp",
            Self::Distortion => "distortion",
            Self::Compressor => "compressor",
            Self::Gate => "gate",
            Self::ParametricEq => "eq",
            Self::Wah => "wah",
            Self::Chorus => "chorus",
            Self::Flanger => "flanger",
            Self::Phaser => "phaser",
            Self::Tremolo => "tremolo",
            Self::Delay => "delay",
            Self::Filter => "filter",
            Self::MultiVibrato => "multivibrato",
            Self::Tape => "tape",
            Self::Reverb => "reverb",
        }
    }
}

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
