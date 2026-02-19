//! Bitcrusher effect: sample rate reduction and bit depth quantization.
//!
//! # Theory
//!
//! Bitcrushing intentionally degrades audio fidelity to produce lo-fi
//! digital artifacts. Two independent mechanisms drive the effect:
//!
//! ## Bit Depth Reduction (Quantization)
//!
//! Reducing bit depth simulates lower-resolution digital audio (e.g., old
//! samplers, game consoles). With B bits, the amplitude axis is divided into
//! 2^B quantization levels. Each sample is rounded to the nearest level:
//!
//! ```text
//! levels = 2^B
//! quantized = floor(x · levels + 0.5) / levels
//! ```
//!
//! This introduces **quantization noise** — a broadband error signal whose
//! power is approximately `1 / (12 · 2^(2B))` relative to full scale. At
//! 16 bits the noise floor is ~−98 dBFS; at 4 bits it is ~−26 dBFS.
//!
//! Reference: Zolzer, "DAFX: Digital Audio Effects" 2nd ed., Chapter 7
//! (Quantization and Sample Rate Conversion).
//!
//! ## Sample Rate Reduction (Zero-Order Hold)
//!
//! Reducing the effective sample rate via zero-order hold (ZOH) repeats each
//! input sample for N output samples. This creates **aliasing** — high
//! frequency components fold back into the audible range — and a characteristic
//! "staircase" distortion. The ZOH frequency response is:
//!
//! ```text
//! H(f) = sin(π·f·N/fs) / (π·f·N/fs)  (sinc envelope)
//! ```
//!
//! which rolls off high frequencies, but the aliased harmonics are typically
//! more audible than the roll-off.
//!
//! Reference: Zolzer, "DAFX" Chapter 7; Smith, "Physical Audio Signal
//! Processing" — Sample Rate Conversion section.
//!
//! ## Jitter
//!
//! Real hardware samplers have clock instability. Jitter adds random variation
//! to the hold counter threshold, producing subtle analog character. The
//! variation is computed with a lightweight LCG PRNG (no heap allocation).

#[cfg(not(feature = "std"))]
extern crate alloc;

use libm::{floorf, powf};
use sonido_core::{
    Effect, ParamDescriptor, ParamFlags, ParamId, ParamUnit, SmoothedParam, wet_dry_mix,
};

/// Bitcrusher effect combining bit depth reduction and sample rate reduction.
///
/// Produces lo-fi digital artifacts via two mechanisms:
/// 1. **Bit depth reduction** — quantizes amplitude to fewer discrete levels,
///    introducing broadband quantization noise.
/// 2. **Sample rate reduction** — repeats (holds) each input sample for
///    multiple output samples via zero-order hold, creating aliasing.
///
/// An optional **jitter** parameter adds random variation to the hold counter
/// threshold, simulating analog clock instability for a more organic character.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Bit Depth | 2–16 | 8 |
/// | 1 | Downsample | 1–64 | 1 |
/// | 2 | Jitter | 0–100% | 0% |
/// | 3 | Mix | 0–100% | 100% |
/// | 4 | Output | -20–+20 dB | 0 dB |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Bitcrusher;
/// use sonido_core::Effect;
///
/// let mut crusher = Bitcrusher::new(44100.0);
/// crusher.set_bit_depth(4.0);
/// crusher.set_downsample(4.0);
///
/// let output = crusher.process(0.5);
/// assert!(output.is_finite());
/// ```
#[derive(Debug, Clone)]
pub struct Bitcrusher {
    sample_rate: f32,
    /// Target bit depth (2–16). Smoothed to allow automation without clicks.
    bit_depth: SmoothedParam,
    /// Downsample factor (1–64, integer steps). Smoothed for automation.
    downsample: SmoothedParam,
    /// Jitter amount as a fraction (0.0–1.0). Not smoothed — jitter is
    /// inherently stochastic and per-hold-event, so smoothing adds no value.
    jitter: f32,
    /// Wet/dry mix (0.0 = dry, 1.0 = fully crushed).
    mix: f32,
    /// Output level as a linear gain, smoothed to avoid clicks on changes.
    output_level: SmoothedParam,
    /// Currently held left (or mono) sample.
    held_l: f32,
    /// Currently held right sample.
    held_r: f32,
    /// Fractional sample counter. Increments each sample; when it exceeds the
    /// downsample threshold a new input is latched.
    counter: f32,
    /// LCG PRNG state for jitter generation.
    rng_state: u32,
}

