//! Dynamics compressor with soft-knee characteristics and program-dependent release.
//!
//! A feed-forward compressor that reduces dynamic range by attenuating
//! signals above a threshold. Features sidechain HPF, dual detection modes,
//! program-dependent release (Giannoulis et al., 2012), and auto makeup gain.
//!
//! # Signal Flow
//!
//! ```text
//! Input → Sidechain HPF → Dual Envelope → Gain Computer → Gain Reduction → Output
//!              ↑              (fast/slow)        ↓
//!        Detection only                   Makeup (auto/manual)
//!                                               ↓
//!                                         Output Level
//! ```
//!
//! # Program-Dependent Release
//!
//! Two parallel envelope followers run on the detection signal:
//! - **Fast** (50 ms release): catches transients quickly
//! - **Slow** (user release time): provides smooth sustained compression
//!
//! The actual envelope is `max(fast, slow)`, so sustained material uses the
//! slow time constant while new transients are caught by the fast follower
//! even during slow release.
//!
//! Reference: Giannoulis, Massberg & Reiss, "Digital Dynamic Range Compressor
//! Design — A Tutorial and Analysis", JAES 2012.
//!
//! # Parameters
//!
//! | Parameter | Range | Description |
//! |-----------|-------|-------------|
//! | Threshold | -60 to 0 dB | Level where compression begins |
//! | Ratio | 1:1 to 20:1 | Compression strength (∞:1 = limiter) |
//! | Attack | 0.1–100 ms | How fast gain reduction engages |
//! | Release | 10–1000 ms | How fast gain reduction releases (slow envelope) |
//! | Makeup | 0–24 dB | Manual output compensation |
//! | Knee | 0–12 dB | Soft-knee width |
//! | Detection | Peak/RMS | Envelope detection mode |
//! | SC HPF | 20–500 Hz | Sidechain high-pass filter frequency |
//! | Auto Makeup | Off/On | Automatic makeup gain from threshold/ratio |
//! | Output | -20 to +20 dB | Output level |
//! | Mix | 0–100% | Parallel compression blend (dry/compressed) |
//!
//! # Tips
//!
//! - **Fast attack** (< 5ms): Catches transients, can sound "squashed"
//! - **Slow attack** (> 20ms): Lets transients through, more natural
//! - **Fast release** (< 100ms): Pumping effect, good for drums
//! - **Slow release** (> 200ms): Smooth, transparent compression
//! - **RMS mode**: Smoother response, better for bus compression
//! - **Sidechain HPF**: Prevents bass from pumping the compressor

use sonido_core::{
    Biquad, DetectionMode, Effect, EnvelopeFollower, ParamDescriptor, ParamFlags, ParamId,
    ParamScale, ParamUnit, SmoothedParam, db_to_linear, fast_db_to_linear, fast_linear_to_db, gain,
    highpass_coefficients, linear_to_db, math::soft_limit, wet_dry_mix, wet_dry_mix_stereo,
};

/// Fixed fast release time for program-dependent release (ms).
const FAST_RELEASE_MS: f32 = 50.0;

/// Default sidechain HPF frequency (Hz).
const DEFAULT_SC_FREQ: f32 = 80.0;

/// Butterworth Q for sidechain HPF (1/√2 ≈ 0.707).
const SC_HPF_Q: f32 = 0.707;

/// Gain computer for calculating compression curve.
#[derive(Debug, Clone)]
struct GainComputer {
    threshold_db: f32,
    ratio: f32,
    knee_db: f32,
}

impl GainComputer {
    fn new() -> Self {
        Self {
            threshold_db: -18.0,
            ratio: 4.0,
            knee_db: 6.0,
        }
    }

    #[inline]
    fn compute_gain_db(&self, input_db: f32) -> f32 {
        let overshoot = input_db - self.threshold_db;

        if overshoot <= -self.knee_db / 2.0 {
            0.0
        } else if overshoot > self.knee_db / 2.0 {
            let gain_reduction = overshoot * (1.0 - 1.0 / self.ratio);
            -gain_reduction
        } else {
            let knee_factor = (overshoot + self.knee_db / 2.0) / self.knee_db;
            let gain_reduction = knee_factor * knee_factor * overshoot * (1.0 - 1.0 / self.ratio);
            -gain_reduction
        }
    }
}

