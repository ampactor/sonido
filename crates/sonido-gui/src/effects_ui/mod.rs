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
}
