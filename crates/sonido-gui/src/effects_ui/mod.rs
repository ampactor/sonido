//! Effect UI panels for each effect type.

mod chorus;
mod compressor;
mod delay;
mod distortion;
mod filter;
mod multivibrato;
mod preamp;
mod reverb;
mod tape;

pub use chorus::ChorusPanel;
pub use compressor::CompressorPanel;
pub use delay::DelayPanel;
pub use distortion::DistortionPanel;
pub use filter::FilterPanel;
pub use multivibrato::MultiVibratoPanel;
pub use preamp::PreampPanel;
pub use reverb::ReverbPanel;
pub use tape::TapePanel;

use crate::audio_bridge::SharedParams;
use egui::Ui;
use std::sync::Arc;

/// Trait for effect UI panels.
pub trait EffectPanel {
    /// The display name of the effect.
    fn name(&self) -> &'static str;

    /// Short name for chain view.
    fn short_name(&self) -> &'static str;

    /// Render the effect's controls.
    fn ui(&mut self, ui: &mut Ui, params: &Arc<SharedParams>);

    /// Check if the effect is bypassed.
    fn is_bypassed(&self, params: &Arc<SharedParams>) -> bool;

    /// Toggle bypass state.
    fn toggle_bypass(&self, params: &Arc<SharedParams>);
}

/// Effect type enumeration for UI purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EffectType {
    Preamp,
    Distortion,
    Compressor,
    Chorus,
    Delay,
    Filter,
    MultiVibrato,
    Tape,
    Reverb,
}

impl EffectType {
    /// Get display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Preamp => "Preamp",
            Self::Distortion => "Distortion",
            Self::Compressor => "Compressor",
            Self::Chorus => "Chorus",
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
            Self::Chorus => "Chor",
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
            Self::Chorus,
            Self::Delay,
            Self::Filter,
            Self::MultiVibrato,
            Self::Tape,
            Self::Reverb,
        ]
    }

    /// Get index in the default order.
    pub fn index(&self) -> usize {
        match self {
            Self::Preamp => 0,
            Self::Distortion => 1,
            Self::Compressor => 2,
            Self::Chorus => 3,
            Self::Delay => 4,
            Self::Filter => 5,
            Self::MultiVibrato => 6,
            Self::Tape => 7,
            Self::Reverb => 8,
        }
    }

    /// Get effect type from index.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::all().get(index).copied()
    }
}