/// Dynamics compressor effect with program-dependent release.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Threshold | -60.0–0.0 dB | -18.0 |
/// | 1 | Ratio | 1.0–20.0 | 4.0 |
/// | 2 | Attack | 0.1–100.0 ms | 10.0 |
/// | 3 | Release | 10.0–1000.0 ms | 100.0 |
/// | 4 | Makeup Gain | 0.0–24.0 dB | 0.0 |
/// | 5 | Knee | 0.0–12.0 dB | 6.0 |
/// | 6 | Detection | 0–1 (Peak/RMS) | 0 (Peak) |
/// | 7 | SC HPF Freq | 20.0–500.0 Hz | 80.0 |
/// | 8 | Auto Makeup | 0–1 (Off/On) | 0 (Off) |
/// | 9 | Output | -20.0–20.0 dB | 0.0 |
/// | 10 | Mix | 0–100% | 100.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Compressor;
/// use sonido_core::Effect;
///
/// let mut comp = Compressor::new(44100.0);
/// comp.set_threshold_db(-20.0);
/// comp.set_ratio(4.0);
/// comp.set_attack_ms(5.0);
/// comp.set_release_ms(50.0);
///
/// let input = 0.5;
/// let output = comp.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Compressor {
    /// Slow envelope follower (user release time).
    envelope_follower: EnvelopeFollower,
    /// Fast envelope follower (50 ms release) for program-dependent release.
    fast_envelope: EnvelopeFollower,
    gain_computer: GainComputer,
    makeup_gain: SmoothedParam,
    output_level: SmoothedParam,
    /// Sidechain high-pass filter for detection path.
    sidechain_hpf: Biquad,
    /// Current sidechain HPF frequency in Hz.
    sidechain_freq: f32,
    /// Auto makeup gain enabled.
    auto_makeup: bool,
    /// Parallel compression mix (0.0 = dry, 1.0 = full compression).
    mix: f32,
    sample_rate: f32,
    /// Last computed gain reduction in dB (always non-positive).
    last_gain_reduction_db: f32,
}

impl Compressor {
    /// Create a new compressor with default settings.
    ///
    /// Defaults: threshold -18 dB, ratio 4:1, attack 10 ms, release 100 ms,
    /// makeup 0 dB, knee 6 dB, peak detection, sidechain HPF 80 Hz,
    /// auto makeup off, output 0 dB.
    pub fn new(sample_rate: f32) -> Self {
        let mut sidechain_hpf = Biquad::new();
        let (b0, b1, b2, a0, a1, a2) =
            highpass_coefficients(DEFAULT_SC_FREQ, SC_HPF_Q, sample_rate);
        sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);

        let mut fast_env = EnvelopeFollower::new(sample_rate);
        fast_env.set_release_ms(FAST_RELEASE_MS);

