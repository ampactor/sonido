//! Distortion effects with multiple anti-aliased waveshaping algorithms.
//!
//! Provides classic distortion/overdrive effects with first-order ADAA
//! (Anti-Derivative Anti-Aliasing) for reduced aliasing at minimal cost.
//!
//! # Signal Flow
//!
//! ```text
//! Input → Drive (gain) → ADAA Waveshaper → Tone EQ → Mix → Soft Limit → Output Level
//! ```
//!
//! # Waveshaping Algorithms
//!
//! | Algorithm | Character | Harmonics | ADAA | Best For |
//! |-----------|-----------|-----------|------|----------|
//! | [`WaveShape::SoftClip`] | Smooth, warm | Odd | Yes | Tube overdrive |
//! | [`WaveShape::HardClip`] | Aggressive | Odd (many) | Yes | Fuzz, transistor |
//! | [`WaveShape::Foldback`] | Complex, buzzy | Even + Odd | No | Synth, experimental |
//! | [`WaveShape::Asymmetric`] | Rich, warm | Even + Odd | Yes | Vintage tube amp |
//!
//! # Anti-Aliasing
//!
//! First-order ADAA reformulates waveshaping as a continuous-time convolution,
//! suppressing aliased harmonics by approximately 6 dB/octave at negligible cost.
//! Foldback mode uses raw waveshaping (no closed-form antiderivative).
//!
//! Reference: Parker et al., "Reducing the Aliasing of Nonlinear Waveshaping
//! Using Continuous-Time Convolution", DAFx-2016.
//!
//! # Parameters
//!
//! - **Drive** (0–40 dB): Input gain before waveshaping.
//! - **Tone** (−12 to +12 dB): Peaking EQ at 1 kHz for tonal shaping.
//! - **Output** (−20 to +20 dB): Output level.
//! - **Waveshape** (0–3): Selects the waveshaping algorithm.
//! - **Mix** (0–100%): Dry/wet blend for parallel distortion.

use sonido_core::math::soft_limit;
use sonido_core::{
    Adaa1, Biquad, Effect, ParamDescriptor, ParamFlags, ParamId, ParamUnit, SmoothedParam,
    asymmetric_clip, asymmetric_clip_ad, db_to_linear, foldback, gain, hard_clip, hard_clip_ad,
    linear_to_db, peaking_eq_coefficients, soft_clip, soft_clip_ad, wet_dry_mix,
    wet_dry_mix_stereo,
};

/// Hard clip with unit threshold, for use as ADAA function pointer.
fn hard_clip_unit(x: f32) -> f32 {
    hard_clip(x, 1.0)
}

/// Antiderivative of hard clip with unit threshold, for use as ADAA function pointer.
fn hard_clip_ad_unit(x: f32) -> f32 {
    hard_clip_ad(x, 1.0)
}

/// ADAA processor type using function pointers.
type AdaaProc = Adaa1<fn(f32) -> f32, fn(f32) -> f32>;

/// Center frequency for the tone peaking EQ.
const TONE_CENTER_HZ: f32 = 1000.0;

/// Q factor for the tone peaking EQ (moderately broad).
const TONE_Q: f32 = 0.7;

/// Waveshaping algorithm selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WaveShape {
    /// Hyperbolic tangent soft clipping — smooth, tube-like.
    /// Anti-aliased via first-order ADAA.
    #[default]
    SoftClip,
    /// Hard clipping at ±1 — aggressive, transistor-like.
    /// Anti-aliased via first-order ADAA.
    HardClip,
    /// Foldback distortion — rich harmonics, synth-style.
    /// No ADAA (no closed-form antiderivative).
    Foldback,
    /// Asymmetric soft clipping — even harmonics, tube-like.
    /// Anti-aliased via first-order ADAA.
    Asymmetric,
}

