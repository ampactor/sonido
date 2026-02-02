//! Distortion effects with multiple waveshaping algorithms.
//!
//! This module provides classic distortion/overdrive effects suitable for
//! guitar, synthesizer, and general audio processing.
//!
//! # Signal Flow
//!
//! ```text
//! Input → Drive (gain) → Waveshaper → Tone Filter → Output Level
//! ```
//!
//! # Waveshaping Algorithms
//!
//! | Algorithm | Character | Harmonics | Best For |
//! |-----------|-----------|-----------|----------|
//! | [`WaveShape::SoftClip`] | Smooth, warm | Odd | Tube overdrive |
//! | [`WaveShape::HardClip`] | Aggressive | Odd (many) | Fuzz, transistor |
//! | [`WaveShape::Foldback`] | Complex, buzzy | Even + Odd | Synth, experimental |
//! | [`WaveShape::Asymmetric`] | Rich, warm | Even + Odd | Vintage tube amp |
//!
//! # Parameters
//!
//! - **Drive** (0-40 dB): Input gain before waveshaping. Higher = more distortion.
//! - **Tone** (500-10000 Hz): One-pole lowpass to tame harsh highs.
//! - **Level** (-20-0 dB): Output level compensation.

use sonido_core::{Effect, SmoothedParam, ParameterInfo, ParamDescriptor, ParamUnit, db_to_linear, linear_to_db, soft_clip, hard_clip, foldback, asymmetric_clip};
use libm::expf;

/// Waveshaping algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaveShape {
    /// Hyperbolic tangent soft clipping - smooth, tube-like
    #[default]
    SoftClip,
    /// Hard clipping at ±1 - aggressive, transistor-like
    HardClip,
    /// Foldback distortion - rich harmonics, synth-style
    Foldback,
    /// Asymmetric soft clipping - even harmonics, tube-like
    Asymmetric,
}

/// Distortion effect with waveshaping and tone control.
///
/// # Example
///
/// ```rust
/// use sonido_effects::{Distortion, WaveShape};
/// use sonido_core::Effect;
///
/// let mut dist = Distortion::new(48000.0);
/// dist.set_drive_db(20.0);
/// dist.set_tone_hz(4000.0);
/// dist.set_level_db(-12.0);
/// dist.set_waveshape(WaveShape::SoftClip);
///
/// let output = dist.process(0.1);
/// ```
pub struct Distortion {
    // Parameters with smoothing
    drive: SmoothedParam,
    level: SmoothedParam,
    tone_coeff: SmoothedParam,

    // Current settings
    sample_rate: f32,
    tone_freq_hz: f32,

    // Waveshaping
    waveshape: WaveShape,
    foldback_threshold: f32,

    // Filter state
    tone_filter_state: f32,
}

impl Distortion {
    /// Create a new distortion effect.
    ///
    /// Defaults: Drive 0dB, Level 0dB, Tone 8kHz, SoftClip
    pub fn new(sample_rate: f32) -> Self {
        let mut dist = Self {
            drive: SmoothedParam::with_config(1.0, sample_rate, 5.0),
            level: SmoothedParam::with_config(1.0, sample_rate, 5.0),
            tone_coeff: SmoothedParam::with_config(0.0, sample_rate, 5.0),
            sample_rate,
            tone_freq_hz: 8000.0,
            waveshape: WaveShape::default(),
            foldback_threshold: 0.8,
            tone_filter_state: 0.0,
        };
        dist.recalculate_tone_coeff();
        dist
    }

    /// Set drive amount in decibels.
    pub fn set_drive_db(&mut self, db: f32) {
        self.drive.set_target(db_to_linear(db));
    }

    /// Set output level in decibels.
    pub fn set_level_db(&mut self, db: f32) {
        self.level.set_target(db_to_linear(db));
    }

    /// Set tone control frequency in Hz.
    pub fn set_tone_hz(&mut self, freq_hz: f32) {
        self.tone_freq_hz = freq_hz;
        self.recalculate_tone_coeff();
    }

    /// Set the waveshaping algorithm.
    pub fn set_waveshape(&mut self, waveshape: WaveShape) {
        self.waveshape = waveshape;
    }

    /// Set foldback threshold (only affects Foldback waveshape).
    pub fn set_foldback_threshold(&mut self, threshold: f32) {
        self.foldback_threshold = threshold.clamp(0.1, 1.0);
    }