        Self {
            envelope_follower: EnvelopeFollower::new(sample_rate),
            fast_envelope: fast_env,
            gain_computer: GainComputer::new(),
            makeup_gain: SmoothedParam::standard(1.0, sample_rate),
            output_level: gain::output_level_param(sample_rate),
            sidechain_hpf,
            sidechain_freq: DEFAULT_SC_FREQ,
            auto_makeup: false,
            mix: 1.0,
            sample_rate,
            last_gain_reduction_db: 0.0,
        }
    }

    /// Set threshold in dB.
    ///
    /// Range: -60.0 to 0.0 dB. Values are clamped.
    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.gain_computer.threshold_db = threshold_db.clamp(-60.0, 0.0);
    }

    /// Set compression ratio.
    ///
    /// Range: 1.0 to 20.0. A ratio of 1:1 means no compression;
    /// 20:1 approaches limiting.
    pub fn set_ratio(&mut self, ratio: f32) {
        self.gain_computer.ratio = ratio.clamp(1.0, 20.0);
    }

    /// Set attack time in milliseconds.
    ///
    /// Range: 0.1 to 100.0 ms. Sets both slow and fast envelope attack times.
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        let clamped = attack_ms.clamp(0.1, 100.0);
        self.envelope_follower.set_attack_ms(clamped);
        self.fast_envelope.set_attack_ms(clamped);
    }

    /// Set release time in milliseconds.
    ///
    /// Range: 10.0 to 1000.0 ms. Controls the slow envelope's release.
    /// The fast envelope always uses a fixed 50 ms release for
    /// program-dependent transient handling.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.envelope_follower
            .set_release_ms(release_ms.clamp(10.0, 1000.0));
        // fast_envelope keeps its fixed FAST_RELEASE_MS
    }

    /// Set knee width in dB.
    ///
    /// Range: 0.0 to 12.0 dB. 0 = hard knee, 6+ = soft knee.
    pub fn set_knee_db(&mut self, knee_db: f32) {
        self.gain_computer.knee_db = knee_db.clamp(0.0, 12.0);
    }

    /// Set makeup gain in dB.
    ///
    /// Range: 0.0 to 24.0 dB. Ignored when auto makeup is enabled.
    pub fn set_makeup_gain_db(&mut self, gain_db: f32) {
        let linear = db_to_linear(gain_db.clamp(0.0, 24.0));
        self.makeup_gain.set_target(linear);
    }

    /// Set the detection mode (Peak or RMS).
    ///
    /// Peak tracks instantaneous amplitude; RMS tracks signal power
    /// for a smoother, more averaged response.
    pub fn set_detection_mode(&mut self, mode: DetectionMode) {
        self.envelope_follower.set_detection_mode(mode);
        self.fast_envelope.set_detection_mode(mode);
    }

    /// Set sidechain high-pass filter frequency.
    ///
    /// Range: 20.0 to 500.0 Hz. Removes low-frequency content from
    /// the detection path to prevent bass from pumping the compressor.
    /// Set to 20 Hz to effectively disable.
    pub fn set_sidechain_freq(&mut self, freq_hz: f32) {
        self.sidechain_freq = freq_hz.clamp(20.0, 500.0);
        let (b0, b1, b2, a0, a1, a2) =
            highpass_coefficients(self.sidechain_freq, SC_HPF_Q, self.sample_rate);
        self.sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);
    }

    /// Set auto makeup gain mode.
    ///
    /// When enabled, makeup gain is computed automatically from
    /// threshold and ratio: `makeup_dB = -threshold × (1 - 1/ratio) × 0.5`.
    /// The manual makeup gain parameter is ignored while auto is active.
    pub fn set_auto_makeup(&mut self, enabled: bool) {
        self.auto_makeup = enabled;
    }

    /// Set parallel compression mix.
    ///
    /// Range: 0.0 (fully dry / no compression) to 1.0 (fully compressed).
    /// At intermediate values, the compressed signal is blended with the
    /// dry input for New York-style parallel compression.
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Get current mix value.
    pub fn mix(&self) -> f32 {
        self.mix
    }

    /// Returns the last computed gain reduction in dB (always non-positive).
    ///
    /// A value of 0.0 means no compression is occurring. A value of -6.0
    /// means the signal is being reduced by 6 dB.
    pub fn gain_reduction_db(&self) -> f32 {
        self.last_gain_reduction_db
    }

    /// Compute auto makeup gain in linear domain.
    ///
    /// Formula: `makeup_dB = -threshold × (1 - 1/ratio) × 0.5`
    ///
    /// The factor of 0.5 accounts for the fact that not all signal is
    /// above threshold, providing a reasonable average compensation.
    #[inline]
    fn auto_makeup_linear(&self) -> f32 {
        let auto_db =
            -self.gain_computer.threshold_db * (1.0 - 1.0 / self.gain_computer.ratio) * 0.5;
        fast_db_to_linear(auto_db)
    }

    /// Run dual-envelope detection on a mono detection signal.
    ///
    /// Returns the program-dependent envelope: `max(fast, slow)`.
    #[inline]
    fn dual_envelope(&mut self, detection: f32) -> f32 {
        let slow = self.envelope_follower.process(detection);
        let fast = self.fast_envelope.process(detection);
        if fast > slow { fast } else { slow }
    }
}

impl Default for Compressor {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Compressor {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // Sidechain HPF: filter detection signal, not audio path
        let detection = self.sidechain_hpf.process(input);
        let envelope = self.dual_envelope(detection);