/// Distortion effect with anti-aliased waveshaping and tone control.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Drive | 0.0–40.0 dB | 12.0 |
/// | 1 | Tone | −12.0–12.0 dB | 0.0 |
/// | 2 | Output | −20.0–20.0 dB | 0.0 |
/// | 3 | Waveshape | 0–3 (SoftClip, HardClip, Foldback, Asymmetric) | 0 |
/// | 4 | Mix | 0–100% | 100.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::{Distortion, WaveShape};
/// use sonido_core::Effect;
///
/// let mut dist = Distortion::new(48000.0);
/// dist.set_drive_db(20.0);
/// dist.set_tone_db(3.0);
/// dist.set_waveshape(WaveShape::SoftClip);
///
/// let output = dist.process(0.1);
/// ```
pub struct Distortion {
    // Smoothed parameters
    drive: SmoothedParam,
    output_level: SmoothedParam,
    mix: SmoothedParam,

    // Settings
    sample_rate: f32,
    tone_gain_db: f32,

    // Waveshaping
    waveshape: WaveShape,
    foldback_threshold: f32,

    // Tone filters (peaking EQ at 1 kHz, one per channel)
    tone_filter: Biquad,
    tone_filter_r: Biquad,

    // ADAA processors (L/R pairs per waveshape mode)
    adaa_soft_l: AdaaProc,
    adaa_soft_r: AdaaProc,
    adaa_hard_l: AdaaProc,
    adaa_hard_r: AdaaProc,
    adaa_asym_l: AdaaProc,
    adaa_asym_r: AdaaProc,
}

impl Distortion {
    /// Create a new distortion effect.
    ///
    /// Defaults: Drive 12 dB, Tone 0 dB, Output 0 dB, SoftClip, Mix 100%.
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            drive: SmoothedParam::fast(db_to_linear(12.0), sample_rate),
            output_level: gain::output_level_param(sample_rate),
            mix: SmoothedParam::standard(1.0, sample_rate),
            sample_rate,
            tone_gain_db: 0.0,
            waveshape: WaveShape::default(),
            foldback_threshold: 0.8,
            tone_filter: Biquad::new(),
            tone_filter_r: Biquad::new(),
            adaa_soft_l: Adaa1::new(soft_clip as fn(f32) -> f32, soft_clip_ad as fn(f32) -> f32),
            adaa_soft_r: Adaa1::new(soft_clip as fn(f32) -> f32, soft_clip_ad as fn(f32) -> f32),
            adaa_hard_l: Adaa1::new(
                hard_clip_unit as fn(f32) -> f32,
                hard_clip_ad_unit as fn(f32) -> f32,
            ),
            adaa_hard_r: Adaa1::new(
                hard_clip_unit as fn(f32) -> f32,
                hard_clip_ad_unit as fn(f32) -> f32,
            ),
            adaa_asym_l: Adaa1::new(
                asymmetric_clip as fn(f32) -> f32,
                asymmetric_clip_ad as fn(f32) -> f32,
            ),
            adaa_asym_r: Adaa1::new(
                asymmetric_clip as fn(f32) -> f32,
                asymmetric_clip_ad as fn(f32) -> f32,
            ),
        };
        s.update_tone_coefficients();
        s
    }

    /// Set drive amount in decibels.
    ///
    /// Range: 0.0 to 40.0 dB. Higher values produce more distortion.
    pub fn set_drive_db(&mut self, db: f32) {
        self.drive.set_target(db_to_linear(db));
    }

    /// Get current drive in dB.
    pub fn drive_db(&self) -> f32 {
        linear_to_db(self.drive.target())
    }

    /// Set tone control gain in dB.
    ///
    /// Range: −12.0 to +12.0 dB. Controls a peaking EQ at 1 kHz.
    /// Positive values brighten, negative values darken the tone.
    pub fn set_tone_db(&mut self, db: f32) {
        self.tone_gain_db = db.clamp(-12.0, 12.0);
        self.update_tone_coefficients();
    }

    /// Get current tone gain in dB.
    pub fn tone_db(&self) -> f32 {
        self.tone_gain_db
    }

    /// Set the waveshaping algorithm.
    pub fn set_waveshape(&mut self, waveshape: WaveShape) {
        self.waveshape = waveshape;
    }

    /// Set foldback threshold (only affects Foldback waveshape).
    ///
    /// Range: 0.1 to 1.0.
    pub fn set_foldback_threshold(&mut self, threshold: f32) {
        self.foldback_threshold = threshold.clamp(0.1, 1.0);
    }

    /// Get current waveshape.
    pub fn waveshape(&self) -> WaveShape {
        self.waveshape
    }

    /// Set wet/dry mix.
    ///
    /// Range: 0.0 (fully dry) to 1.0 (fully wet).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Recalculate tone filter biquad coefficients.
    fn update_tone_coefficients(&mut self) {
        let (b0, b1, b2, a0, a1, a2) =
            peaking_eq_coefficients(TONE_CENTER_HZ, TONE_Q, self.tone_gain_db, self.sample_rate);
        self.tone_filter.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.tone_filter_r.set_coefficients(b0, b1, b2, a0, a1, a2);
    }
}