    /// Get current drive in dB.
    pub fn drive_db(&self) -> f32 {
        linear_to_db(self.drive.target())
    }

    /// Get current level in dB.
    pub fn level_db(&self) -> f32 {
        linear_to_db(self.level.target())
    }

    /// Get current tone frequency in Hz.
    pub fn tone_hz(&self) -> f32 {
        self.tone_freq_hz
    }

    /// Get current waveshape.
    pub fn waveshape(&self) -> WaveShape {
        self.waveshape
    }

    fn recalculate_tone_coeff(&mut self) {
        let normalized = self.tone_freq_hz / self.sample_rate;
        let coeff = 1.0 - expf(-core::f32::consts::TAU * normalized);
        self.tone_coeff.set_target(coeff);
    }

    #[inline]
    fn apply_waveshape(&self, x: f32) -> f32 {
        match self.waveshape {
            WaveShape::SoftClip => soft_clip(x),
            WaveShape::HardClip => hard_clip(x, 1.0),
            WaveShape::Foldback => foldback(x, self.foldback_threshold),
            WaveShape::Asymmetric => asymmetric_clip(x),
        }
    }

    #[inline]
    fn tone_filter(&mut self, input: f32, coeff: f32) -> f32 {
        self.tone_filter_state += coeff * (input - self.tone_filter_state);
        self.tone_filter_state
    }
}

impl Effect for Distortion {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let drive = self.drive.advance();
        let level = self.level.advance();
        let tone_coeff = self.tone_coeff.advance();

        let driven = input * drive;
        let shaped = self.apply_waveshape(driven);
        let filtered = self.tone_filter(shaped, tone_coeff);

        filtered * level
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.drive.set_sample_rate(sample_rate);
        self.level.set_sample_rate(sample_rate);
        self.tone_coeff.set_sample_rate(sample_rate);
        self.recalculate_tone_coeff();
    }

    fn reset(&mut self) {
        self.tone_filter_state = 0.0;
        self.drive.snap_to_target();
        self.level.snap_to_target();
        self.tone_coeff.snap_to_target();
    }
}

impl ParameterInfo for Distortion {
    fn param_count(&self) -> usize {
        4
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Drive",
                short_name: "Drive",
                unit: ParamUnit::Decibels,
                min: 0.0,
                max: 40.0,
                default: 12.0,
                step: 0.5,
            }),
            1 => Some(ParamDescriptor {
                name: "Tone",
                short_name: "Tone",
                unit: ParamUnit::Hertz,
                min: 500.0,
                max: 10000.0,
                default: 4000.0,
                step: 100.0,
            }),
            2 => Some(ParamDescriptor {
                name: "Level",
                short_name: "Level",
                unit: ParamUnit::Decibels,
                min: -20.0,
                max: 0.0,
                default: -6.0,
                step: 0.5,
            }),
            3 => Some(ParamDescriptor {
                name: "Waveshape",
                short_name: "Shape",
                unit: ParamUnit::None,
                min: 0.0,
                max: 3.0,
                default: 0.0,
                step: 1.0,
            }),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.drive_db(),
            1 => self.tone_hz(),
            2 => self.level_db(),
            3 => self.waveshape as u8 as f32,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_drive_db(value.clamp(0.0, 40.0)),
            1 => self.set_tone_hz(value.clamp(500.0, 10000.0)),
            2 => self.set_level_db(value.clamp(-20.0, 0.0)),
            3 => {
                let shape = match value as u8 {
                    0 => WaveShape::SoftClip,
                    1 => WaveShape::HardClip,
                    2 => WaveShape::Foldback,
                    _ => WaveShape::Asymmetric,
                };
                self.set_waveshape(shape);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distortion_basic() {
        let mut dist = Distortion::new(48000.0);
        dist.set_drive_db(20.0);
        dist.reset();

        for _ in 0..100 {
            let output = dist.process(0.1);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_distortion_waveshapes() {
        let mut dist = Distortion::new(48000.0);
        dist.set_drive_db(20.0);

        for ws in [WaveShape::SoftClip, WaveShape::HardClip, WaveShape::Foldback, WaveShape::Asymmetric] {
            dist.set_waveshape(ws);
            dist.reset();
            let output = dist.process(0.1);
            assert!(output.is_finite(), "Waveshape {:?} produced invalid output", ws);
        }
    }
}