        let envelope_db = fast_linear_to_db(envelope);
        let gain_reduction_db = self.gain_computer.compute_gain_db(envelope_db);
        self.last_gain_reduction_db = gain_reduction_db;
        let gain_linear = fast_db_to_linear(gain_reduction_db);

        let manual_makeup = self.makeup_gain.advance();
        let makeup = if self.auto_makeup {
            self.auto_makeup_linear()
        } else {
            manual_makeup
        };

        let compressed = input * gain_linear * makeup;
        let mixed = wet_dry_mix(input, compressed, self.mix);
        let level = self.output_level.advance();
        soft_limit(mixed * level, 1.0)
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Linked stereo: detect envelope from mid signal, apply same gain.
        // Prevents image shifting from independent L/R compression.
        let sum = (left + right) * 0.5;
        let detection = self.sidechain_hpf.process(sum);
        let envelope = self.dual_envelope(detection);

        let envelope_db = fast_linear_to_db(envelope);
        let gain_reduction_db = self.gain_computer.compute_gain_db(envelope_db);
        self.last_gain_reduction_db = gain_reduction_db;
        let gain_linear = fast_db_to_linear(gain_reduction_db);

        let manual_makeup = self.makeup_gain.advance();
        let makeup = if self.auto_makeup {
            self.auto_makeup_linear()
        } else {
            manual_makeup
        };

        let comp_gain = gain_linear * makeup;
        let (mixed_l, mixed_r) =
            wet_dry_mix_stereo(left, right, left * comp_gain, right * comp_gain, self.mix);
        let level = self.output_level.advance();
        (
            soft_limit(mixed_l * level, 1.0),
            soft_limit(mixed_r * level, 1.0),
        )
    }

    /// Process a block of stereo samples with linked stereo detection.
    ///
    /// Optimized block override that eliminates vtable dispatch overhead and
    /// enables compiler inlining of the full processing chain. Produces
    /// bit-identical output to calling [`process_stereo`](Effect::process_stereo)
    /// per sample.
    ///
    /// The envelope is derived from the mid signal `(L + R) / 2` so that
    /// both channels receive identical gain reduction, preventing stereo
    /// image shift.
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

        // Hoist gain computer constants out of the loop
        let threshold_db = self.gain_computer.threshold_db;
        let ratio = self.gain_computer.ratio;
        let knee_db = self.gain_computer.knee_db;
        let half_knee = knee_db / 2.0;
        let inv_ratio_complement = 1.0 - 1.0 / ratio;

        let is_auto_makeup = self.auto_makeup;
        let auto_makeup_lin = if is_auto_makeup {
            self.auto_makeup_linear()
        } else {
            1.0 // unused
        };
        let mix = self.mix;

        for i in 0..left_in.len() {
            let left = left_in[i];
            let right = right_in[i];

            // Linked stereo detection: sidechain HPF on mid signal
            let sum = (left + right) * 0.5;
            let detection = self.sidechain_hpf.process(sum);
            let envelope = self.dual_envelope(detection);

            // Gain computer (inlined from GainComputer::compute_gain_db)
            let envelope_db = fast_linear_to_db(envelope);
            let overshoot = envelope_db - threshold_db;

            let gain_reduction_db = if overshoot <= -half_knee {
                0.0
            } else if overshoot > half_knee {
                -(overshoot * inv_ratio_complement)
            } else {
                let knee_factor = (overshoot + half_knee) / knee_db;
                -(knee_factor * knee_factor * overshoot * inv_ratio_complement)
            };

            self.last_gain_reduction_db = gain_reduction_db;
            let gain_linear = fast_db_to_linear(gain_reduction_db);

            let manual_makeup = self.makeup_gain.advance();
            let makeup = if is_auto_makeup {
                auto_makeup_lin
            } else {
                manual_makeup
            };

            let comp_gain = gain_linear * makeup;
            let (mixed_l, mixed_r) =
                wet_dry_mix_stereo(left, right, left * comp_gain, right * comp_gain, mix);
            let level = self.output_level.advance();
            left_out[i] = soft_limit(mixed_l * level, 1.0);
            right_out[i] = soft_limit(mixed_r * level, 1.0);
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.envelope_follower.set_sample_rate(sample_rate);
        self.fast_envelope.set_sample_rate(sample_rate);
        self.makeup_gain.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        // Recalculate sidechain HPF coefficients for new sample rate
        let (b0, b1, b2, a0, a1, a2) =
            highpass_coefficients(self.sidechain_freq, SC_HPF_Q, sample_rate);
        self.sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);
    }

    fn reset(&mut self) {
        self.envelope_follower.reset();
        self.fast_envelope.reset();
        self.makeup_gain.snap_to_target();
        self.output_level.snap_to_target();
        self.sidechain_hpf.clear();
        self.last_gain_reduction_db = 0.0;
    }
}