impl Effect for Distortion {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let drive = self.drive.advance();
        let level = self.output_level.advance();
        let mix_val = self.mix.advance();

        let driven = input * drive;
        let shaped = match self.waveshape {
            WaveShape::SoftClip => self.adaa_soft_l.process(driven),
            WaveShape::HardClip => self.adaa_hard_l.process(driven),
            WaveShape::Foldback => foldback(driven, self.foldback_threshold),
            WaveShape::Asymmetric => self.adaa_asym_l.process(driven),
        };
        let wet = self.tone_filter.process(shaped);
        soft_limit(wet_dry_mix(input, wet, mix_val), 1.0) * level
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.advance();
        let level = self.output_level.advance();
        let mix_val = self.mix.advance();

        let driven_l = left * drive;
        let driven_r = right * drive;

        let (shaped_l, shaped_r) = match self.waveshape {
            WaveShape::SoftClip => (
                self.adaa_soft_l.process(driven_l),
                self.adaa_soft_r.process(driven_r),
            ),
            WaveShape::HardClip => (
                self.adaa_hard_l.process(driven_l),
                self.adaa_hard_r.process(driven_r),
            ),
            WaveShape::Foldback => (
                foldback(driven_l, self.foldback_threshold),
                foldback(driven_r, self.foldback_threshold),
            ),
            WaveShape::Asymmetric => (
                self.adaa_asym_l.process(driven_l),
                self.adaa_asym_r.process(driven_r),
            ),
        };

        let wet_l = self.tone_filter.process(shaped_l);
        let wet_r = self.tone_filter_r.process(shaped_r);

