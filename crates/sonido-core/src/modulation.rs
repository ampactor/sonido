//! Modulation source abstraction for parameter control.
//!
//! Provides a unified interface for autonomous modulation generators:
//! LFOs, ADSR envelopes, and audio-rate modulators. The trait is for
//! sources that produce time-varying signals independently — not
//! input-dependent processors.
//!
//! `EnvelopeFollower` requires input via `process()` and does not
//! implement this trait.

use crate::Lfo;

/// Trait for anything that can generate modulation signals.
///
/// Modulation sources produce time-varying values used to control effect
/// parameters (filter cutoff, amplitude, pitch, etc.). The trait provides
/// a unified interface across fundamentally different signal generators:
///
/// - **LFOs** (bipolar, free-running periodic signals)
/// - **ADSR envelopes** (unipolar, gate-triggered)
/// - **Audio-rate modulators** (bipolar, for FM/AM synthesis)
///
/// The bipolar/unipolar distinction matters for correct modulation routing.
/// A bipolar LFO (-1 to 1) centered around zero creates symmetric modulation
/// (vibrato). A unipolar envelope (0 to 1) creates one-directional modulation
/// (filter sweep from low to high). The `mod_advance_unipolar()` and
/// `mod_advance_bipolar()` conversion methods handle the math so that
/// modulation destinations don't need to know the source type.
///
/// # Example
///
/// ```rust
/// use sonido_core::{Lfo, ModulationSource};
///
/// let mut lfo = Lfo::new(48000.0, 2.0);
///
/// // Use through the trait
/// let value = lfo.mod_advance();
/// assert!(value >= -1.0 && value <= 1.0);
/// assert!(lfo.is_bipolar());
/// ```
pub trait ModulationSource {
    /// Get the next modulation value.
    ///
    /// Returns a value in the range:
    /// - Bipolar sources: -1.0 to 1.0
    /// - Unipolar sources: 0.0 to 1.0
    fn mod_advance(&mut self) -> f32;

    /// Check if this source is bipolar (-1 to 1) or unipolar (0 to 1).
    fn is_bipolar(&self) -> bool;

    /// Reset the modulation source to its initial state.
    fn mod_reset(&mut self);

    /// Get the current value without advancing.
    fn mod_value(&self) -> f32;

    /// Convert to unipolar (0 to 1) regardless of source type.
    fn mod_advance_unipolar(&mut self) -> f32 {
        let value = self.mod_advance();
        if self.is_bipolar() {
            (value + 1.0) * 0.5
        } else {
            value
        }
    }

    /// Convert to bipolar (-1 to 1) regardless of source type.
    fn mod_advance_bipolar(&mut self) -> f32 {
        let value = self.mod_advance();
        if self.is_bipolar() {
            value
        } else {
            value * 2.0 - 1.0
        }
    }
}

impl ModulationSource for Lfo {
    fn mod_advance(&mut self) -> f32 {
        self.advance()
    }

    fn is_bipolar(&self) -> bool {
        true
    }

    fn mod_reset(&mut self) {
        self.reset();
    }

    fn mod_value(&self) -> f32 {
        self.value_at_phase()
    }
}

/// A modulation amount that can be applied to a parameter.
///
/// Combines a modulation source with depth and optional inversion.
/// The `apply()` method computes `base + mod_value * depth * range`,
/// where `range` is in the parameter's native units (Hz for filter cutoff,
/// semitones for pitch, etc.). This additive model is standard in synth
/// architectures and matches user expectations: depth=0.5 with range=1000 Hz
/// means the filter sweeps +/- 500 Hz from the base cutoff.
#[derive(Debug, Clone, Copy)]
pub struct ModulationAmount {
    /// Modulation depth (0.0 to 1.0)
    pub depth: f32,
    /// Whether to invert the modulation signal
    pub inverted: bool,
}

impl ModulationAmount {
    /// Create a new modulation amount.
    pub fn new(depth: f32) -> Self {
        Self {
            depth: depth.clamp(0.0, 1.0),
            inverted: false,
        }
    }

    /// Create an inverted modulation amount.
    pub fn inverted(depth: f32) -> Self {
        Self {
            depth: depth.clamp(0.0, 1.0),
            inverted: true,
        }
    }

    /// Apply modulation to a base value.
    ///
    /// # Arguments
    /// * `base` - The base parameter value
    /// * `mod_value` - The modulation signal (-1 to 1 or 0 to 1)
    /// * `range` - The maximum modulation range (in parameter units)
    pub fn apply(&self, base: f32, mod_value: f32, range: f32) -> f32 {
        let scaled = if self.inverted {
            -mod_value * self.depth * range
        } else {
            mod_value * self.depth * range
        };
        base + scaled
    }
}

impl Default for ModulationAmount {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LfoWaveform;

    #[test]
    fn test_lfo_modulation_source() {
        let mut lfo = Lfo::new(48000.0, 1.0);
        lfo.set_waveform(LfoWaveform::Sine);

        assert!(lfo.is_bipolar());

        let value = lfo.mod_advance();
        assert!((-1.0..=1.0).contains(&value));
    }

    #[test]
    fn test_lfo_unipolar_conversion() {
        let mut lfo = Lfo::new(48000.0, 1.0);

        for _ in 0..1000 {
            let value = lfo.mod_advance_unipolar();
            assert!(
                (0.0..=1.0).contains(&value),
                "Unipolar value {} out of range",
                value
            );
        }
    }

    #[test]
    fn test_modulation_amount() {
        let amount = ModulationAmount::new(0.5);

        // With mod_value = 1.0, depth = 0.5, range = 100
        // Result should be base + 50
        let result = amount.apply(0.0, 1.0, 100.0);
        assert!((result - 50.0).abs() < 0.001);

        // Inverted
        let inverted = ModulationAmount::inverted(0.5);
        let result = inverted.apply(0.0, 1.0, 100.0);
        assert!((result - (-50.0)).abs() < 0.001);
    }

    #[test]
    fn test_bipolar_to_unipolar_conversion() {
        let mut lfo = Lfo::new(48000.0, 10.0);

        // Sample many values
        let mut min = f32::MAX;
        let mut max = f32::MIN;

        for _ in 0..10000 {
            let value = lfo.mod_advance_unipolar();
            min = min.min(value);
            max = max.max(value);
        }

        // Should be in 0..1 range
        assert!(min >= 0.0, "Min {} should be >= 0", min);
        assert!(max <= 1.0, "Max {} should be <= 1", max);
        // And should span most of the range
        assert!(min < 0.1, "Min {} should be near 0", min);
        assert!(max > 0.9, "Max {} should be near 1", max);
    }
}