sonido_core::impl_params! {
    Compressor, this {
        [0] ParamDescriptor::gain_db("Threshold", "Thresh", -60.0, 0.0, -18.0)
                .with_id(ParamId(300), "comp_thresh"),
            get: this.gain_computer.threshold_db,
            set: |v| this.set_threshold_db(v);

        [1] ParamDescriptor::custom("Ratio", "Ratio", 1.0, 20.0, 4.0)
                .with_unit(ParamUnit::Ratio)
                .with_step(0.1)
                .with_id(ParamId(301), "comp_ratio"),
            get: this.gain_computer.ratio,
            set: |v| this.set_ratio(v);

        [2] ParamDescriptor::custom("Attack", "Attack", 0.1, 100.0, 10.0)
                .with_unit(ParamUnit::Milliseconds)
                .with_step(0.1)
                .with_id(ParamId(302), "comp_attack"),
            get: this.envelope_follower.attack_ms(),
            set: |v| this.set_attack_ms(v);

        [3] ParamDescriptor::time_ms("Release", "Release", 10.0, 1000.0, 100.0)
                .with_id(ParamId(303), "comp_release"),
            get: this.envelope_follower.release_ms(),
            set: |v| this.set_release_ms(v);

        [4] ParamDescriptor::gain_db("Makeup Gain", "Makeup", 0.0, 24.0, 0.0)
                .with_id(ParamId(304), "comp_makeup"),
            get: linear_to_db(this.makeup_gain.target()),
            set: |v| this.set_makeup_gain_db(v);

        [5] ParamDescriptor::gain_db("Knee", "Knee", 0.0, 12.0, 6.0)
                .with_id(ParamId(305), "comp_knee"),
            get: this.gain_computer.knee_db,
            set: |v| this.set_knee_db(v);

        [6] ParamDescriptor::custom("Detection", "Detect", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(306), "comp_detect")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Peak", "RMS"]),
            get: this.envelope_follower.detection_mode() as u8 as f32,
            set: |v| {
                let mode = if v < 0.5 {
                    DetectionMode::Peak
                } else {
                    DetectionMode::Rms
                };
                this.set_detection_mode(mode);
            };

        [7] ParamDescriptor::custom("SC HPF Freq", "SC HPF", 20.0, 500.0, 80.0)
                .with_unit(ParamUnit::Hertz)
                .with_step(1.0)
                .with_id(ParamId(307), "comp_sc_freq")
                .with_scale(ParamScale::Logarithmic),
            get: this.sidechain_freq,
            set: |v| this.set_sidechain_freq(v);

        [8] ParamDescriptor::custom("Auto Makeup", "AutoMU", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(308), "comp_auto_makeup")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.auto_makeup { 1.0 } else { 0.0 },
            set: |v| this.set_auto_makeup(v >= 0.5);

        [9] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(309), "comp_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);

        [10] ParamDescriptor { default: 100.0, ..ParamDescriptor::mix() }
                .with_id(ParamId(310), "comp_mix"),
            get: this.mix * 100.0,
            set: |v| this.set_mix(v / 100.0);
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::{vec, vec::Vec};
    use sonido_core::ParameterInfo;

    #[test]
    fn test_compressor_basic() {
        let mut comp = Compressor::new(44100.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);

        for _ in 0..100 {
            let output = comp.process(0.1);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_compressor_block_stereo_matches_per_sample() {
        let sample_rate = 48000.0;

        // Build two identical compressors
        let mut comp_ref = Compressor::new(sample_rate);
        comp_ref.set_threshold_db(-12.0);
        comp_ref.set_ratio(6.0);
        comp_ref.set_attack_ms(5.0);
        comp_ref.set_release_ms(80.0);
        comp_ref.set_makeup_gain_db(3.0);
        comp_ref.set_knee_db(4.0);

        let mut comp_block = comp_ref.clone();

        // Generate test signal: stereo sine-ish sweep with asymmetric levels
        let n = 512;
        let left_in: Vec<f32> = (0..n).map(|i| libm::sinf(i as f32 * 0.05) * 0.8).collect();
        let right_in: Vec<f32> = (0..n).map(|i| libm::cosf(i as f32 * 0.07) * 0.6).collect();

        // Per-sample reference
        let mut left_ref = vec![0.0f32; n];
        let mut right_ref = vec![0.0f32; n];
        for i in 0..n {
            let (l, r) = comp_ref.process_stereo(left_in[i], right_in[i]);
            left_ref[i] = l;
            right_ref[i] = r;
        }

        // Block processing
        let mut left_out = vec![0.0f32; n];
        let mut right_out = vec![0.0f32; n];
        comp_block.process_block_stereo(&left_in, &right_in, &mut left_out, &mut right_out);

        // Must be bit-identical
        for i in 0..n {
            assert_eq!(
                left_out[i].to_bits(),
                left_ref[i].to_bits(),
                "Left mismatch at sample {i}: block={} ref={}",
                left_out[i],
                left_ref[i],
            );
            assert_eq!(
                right_out[i].to_bits(),
                right_ref[i].to_bits(),
                "Right mismatch at sample {i}: block={} ref={}",
                right_out[i],
                right_ref[i],
            );
        }
    }

    #[test]
    fn test_compressor_reduces_peaks() {
        let mut comp = Compressor::new(44100.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);
        comp.set_attack_ms(1.0);
        comp.reset();

        // Use a 440 Hz sine wave (well above sidechain HPF cutoff)
        let freq = 440.0;
        let sr = 44100.0;
        let mut max_out = 0.0_f32;
        for i in 0..2000 {
            let input = libm::sinf(i as f32 * 2.0 * core::f32::consts::PI * freq / sr) * 0.5;
            let output = comp.process(input);
            if i >= 500 {
                max_out = if output.abs() > max_out {
                    output.abs()
                } else {
                    max_out
                };
            }
        }

        assert!(
            max_out < 0.5,
            "Output should be compressed, got {}",
            max_out
        );
    }

    #[test]
    fn test_param_count() {
        let comp = Compressor::new(48000.0);
        assert_eq!(comp.param_count(), 11);
    }

    #[test]
    fn test_detection_mode_param() {
        let mut comp = Compressor::new(48000.0);

        // Default is Peak (0)
        assert_eq!(comp.get_param(6), 0.0);

        // Set to RMS
        comp.set_param(6, 1.0);
        assert_eq!(comp.get_param(6), 1.0);

        // Set back to Peak
        comp.set_param(6, 0.0);
        assert_eq!(comp.get_param(6), 0.0);
    }

    #[test]
    fn test_sidechain_freq_param() {
        let mut comp = Compressor::new(48000.0);

        // Default is 80 Hz
        assert!((comp.get_param(7) - 80.0).abs() < 0.01);

        // Set to 200 Hz
        comp.set_param(7, 200.0);
        assert!((comp.get_param(7) - 200.0).abs() < 0.01);

        // Clamped to range
        comp.set_param(7, 1000.0);
        assert!((comp.get_param(7) - 500.0).abs() < 0.01);
    }

    #[test]
    fn test_auto_makeup_param() {
        let mut comp = Compressor::new(48000.0);

        // Default is Off (0)
        assert_eq!(comp.get_param(8), 0.0);

        // Enable
        comp.set_param(8, 1.0);
        assert_eq!(comp.get_param(8), 1.0);
        assert!(comp.auto_makeup);
    }

    #[test]
    fn test_auto_makeup_gain_formula() {
        let mut comp = Compressor::new(48000.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);
        comp.set_auto_makeup(true);

        // Expected: -(-20) * (1 - 1/4) * 0.5 = 20 * 0.75 * 0.5 = 7.5 dB
        let expected_db = 7.5;
        let expected_linear = db_to_linear(expected_db);
        let actual_linear = comp.auto_makeup_linear();
        assert!(
            (actual_linear - expected_linear).abs() < 0.1,
            "Auto makeup: expected ~{expected_linear}, got {actual_linear}"
        );
    }

    #[test]
    fn test_output_level_param() {
        let mut comp = Compressor::new(48000.0);

        // Default 0 dB
        let out_db = comp.get_param(9);
        assert!(
            (out_db - 0.0).abs() < 0.1,
            "Default output should be 0 dB, got {out_db}"
        );

        // Set to -6 dB
        comp.set_param(9, -6.0);
        let out_db = comp.get_param(9);
        assert!(
            (out_db - (-6.0)).abs() < 0.1,
            "Output should be -6 dB, got {out_db}"
        );
    }

    #[test]
    fn test_rms_mode_compresses() {
        let mut comp = Compressor::new(44100.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);
        comp.set_attack_ms(1.0);
        comp.set_detection_mode(DetectionMode::Rms);
        comp.reset();

        let freq = 440.0;
        let sr = 44100.0;
        let mut max_out = 0.0_f32;
        for i in 0..2000 {
            let input = libm::sinf(i as f32 * 2.0 * core::f32::consts::PI * freq / sr) * 0.5;
            let output = comp.process(input);
            if i >= 500 {
                max_out = if output.abs() > max_out {
                    output.abs()
                } else {
                    max_out
                };
            }
        }

        assert!(
            max_out < 0.5,
            "RMS mode should also compress, got {}",
            max_out
        );
    }

    #[test]
    fn test_program_dependent_release() {
        // With program-dependent release, the compressor should respond faster
        // to new transients during slow release
        let mut comp = Compressor::new(48000.0);
        comp.set_threshold_db(-20.0);
        comp.set_ratio(4.0);
        comp.set_attack_ms(1.0);
        comp.set_release_ms(500.0); // Slow release
        comp.reset();

        let freq = 440.0;
        let sr = 48000.0;

        // Feed a loud burst to engage compression
        for i in 0..2400 {
            let input = libm::sinf(i as f32 * 2.0 * core::f32::consts::PI * freq / sr) * 0.8;
            comp.process(input);
        }

        // Should be compressing
        assert!(comp.gain_reduction_db() < -1.0);

        // Brief silence, then a second burst
        for _ in 0..480 {
            comp.process(0.0);
        }

        // The fast envelope (50ms) should have partially released
        // while the slow envelope (500ms) is still holding
        let gr_after_gap = comp.gain_reduction_db();

        // Feed another burst — fast envelope should catch it
        for i in 0..480 {
            let input = libm::sinf(i as f32 * 2.0 * core::f32::consts::PI * freq / sr) * 0.8;
            comp.process(input);
        }

        let gr_second_burst = comp.gain_reduction_db();
        assert!(
            gr_second_burst < -1.0,
            "Should compress second burst, got {} (after gap: {})",
            gr_second_burst,
            gr_after_gap,
        );
    }

    #[test]
    fn test_stable_param_ids() {
        let comp = Compressor::new(48000.0);
        assert_eq!(comp.param_id(0), Some(ParamId(300))); // Threshold
        assert_eq!(comp.param_id(1), Some(ParamId(301))); // Ratio
        assert_eq!(comp.param_id(2), Some(ParamId(302))); // Attack
        assert_eq!(comp.param_id(3), Some(ParamId(303))); // Release
        assert_eq!(comp.param_id(4), Some(ParamId(304))); // Makeup
        assert_eq!(comp.param_id(5), Some(ParamId(305))); // Knee
        assert_eq!(comp.param_id(6), Some(ParamId(306))); // Detection
        assert_eq!(comp.param_id(7), Some(ParamId(307))); // SC HPF
        assert_eq!(comp.param_id(8), Some(ParamId(308))); // Auto Makeup
        assert_eq!(comp.param_id(9), Some(ParamId(309))); // Output
    }
}