        let (mixed_l, mixed_r) = wet_dry_mix_stereo(left, right, wet_l, wet_r, mix_val);
        (
            soft_limit(mixed_l, 1.0) * level,
            soft_limit(mixed_r, 1.0) * level,
        )
    }

    /// Process a block of stereo samples with per-mode ADAA dispatch.
    ///
    /// Resolves the waveshaping mode once at block start. For SoftClip,
    /// HardClip, and Asymmetric, uses ADAA processors for anti-aliased output.
    /// Foldback uses raw waveshaping (no closed-form antiderivative).
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

        match self.waveshape {
            WaveShape::SoftClip => process_block_adaa(
                &mut self.drive,
                &mut self.output_level,
                &mut self.mix,
                &mut self.tone_filter,
                &mut self.tone_filter_r,
                &mut self.adaa_soft_l,
                &mut self.adaa_soft_r,
                left_in,
                right_in,
                left_out,
                right_out,
            ),
            WaveShape::HardClip => process_block_adaa(
                &mut self.drive,
                &mut self.output_level,
                &mut self.mix,
                &mut self.tone_filter,
                &mut self.tone_filter_r,
                &mut self.adaa_hard_l,
                &mut self.adaa_hard_r,
                left_in,
                right_in,
                left_out,
                right_out,
            ),
            WaveShape::Foldback => {
                let threshold = self.foldback_threshold;
                for i in 0..left_in.len() {
                    let drv = self.drive.advance();
                    let lvl = self.output_level.advance();
                    let mx = self.mix.advance();
                    let (dry_l, dry_r) = (left_in[i], right_in[i]);
                    let wet_l = self.tone_filter.process(foldback(dry_l * drv, threshold));
                    let wet_r = self.tone_filter_r.process(foldback(dry_r * drv, threshold));
                    left_out[i] = soft_limit(wet_dry_mix(dry_l, wet_l, mx), 1.0) * lvl;
                    right_out[i] = soft_limit(wet_dry_mix(dry_r, wet_r, mx), 1.0) * lvl;
                }
            }
            WaveShape::Asymmetric => process_block_adaa(
                &mut self.drive,
                &mut self.output_level,
                &mut self.mix,
                &mut self.tone_filter,
                &mut self.tone_filter_r,
                &mut self.adaa_asym_l,
                &mut self.adaa_asym_r,
                left_in,
                right_in,
                left_out,
                right_out,
            ),
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.drive.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.update_tone_coefficients();
    }

    fn reset(&mut self) {
        self.tone_filter.clear();
        self.tone_filter_r.clear();
        self.drive.snap_to_target();
        self.output_level.snap_to_target();
        self.mix.snap_to_target();
        self.adaa_soft_l.reset();
        self.adaa_soft_r.reset();
        self.adaa_hard_l.reset();
        self.adaa_hard_r.reset();
        self.adaa_asym_l.reset();
        self.adaa_asym_r.reset();
    }
}

/// Block processing loop for ADAA-enabled waveshape modes.
///
/// Extracted as a free function to allow disjoint field borrows from `Distortion`.
#[inline]
#[allow(clippy::too_many_arguments)]
fn process_block_adaa(
    drive: &mut SmoothedParam,
    output_level: &mut SmoothedParam,
    mix: &mut SmoothedParam,
    tone_l: &mut Biquad,
    tone_r: &mut Biquad,
    adaa_l: &mut AdaaProc,
    adaa_r: &mut AdaaProc,
    left_in: &[f32],
    right_in: &[f32],
    left_out: &mut [f32],
    right_out: &mut [f32],
) {
    for i in 0..left_in.len() {
        let drv = drive.advance();
        let lvl = output_level.advance();
        let mx = mix.advance();
        let (dry_l, dry_r) = (left_in[i], right_in[i]);
        let wet_l = tone_l.process(adaa_l.process(dry_l * drv));
        let wet_r = tone_r.process(adaa_r.process(dry_r * drv));
        left_out[i] = soft_limit(wet_dry_mix(dry_l, wet_l, mx), 1.0) * lvl;
        right_out[i] = soft_limit(wet_dry_mix(dry_r, wet_r, mx), 1.0) * lvl;
    }
}