impl Bitcrusher {
    /// Create a new `Bitcrusher` at the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            bit_depth: SmoothedParam::standard(8.0, sample_rate),
            downsample: SmoothedParam::standard(1.0, sample_rate),
            jitter: 0.0,
            mix: 1.0,
            output_level: sonido_core::gain::output_level_param(sample_rate),
            held_l: 0.0,
            held_r: 0.0,
            counter: 0.0,
            rng_state: 0x1234_5678,
        }
    }

    /// Set bit depth (2.0–16.0). Values are clamped to that range.
    pub fn set_bit_depth(&mut self, bits: f32) {
        self.bit_depth.set_target(bits.clamp(2.0, 16.0));
    }

    /// Get current bit depth target.
    #[must_use]
    pub fn bit_depth(&self) -> f32 {
        self.bit_depth.target()
    }

    /// Set the downsample factor (1.0–64.0). 1 = no rate reduction.
    pub fn set_downsample(&mut self, factor: f32) {
        self.downsample.set_target(factor.clamp(1.0, 64.0));
    }

    /// Get current downsample factor target.
    #[must_use]
    pub fn downsample(&self) -> f32 {
        self.downsample.target()
    }

    /// Set jitter amount (0.0–1.0). 0 = no jitter; 1 = full random variation.
    pub fn set_jitter(&mut self, amount: f32) {
        self.jitter = amount.clamp(0.0, 1.0);
    }

    /// Get current jitter amount (0.0–1.0).
    #[must_use]
    pub fn jitter(&self) -> f32 {
        self.jitter
    }

    /// Set wet/dry mix (0.0 = dry, 1.0 = fully crushed).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Get current wet/dry mix (0.0–1.0).
    #[must_use]
    pub fn mix(&self) -> f32 {
        self.mix
    }

    /// Advance the LCG PRNG and return a value in [0.0, 1.0).
    ///
    /// Uses the Numerical Recipes LCG constants (`a = 1664525`, `c = 1013904223`)
    /// which have good statistical properties for a 32-bit generator. The upper
    /// 16 bits are used to reduce correlation between successive values.
    #[inline]
    fn next_random(&mut self) -> f32 {
        self.rng_state = self
            .rng_state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        // Extract upper 16 bits as u16 (max 65535) before widening to f32.
        // Casting u16 → f32 is lossless (u16 fits in 23-bit mantissa exactly).
        let upper = (self.rng_state >> 16) as u16;
        f32::from(upper) / 65_536.0
    }

    /// Quantize a single sample to the current bit depth.
    ///
    /// Applies mid-tread uniform quantization:
    /// ```text
    /// levels = 2^bits
    /// quantized = floor(x · levels + 0.5) / levels
    /// ```
    ///
    /// The `+ 0.5` shifts from floor-rounding to nearest-level rounding.
    /// Input range is assumed to be approximately [-1.0, 1.0]; values
    /// outside that range are still quantized correctly but may clip.
    #[inline]
    fn quantize(sample: f32, bits: f32) -> f32 {
        let levels = powf(2.0, bits);
        floorf(sample * levels + 0.5) / levels
    }

    /// Compute the downsample threshold for the current sample, including jitter.
    ///
    /// When jitter > 0, adds a random offset in `[0, jitter · downsample)` so
    /// the hold duration varies each cycle, simulating clock instability.
    #[inline]
    fn threshold_with_jitter(&mut self, downsample: f32) -> f32 {
        if self.jitter > 0.0 {
            // Random offset in [0, jitter * downsample)
            downsample + self.next_random() * self.jitter * downsample
        } else {
            downsample
        }
    }
}

impl Effect for Bitcrusher {
    /// Process a single mono sample through the bitcrusher.
    ///
    /// The sample rate reducer and quantizer both advance their state here.
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bits = self.bit_depth.advance();
        let downsample = self.downsample.advance();
        let output_gain = self.output_level.advance();

        self.counter += 1.0;
        let threshold = self.threshold_with_jitter(downsample);
        if self.counter >= threshold {
            self.held_l = Self::quantize(input, bits);
            // Keep only the fractional overshoot so transitions are smooth
            // when downsample changes mid-cycle.
            self.counter -= floorf(self.counter);
        }

        wet_dry_mix(input, self.held_l, self.mix) * output_gain
    }

    /// Process a stereo pair through the bitcrusher.
    ///
    /// L and R channels are crushed independently (separate held samples) but
    /// share the same counter/threshold, giving dual-mono behaviour: the hold
    /// boundary fires at the same moment for both channels, preserving stereo
    /// imaging while crushing each side independently.
    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let bits = self.bit_depth.advance();
        let downsample = self.downsample.advance();
        let output_gain = self.output_level.advance();

        self.counter += 1.0;
        let threshold = self.threshold_with_jitter(downsample);
        if self.counter >= threshold {
            self.held_l = Self::quantize(left, bits);
            self.held_r = Self::quantize(right, bits);
            self.counter -= floorf(self.counter);
        }

        let wet_l = wet_dry_mix(left, self.held_l, self.mix);
        let wet_r = wet_dry_mix(right, self.held_r, self.mix);
        (wet_l * output_gain, wet_r * output_gain)
    }

    /// Returns `false` — the bitcrusher is dual-mono (shared counter, independent channels).
    fn is_true_stereo(&self) -> bool {
        false
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.bit_depth.set_sample_rate(sample_rate);
        self.downsample.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    /// Reset all stateful fields.
    ///
    /// Clears held samples, resets the counter, and snaps all smoothed params
    /// to their current targets (no ramp). The PRNG is reset to its initial
    /// seed so behaviour is deterministic after reset.
    fn reset(&mut self) {
        self.held_l = 0.0;
        self.held_r = 0.0;
        self.counter = 0.0;
        self.rng_state = 0x1234_5678;
        self.bit_depth.snap_to_target();
        self.downsample.snap_to_target();
        self.output_level.snap_to_target();
    }
}

