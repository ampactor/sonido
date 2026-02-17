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

use sonido_core::{
    Effect, OnePole, ParamDescriptor, ParamFlags, ParamId, ParamScale, ParamUnit, ParameterInfo,
    SmoothedParam, asymmetric_clip, db_to_linear, foldback, hard_clip, linear_to_db, soft_clip,
};

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
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Drive | 0.0–40.0 dB | 12.0 |
/// | 1 | Tone | 500.0–10000.0 Hz | 4000.0 |
/// | 2 | Level | -20.0–0.0 dB | -6.0 |
/// | 3 | Waveshape | 0–3 (SoftClip, HardClip, Foldback, Asymmetric) | 0 |
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

    // Current settings
    sample_rate: f32,
    tone_freq_hz: f32,

    // Waveshaping
    waveshape: WaveShape,
    foldback_threshold: f32,

    // Tone filters (one-pole lowpass, one per channel)
    tone_filter: OnePole,
    tone_filter_r: OnePole,
}

impl Distortion {
    /// Create a new distortion effect.
    ///
    /// Defaults: Drive 12dB, Level -6dB, Tone 4kHz, SoftClip
    pub fn new(sample_rate: f32) -> Self {
        Self {
            drive: SmoothedParam::fast(db_to_linear(12.0), sample_rate),
            level: SmoothedParam::fast(db_to_linear(-6.0), sample_rate),
            sample_rate,
            tone_freq_hz: 4000.0,
            waveshape: WaveShape::default(),
            foldback_threshold: 0.8,
            tone_filter: OnePole::new(sample_rate, 4000.0),
            tone_filter_r: OnePole::new(sample_rate, 4000.0),
        }
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
        self.tone_filter.set_frequency(freq_hz);
        self.tone_filter_r.set_frequency(freq_hz);
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

    #[inline]
    fn apply_waveshape(&self, x: f32) -> f32 {
        match self.waveshape {
            WaveShape::SoftClip => soft_clip(x),
            WaveShape::HardClip => hard_clip(x, 1.0),
            WaveShape::Foldback => foldback(x, self.foldback_threshold),
            WaveShape::Asymmetric => asymmetric_clip(x),
        }
    }

    /// Inner block-processing loop, generic over the waveshaping function.
    ///
    /// Monomorphization produces a specialized loop per waveshape variant,
    /// eliminating per-sample branching and enabling autovectorization.
    /// The operation order matches [`process_stereo`](Effect::process_stereo)
    /// exactly: advance drive, advance level, then process L then R.
    #[inline]
    fn process_block_stereo_inner<F: Fn(f32) -> f32>(
        &mut self,
        left_in: &[f32],
        right_in: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
        shaper: F,
    ) {
        for i in 0..left_in.len() {
            let drive = self.drive.advance();
            let level = self.level.advance();

            // Left channel
            let driven_l = left_in[i] * drive;
            let shaped_l = shaper(driven_l);
            let filtered_l = self.tone_filter.process(shaped_l);
            left_out[i] = filtered_l * level;

            // Right channel (separate filter state)
            let driven_r = right_in[i] * drive;
            let shaped_r = shaper(driven_r);
            let filtered_r = self.tone_filter_r.process(shaped_r);
            right_out[i] = filtered_r * level;
        }
    }
}

impl Effect for Distortion {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let drive = self.drive.advance();
        let level = self.level.advance();

        let driven = input * drive;
        let shaped = self.apply_waveshape(driven);
        let filtered = self.tone_filter.process(shaped);

        filtered * level
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Dual-mono: process each channel independently with separate filter states
        let drive = self.drive.advance();
        let level = self.level.advance();

        // Process left channel
        let driven_l = left * drive;
        let shaped_l = self.apply_waveshape(driven_l);
        let filtered_l = self.tone_filter.process(shaped_l);
        let out_l = filtered_l * level;

        // Process right channel with separate filter state
        let driven_r = right * drive;
        let shaped_r = self.apply_waveshape(driven_r);
        let filtered_r = self.tone_filter_r.process(shaped_r);
        let out_r = filtered_r * level;

        (out_l, out_r)
    }

    /// Process a block of stereo samples with optimized waveshaper dispatch.
    ///
    /// Resolves the waveshaping function once at block start, eliminating
    /// per-sample branching. The output is bit-identical to calling
    /// [`process_stereo`](Effect::process_stereo) per sample.
    ///
    /// Both channels share drive/level parameters (dual-mono), so
    /// `SmoothedParam::advance()` is called once per sample pair.
    fn process_block_stereo(
        &mut self,
        left_in: &[f32],
        right_in: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        debug_assert_eq!(left_in.len(), right_in.len());
        debug_assert_eq!(left_in.len(), left_out.len());
        debug_assert_eq!(left_out.len(), right_out.len());

        // Resolve waveshaper once at block start — avoids per-sample match.
        // Each variant gets a monomorphized loop via the generic helper,
        // enabling autovectorization of the waveshaping arithmetic.
        let threshold = self.foldback_threshold;
        match self.waveshape {
            WaveShape::SoftClip => {
                self.process_block_stereo_inner(left_in, right_in, left_out, right_out, soft_clip);
            }
            WaveShape::HardClip => {
                self.process_block_stereo_inner(left_in, right_in, left_out, right_out, |x| {
                    hard_clip(x, 1.0)
                });
            }
            WaveShape::Foldback => {
                self.process_block_stereo_inner(left_in, right_in, left_out, right_out, |x| {
                    foldback(x, threshold)
                });
            }
            WaveShape::Asymmetric => {
                self.process_block_stereo_inner(
                    left_in,
                    right_in,
                    left_out,
                    right_out,
                    asymmetric_clip,
                );
            }
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.drive.set_sample_rate(sample_rate);
        self.level.set_sample_rate(sample_rate);
        self.tone_filter.set_sample_rate(sample_rate);
        self.tone_filter_r.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.tone_filter.reset();
        self.tone_filter_r.reset();
        self.drive.snap_to_target();
        self.level.snap_to_target();
    }
}

impl ParameterInfo for Distortion {
    fn param_count(&self) -> usize {
        4
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(
                ParamDescriptor::gain_db("Drive", "Drive", 0.0, 40.0, 12.0)
                    .with_id(ParamId(200), "dist_drive"),
            ),
            1 => Some(
                ParamDescriptor {
                    name: "Tone",
                    short_name: "Tone",
                    unit: ParamUnit::Hertz,
                    min: 500.0,
                    max: 10000.0,
                    default: 4000.0,
                    step: 100.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(201), "dist_tone")
                .with_scale(ParamScale::Logarithmic),
            ),
            2 => Some(
                ParamDescriptor::gain_db("Level", "Level", -20.0, 0.0, -6.0)
                    .with_id(ParamId(202), "dist_level"),
            ),
            3 => Some(
                ParamDescriptor {
                    name: "Waveshape",
                    short_name: "Shape",
                    unit: ParamUnit::None,
                    min: 0.0,
                    max: 3.0,
                    default: 0.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(203), "dist_shape")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&[
                    "Soft Clip",
                    "Hard Clip",
                    "Foldback",
                    "Asymmetric",
                ]),
            ),
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

        for ws in [
            WaveShape::SoftClip,
            WaveShape::HardClip,
            WaveShape::Foldback,
            WaveShape::Asymmetric,
        ] {
            dist.set_waveshape(ws);
            dist.reset();
            let output = dist.process(0.1);
            assert!(
                output.is_finite(),
                "Waveshape {:?} produced invalid output",
                ws
            );
        }
    }
}