sonido_core::impl_params! {
    Distortion, this {
        [0] ParamDescriptor::gain_db("Drive", "Drive", 0.0, 40.0, 12.0)
                .with_id(ParamId(200), "dist_drive"),
            get: this.drive_db(),
            set: |v| this.set_drive_db(v);

        [1] ParamDescriptor::custom("Tone", "Tone", -12.0, 12.0, 0.0)
                .with_unit(ParamUnit::Decibels)
                .with_step(0.5)
                .with_id(ParamId(201), "dist_tone"),
            get: this.tone_db(),
            set: |v| this.set_tone_db(v);

        [2] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(202), "dist_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);

        [3] ParamDescriptor::custom("Waveshape", "Shape", 0.0, 3.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(203), "dist_shape")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Soft Clip", "Hard Clip", "Foldback", "Asymmetric"]),
            get: this.waveshape as u8 as f32,
            set: |v| {
                this.waveshape = match v as u8 {
                    0 => WaveShape::SoftClip,
                    1 => WaveShape::HardClip,
                    2 => WaveShape::Foldback,
                    _ => WaveShape::Asymmetric,
                }
            };

        [4] ParamDescriptor::custom("Mix", "Mix", 0.0, 100.0, 100.0)
                .with_unit(ParamUnit::Percent)
                .with_step(1.0)
                .with_id(ParamId(204), "dist_mix"),
            get: this.mix.target() * 100.0,
            set: |v| this.set_mix(v / 100.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

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
    fn test_distortion_waveshape_clamp() {
        let mut dist = Distortion::new(48000.0);
        // Out-of-range value should be clamped to max (3.0 = Asymmetric)
        dist.set_param(3, 99.0);
        assert_eq!(dist.get_param(3), 3.0);

        // Negative value should clamp to 0 (SoftClip)
        dist.set_param(3, -5.0);
        assert_eq!(dist.get_param(3), 0.0);
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

    #[test]
    fn test_distortion_adaa_bounded() {
        let mut dist = Distortion::new(48000.0);
        dist.set_drive_db(30.0);
        dist.set_mix(1.0);
        dist.reset();

        // Step input: silence then signal — ADAA should produce bounded output
        for i in 0..128 {
            let input = if i < 64 { 0.0 } else { 0.5 };
            let y = dist.process(input);
            assert!(
                y.is_finite() && y.abs() < 2.0,
                "ADAA output at sample {i}: {y}"
            );
        }
    }

    #[test]
    fn test_distortion_mix_bypass() {
        let mut dist = Distortion::new(48000.0);
        dist.set_drive_db(20.0);
        dist.set_mix(0.0);
        dist.reset();

        // With mix=0, output should be dry signal × output_level (1.0)
        for _ in 0..1000 {
            dist.process(0.3);
        }
        let output = dist.process(0.3);
        assert!(
            (output - 0.3).abs() < 0.05,
            "Mix=0 should pass dry signal, got {output}"
        );
    }

    #[test]
    fn test_distortion_tone_range() {
        let mut dist = Distortion::new(48000.0);
        dist.set_tone_db(12.0);
        assert!((dist.tone_db() - 12.0).abs() < 0.01);
        dist.set_tone_db(-12.0);
        assert!((dist.tone_db() - (-12.0)).abs() < 0.01);
        // Clamping
        dist.set_tone_db(20.0);
        assert!((dist.tone_db() - 12.0).abs() < 0.01);
    }

    #[test]
    fn test_distortion_param_count() {
        let dist = Distortion::new(48000.0);
        assert_eq!(dist.param_count(), 5);
    }

    #[test]
    fn test_distortion_block_stereo() {
        let mut dist = Distortion::new(48000.0);
        dist.set_drive_db(15.0);
        dist.reset();

        let left_in = [0.1; 64];
        let right_in = [0.2; 64];
        let mut left_out = [0.0; 64];
        let mut right_out = [0.0; 64];

        dist.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

        for i in 0..64 {
            assert!(left_out[i].is_finite(), "L[{i}] not finite");
            assert!(right_out[i].is_finite(), "R[{i}] not finite");
        }
    }

    #[test]
    fn test_distortion_all_modes_block() {
        for ws in [
            WaveShape::SoftClip,
            WaveShape::HardClip,
            WaveShape::Foldback,
            WaveShape::Asymmetric,
        ] {
            let mut dist = Distortion::new(48000.0);
            dist.set_drive_db(25.0);
            dist.set_waveshape(ws);
            dist.reset();

            let left_in = [0.3; 32];
            let right_in = [0.4; 32];
            let mut left_out = [0.0; 32];
            let mut right_out = [0.0; 32];

            dist.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

            for i in 0..32 {
                assert!(
                    left_out[i].is_finite() && right_out[i].is_finite(),
                    "{ws:?} block output not finite at {i}"
                );
            }
        }
    }
}