sonido_core::impl_params! {
    Bitcrusher, this {
        [0] ParamDescriptor::custom("Bit Depth", "Bits", 2.0, 16.0, 8.0)
                .with_step(1.0)
                .with_id(ParamId(1700), "crush_bits")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED)),
            get: this.bit_depth.target(),
            set: |v| this.bit_depth.set_target(v);

        [1] ParamDescriptor::custom("Downsample", "Down", 1.0, 64.0, 1.0)
                .with_step(1.0)
                .with_id(ParamId(1701), "crush_down")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED)),
            get: this.downsample.target(),
            set: |v| this.downsample.set_target(v);

        [2] ParamDescriptor::custom("Jitter", "Jitter", 0.0, 100.0, 0.0)
                .with_unit(ParamUnit::Percent)
                .with_id(ParamId(1702), "crush_jitter"),
            get: this.jitter * 100.0,
            set: |v| this.jitter = v / 100.0;

        [3] ParamDescriptor { default: 100.0, ..ParamDescriptor::mix() }
                .with_id(ParamId(1703), "crush_mix"),
            get: this.mix * 100.0,
            set: |v| this.mix = v / 100.0;

        [4] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1704), "crush_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_default_params() {
        let crusher = Bitcrusher::new(44100.0);
        assert_eq!(crusher.param_count(), 5);

        let bits = crusher.param_info(0).unwrap();
        assert_eq!(bits.name, "Bit Depth");
        assert!((bits.default - 8.0).abs() < 1e-6);

        let down = crusher.param_info(1).unwrap();
        assert_eq!(down.name, "Downsample");
        assert!((down.default - 1.0).abs() < 1e-6);

        let jitter = crusher.param_info(2).unwrap();
        assert_eq!(jitter.name, "Jitter");
        assert!((jitter.default - 0.0).abs() < 1e-6);

        let mix = crusher.param_info(3).unwrap();
        assert_eq!(mix.name, "Mix");
        assert!((mix.default - 100.0).abs() < 1e-6);

        let out = crusher.param_info(4).unwrap();
        assert_eq!(out.name, "Output");
        assert!((out.default - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_bit_reduction_coarse() {
        // At 1-bit: levels=2, step=0.5. floor(x*2 + 0.5) / 2:
        //   x=0.9 → floor(1.8+0.5)/2 = floor(2.3)/2 = 2/2 = 1.0
        //   x=0.1 → floor(0.2+0.5)/2 = floor(0.7)/2 = 0/2 = 0.0
        //   x=-0.9 → floor(-1.8+0.5)/2 = floor(-1.3)/2 = -2/2 = -1.0
        let crusher = Bitcrusher::new(44100.0);
        let q_high = Bitcrusher::quantize(0.9, 1.0);
        assert!(
            (q_high - 1.0).abs() < 1e-5,
            "1-bit quantize(0.9) expected 1.0, got {}",
            q_high
        );

        let q_low = Bitcrusher::quantize(0.1, 1.0);
        assert!(
            q_low.abs() < 1e-5,
            "1-bit quantize(0.1) expected 0.0, got {}",
            q_low
        );

        let q_neg = Bitcrusher::quantize(-0.9, 1.0);
        assert!(
            (q_neg + 1.0).abs() < 1e-5,
            "1-bit quantize(-0.9) expected -1.0, got {}",
            q_neg
        );
    }

    #[test]
    fn test_bit_reduction_transparent_at_16bit() {
        let crusher = Bitcrusher::new(44100.0);
        let input = 0.123_456_78;
        let q = Bitcrusher::quantize(input, 16.0);
        // 16-bit gives ~0.000015 step — difference should be tiny
        assert!(
            (q - input).abs() < 2e-4,
            "16-bit quantization should be near-transparent, delta={}",
            (q - input).abs()
        );
    }

    #[test]
    fn test_downsample_holds() {
        let mut crusher = Bitcrusher::new(44100.0);
        // Snap params immediately so smoothing doesn't interfere
        crusher.set_bit_depth(16.0); // near-transparent quantization
        crusher.set_downsample(4.0);
        crusher.set_mix(1.0); // fully wet
        crusher.reset();

        // Feed distinct samples and collect outputs
        let outputs: Vec<f32> = (0..8).map(|i| crusher.process(i as f32 * 0.1)).collect();

        // The first held value is captured on counter==4 (samples 0..3 output 0.0 held_l=0)
        // Then held again at counter overflow. Exact timing depends on counter reset logic.
        // Key property: consecutive outputs should sometimes repeat (hold behaviour).
        let has_repeat = outputs.windows(2).any(|w| (w[0] - w[1]).abs() < 1e-6);
        assert!(
            has_repeat,
            "Downsample=4 should produce repeated (held) output values, got: {:?}",
            outputs
        );
    }

    #[test]
    fn test_passthrough_at_max_bits_no_downsample() {
        let mut crusher = Bitcrusher::new(44100.0);
        crusher.set_bit_depth(16.0);
        crusher.set_downsample(1.0);
        crusher.set_mix(1.0);
        crusher.reset();

        // Let smoothing settle
        let input = 0.5;
        for _ in 0..2000 {
            crusher.process(input);
        }

        let out = crusher.process(input);
        // 16-bit quantization with no rate reduction should be near-transparent
        assert!(
            (out - input).abs() < 1e-3,
            "16-bit/no-downsample should be near-transparent, got {}",
            out
        );
    }

    #[test]
    fn test_mix_dry() {
        let mut crusher = Bitcrusher::new(44100.0);
        crusher.set_bit_depth(2.0); // heavy crushing
        crusher.set_mix(0.0); // fully dry
        crusher.reset();

        let input = 0.3;
        // Let counter settle (held_l = 0.0 initially, but mix=0 means we get dry)
        for _ in 0..10 {
            let out = crusher.process(input);
            assert!(
                (out - input).abs() < 1e-5,
                "mix=0.0 should pass signal dry, got {}",
                out
            );
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let mut crusher = Bitcrusher::new(44100.0);
        crusher.set_downsample(8.0);
        crusher.set_mix(1.0);
        crusher.reset();

        // Process some samples to set held_l to a non-zero value
        for _ in 0..20 {
            crusher.process(0.7);
        }

        // Reset should clear held samples and counter
        crusher.reset();
        assert!((crusher.held_l).abs() < 1e-10);
        assert!((crusher.held_r).abs() < 1e-10);
        assert!((crusher.counter).abs() < 1e-10);
    }

    #[test]
    fn test_output_bounded() {
        let mut crusher = Bitcrusher::new(44100.0);
        crusher.set_bit_depth(2.0);
        crusher.set_downsample(8.0);
        crusher.set_jitter(0.5);
        crusher.set_mix(1.0);
        crusher.reset();

        for i in 0..4096 {
            // Sweep full amplitude range
            let input = libm::sinf(i as f32 * 0.01);
            let (l, r) = crusher.process_stereo(input, -input);
            assert!(l.is_finite() && l.abs() <= 2.0, "L out of bounds: {}", l);
            assert!(r.is_finite() && r.abs() <= 2.0, "R out of bounds: {}", r);
        }
    }

    #[test]
    fn test_stereo_independent_channels() {
        let mut crusher = Bitcrusher::new(44100.0);
        crusher.set_bit_depth(2.0);
        crusher.set_downsample(1.0);
        crusher.set_mix(1.0);
        crusher.reset();

        // Feed different values to L and R — they should produce different crushed outputs.
        // With 2-bit: levels=4, step=0.25
        //   L=0.9 → floor(0.9*4+0.5)/4 = floor(4.1)/4 = 4/4 = 1.0
        //   R=-0.9 → floor(-0.9*4+0.5)/4 = floor(-3.1)/4 = -4/4 = -1.0
        let (l, r) = crusher.process_stereo(0.9, -0.9);
        assert!(l.is_finite() && r.is_finite());
        // L and R should differ (opposite polarity inputs → opposite polarity outputs)
        assert!(
            (l - r).abs() > 0.5,
            "2-bit crush of +0.9/-0.9 should produce distinct L ({}) and R ({})",
            l,
            r
        );
    }

    #[test]
    fn test_param_set_get_roundtrip() {
        let mut crusher = Bitcrusher::new(44100.0);

        crusher.set_param(0, 4.0); // bit depth
        assert!((crusher.get_param(0) - 4.0).abs() < 1e-5);

        crusher.set_param(1, 8.0); // downsample
        assert!((crusher.get_param(1) - 8.0).abs() < 1e-5);

        crusher.set_param(2, 50.0); // jitter %
        assert!((crusher.get_param(2) - 50.0).abs() < 1e-5);

        crusher.set_param(3, 75.0); // mix %
        assert!((crusher.get_param(3) - 75.0).abs() < 1e-5);
    }
}
