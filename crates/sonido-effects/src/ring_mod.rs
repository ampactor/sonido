//! Ring modulator effect.
//!
//! Ring modulation is amplitude modulation with a bipolar carrier oscillator,
//! producing sum and difference frequencies. Unlike tremolo (unipolar AM),
//! the carrier ranges from -1 to +1, causing sidebands at both `f_in ± f_carrier`
//! while suppressing the original carrier frequency.
//!
//! ## Signal Flow
//!
//! ```text
//! Input × (1 - depth + depth × carrier) → Wet/Dry Mix → Output Level
//! ```
//!
//! At `depth = 1.0`: full ring mod — `output = input × carrier`
//! At `depth = 0.0`: bypass — `output = input`
//!
//! ## Theory
//!
//! For a sinusoidal input `A·sin(2π·f_in·t)` and carrier `sin(2π·f_c·t)`:
//!
//! ```text
//! output = A/2 · [cos(2π(f_in - f_c)t) - cos(2π(f_in + f_c)t)]
//! ```
//!
//! The result is two sidebands with no original frequency components, producing
//! the classic "robot voice" or "Dalek" timbre.
//!
//! Reference: Zölzer, "DAFX: Digital Audio Effects" (2011), Ch. 2
//! (Amplitude Modulation and Ring Modulation).

use core::f32::consts::TAU;
use libm::{fabsf, sinf};
use sonido_core::{
    Effect, ParamDescriptor, ParamFlags, ParamId, ParamScale, ParamUnit, SmoothedParam, impl_params,
};

/// Carrier oscillator waveform for ring modulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CarrierWaveform {
    /// Sine wave — classic ring modulation with pure sidebands.
    #[default]
    Sine,
    /// Triangle wave — softer harmonic content, richer sidebands.
    Triangle,
    /// Square wave — aggressive metallic timbre, many harmonic sidebands.
    Square,
}

/// Ring modulator effect.
///
/// Multiplies the input signal by a carrier oscillator, generating sum and
/// difference frequencies. Classic applications include metallic timbres,
/// "robot voice" effects, and experimental sound design.
///
/// ## Parameters
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Frequency | 20–2000 Hz | 220.0 Hz |
/// | 1 | Depth | 0–100% | 100.0% |
/// | 2 | Waveform | 0/1/2 (Sine/Triangle/Square) | 0 (Sine) |
/// | 3 | Mix | 0–100% | 50.0% |
/// | 4 | Output | -20–20 dB | 0.0 dB |
///
/// # Example
///
/// ```rust
/// use sonido_effects::{RingMod, CarrierWaveform};
/// use sonido_core::Effect;
///
/// let mut ring_mod = RingMod::new(48000.0);
/// ring_mod.set_frequency(440.0);
/// ring_mod.set_depth(1.0);
/// ring_mod.set_waveform(CarrierWaveform::Sine);
///
/// let output = ring_mod.process(0.5);
/// assert!(output.is_finite());
/// ```
#[derive(Debug, Clone)]
pub struct RingMod {
    /// Sample rate in Hz.
    sample_rate: f32,
    /// Carrier frequency (smoothed to avoid zipper noise).
    frequency: SmoothedParam,
    /// Modulation depth: 0.0 = bypass, 1.0 = full ring mod.
    depth: f32,
    /// Carrier oscillator waveform shape.
    waveform: CarrierWaveform,
    /// Wet/dry mix: 0.0 = dry only, 1.0 = wet only.
    mix: f32,
    /// Output level as linear gain (smoothed).
    output_level: SmoothedParam,
    /// Phase accumulator in [0.0, 1.0).
    phase: f32,
}

impl RingMod {
    /// Create a new ring modulator at the given sample rate.
    ///
    /// Defaults: 220 Hz carrier, 100% depth, sine waveform, 50% mix, 0 dB output.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            frequency: SmoothedParam::fast(220.0, sample_rate),
            depth: 1.0,
            waveform: CarrierWaveform::Sine,
            mix: 0.5,
            output_level: sonido_core::gain::output_level_param(sample_rate),
            phase: 0.0,
        }
    }

    /// Set carrier frequency in Hz (20–2000 Hz).
    pub fn set_frequency(&mut self, freq_hz: f32) {
        self.frequency.set_target(freq_hz.clamp(20.0, 2000.0));
    }

    /// Get current carrier frequency target in Hz.
    pub fn frequency(&self) -> f32 {
        self.frequency.target()
    }

    /// Set modulation depth (0.0–1.0).
    ///
    /// At 0.0, the effect is bypassed (dry signal only).
    /// At 1.0, full ring modulation is applied (carrier × input).
    pub fn set_depth(&mut self, depth: f32) {
        self.depth = depth.clamp(0.0, 1.0);
    }

    /// Get current modulation depth (0.0–1.0).
    pub fn depth(&self) -> f32 {
        self.depth
    }

    /// Set carrier waveform shape.
    pub fn set_waveform(&mut self, waveform: CarrierWaveform) {
        self.waveform = waveform;
    }

    /// Get current carrier waveform.
    pub fn waveform(&self) -> CarrierWaveform {
        self.waveform
    }

    /// Set wet/dry mix (0.0 = dry only, 1.0 = wet only).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Get current wet/dry mix.
    pub fn mix(&self) -> f32 {
        self.mix
    }

    /// Compute the current carrier value from the phase accumulator.
    ///
    /// Returns a bipolar value in [-1.0, 1.0].
    #[inline]
    fn carrier_value(&self) -> f32 {
        match self.waveform {
            CarrierWaveform::Sine => sinf(self.phase * TAU),
            // Bipolar triangle: 1 at phase=0.25, -1 at phase=0.75, 0 at 0 and 0.5
            CarrierWaveform::Triangle => 4.0 * fabsf(self.phase - 0.5) - 1.0,
            CarrierWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
        }
    }

    /// Advance the phase accumulator by one sample at `freq_hz`.
    #[inline]
    fn advance_phase(&mut self, freq_hz: f32) {
        self.phase += freq_hz / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
    }

    /// Process a single sample through the ring modulator.
    ///
    /// Returns the mixed, level-adjusted output sample.
    #[inline]
    fn process_sample(&mut self, input: f32, freq: f32, out_gain: f32) -> f32 {
        let carrier = self.carrier_value();
        self.advance_phase(freq);
        let modulated = input * (1.0 - self.depth + self.depth * carrier);
        let mixed = sonido_core::wet_dry_mix(input, modulated, self.mix);
        mixed * out_gain
    }
}

impl Effect for RingMod {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let freq = self.frequency.advance();
        let out_gain = self.output_level.advance();
        self.process_sample(input, freq, out_gain)
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Advance smoothed params once per sample — shared for both channels
        let freq = self.frequency.advance();
        let out_gain = self.output_level.advance();

        // Compute carrier once; same phase for both channels (dual-mono with shared oscillator)
        let carrier = self.carrier_value();
        self.advance_phase(freq);

        let mod_l = left * (1.0 - self.depth + self.depth * carrier);
        let mod_r = right * (1.0 - self.depth + self.depth * carrier);

        let out_l = sonido_core::wet_dry_mix(left, mod_l, self.mix) * out_gain;
        let out_r = sonido_core::wet_dry_mix(right, mod_r, self.mix) * out_gain;

        (out_l, out_r)
    }

    fn is_true_stereo(&self) -> bool {
        // Shared carrier phase — both channels receive identical modulation
        false
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.frequency.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.phase = 0.0;
        self.frequency.snap_to_target();
        self.output_level.snap_to_target();
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

impl_params! {
    RingMod, this {
        [0] ParamDescriptor::custom("Frequency", "Freq", 20.0, 2000.0, 220.0)
                .with_unit(ParamUnit::Hertz)
                .with_scale(ParamScale::Logarithmic)
                .with_id(ParamId(1800), "ring_freq"),
            get: this.frequency.target(),
            set: |v| this.frequency.set_target(v);

        [1] ParamDescriptor::depth()
                .with_id(ParamId(1801), "ring_depth"),
            get: this.depth * 100.0,
            set: |v| this.depth = v / 100.0;

        [2] ParamDescriptor::custom("Waveform", "Wave", 0.0, 2.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(1802), "ring_wave")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Sine", "Triangle", "Square"]),
            get: this.waveform as u8 as f32,
            set: |v| {
                this.waveform = if v < 0.5 {
                    CarrierWaveform::Sine
                } else if v < 1.5 {
                    CarrierWaveform::Triangle
                } else {
                    CarrierWaveform::Square
                };
            };

        [3] ParamDescriptor::mix()
                .with_id(ParamId(1803), "ring_mix"),
            get: this.mix * 100.0,
            set: |v| this.mix = v / 100.0;

        [4] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1804), "ring_output"),
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
        let ring_mod = RingMod::new(48000.0);
        assert_eq!(ring_mod.param_count(), 5);

        // Frequency default
        let freq_desc = ring_mod.param_info(0).unwrap();
        assert_eq!(freq_desc.default, 220.0);

        // Depth default = 100%
        assert!((ring_mod.get_param(1) - 100.0).abs() < 0.01);

        // Waveform default = 0 (Sine)
        assert!((ring_mod.get_param(2) - 0.0).abs() < 0.01);

        // Mix default = 50%
        assert!((ring_mod.get_param(3) - 50.0).abs() < 0.01);

        // Output default = 0 dB
        assert!((ring_mod.get_param(4) - 0.0).abs() < 0.1);
    }

    #[test]
    fn test_zero_depth_passthrough() {
        let mut ring_mod = RingMod::new(48000.0);
        ring_mod.set_depth(0.0);
        ring_mod.set_mix(1.0); // full wet, but at depth=0 wet = dry

        // Let smoothing settle
        for _ in 0..1000 {
            ring_mod.process(0.5);
        }

        // At depth=0, modulated = input * (1 - 0 + 0 * carrier) = input
        let output = ring_mod.process(0.5);
        assert!(
            (output - 0.5).abs() < 0.01,
            "Zero depth should pass signal unchanged, got {}",
            output
        );
    }

    #[test]
    fn test_full_depth_ring_mod() {
        let mut ring_mod = RingMod::new(48000.0);
        ring_mod.set_depth(1.0);
        ring_mod.set_mix(1.0); // pure wet

        // Let smoothing settle
        for _ in 0..1000 {
            ring_mod.process(1.0);
        }

        // At depth=1, a DC input of 1.0 should produce the carrier waveform.
        // The sine carrier at 220 Hz should oscillate between ~-1 and ~+1.
        let mut min_val = f32::MAX;
        let mut max_val = f32::MIN;
        // 48000 / 220 ≈ 218 samples per cycle; collect a few cycles
        for _ in 0..2000 {
            let out = ring_mod.process(1.0);
            min_val = min_val.min(out);
            max_val = max_val.max(out);
        }

        assert!(
            min_val < -0.5,
            "Full ring mod on DC should produce negative values, min={}",
            min_val
        );
        assert!(
            max_val > 0.5,
            "Full ring mod on DC should produce positive values, max={}",
            max_val
        );
    }

    #[test]
    fn test_waveform_switching() {
        let sr = 48000.0;
        let input = 0.7_f32;

        // Collect 1000 samples from each waveform at the same frequency
        let collect = |wf: CarrierWaveform| -> Vec<f32> {
            let mut rm = RingMod::new(sr);
            rm.set_waveform(wf);
            rm.set_frequency(220.0);
            rm.set_mix(1.0);
            // Settle smoothing
            for _ in 0..500 {
                rm.process(input);
            }
            (0..1000).map(|_| rm.process(input)).collect()
        };

        let sine_out = collect(CarrierWaveform::Sine);
        let tri_out = collect(CarrierWaveform::Triangle);
        let sq_out = collect(CarrierWaveform::Square);

        // Each waveform should produce a different pattern
        let diff_sine_tri: f32 = sine_out
            .iter()
            .zip(tri_out.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 1000.0;
        let diff_sine_sq: f32 = sine_out
            .iter()
            .zip(sq_out.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 1000.0;

        assert!(
            diff_sine_tri > 0.01,
            "Sine and Triangle should differ, mean_diff={}",
            diff_sine_tri
        );
        assert!(
            diff_sine_sq > 0.01,
            "Sine and Square should differ, mean_diff={}",
            diff_sine_sq
        );
    }

    #[test]
    fn test_frequency_affects_output() {
        let sr = 48000.0;
        let input = 0.5_f32;

        let collect = |freq: f32| -> Vec<f32> {
            let mut rm = RingMod::new(sr);
            rm.set_frequency(freq);
            rm.set_mix(1.0);
            for _ in 0..1000 {
                rm.process(input);
            }
            (0..500).map(|_| rm.process(input)).collect()
        };

        let low = collect(100.0);
        let high = collect(800.0);

        // Different frequencies → different oscillation patterns
        let mean_diff: f32 = low
            .iter()
            .zip(high.iter())
            .map(|(a, b)| (a - b).abs())
            .sum::<f32>()
            / 500.0;

        assert!(
            mean_diff > 0.01,
            "Different frequencies should produce different outputs, mean_diff={}",
            mean_diff
        );
    }

    #[test]
    fn test_reset_clears_phase() {
        let mut ring_mod = RingMod::new(48000.0);

        // Run for a while to accumulate phase
        for _ in 0..5000 {
            ring_mod.process(0.5);
        }

        ring_mod.reset();
        assert!(
            ring_mod.phase.abs() < 1e-10,
            "Phase should be 0 after reset, got {}",
            ring_mod.phase
        );
    }

    #[test]
    fn test_output_bounded() {
        let mut ring_mod = RingMod::new(48000.0);
        ring_mod.set_depth(1.0);

        let inputs = [-1.0_f32, -0.5, 0.0, 0.5, 1.0];
        for &input in &inputs {
            for _ in 0..100 {
                let out = ring_mod.process(input);
                assert!(out.is_finite(), "Output must be finite for input {}", input);
                // Input is ≤1.0 and output gain is unity → output should not exceed ~1.0 by much
                assert!(
                    out.abs() <= 1.1,
                    "Output exceeded expected bounds: input={}, out={}",
                    input,
                    out
                );
            }
        }
    }

    #[test]
    fn test_stereo_dual_mono() {
        let mut ring_mod = RingMod::new(48000.0);
        ring_mod.set_depth(1.0);
        ring_mod.set_mix(1.0);

        // With identical L and R input, outputs should be identical (shared carrier)
        for _ in 0..2000 {
            let (l, r) = ring_mod.process_stereo(0.5, 0.5);
            assert!(
                (l - r).abs() < 1e-6,
                "Dual-mono: L and R should be identical, got L={} R={}",
                l,
                r
            );
        }

        assert!(!ring_mod.is_true_stereo());
    }

    #[test]
    fn test_param_roundtrip() {
        let mut ring_mod = RingMod::new(48000.0);

        // Frequency
        ring_mod.set_param(0, 440.0);
        assert!(
            (ring_mod.get_param(0) - 440.0).abs() < 0.01,
            "Frequency roundtrip failed"
        );

        // Depth (percent)
        ring_mod.set_param(1, 75.0);
        assert!(
            (ring_mod.get_param(1) - 75.0).abs() < 0.01,
            "Depth roundtrip failed"
        );

        // Waveform → Triangle
        ring_mod.set_param(2, 1.0);
        assert_eq!(ring_mod.waveform, CarrierWaveform::Triangle);
        assert!((ring_mod.get_param(2) - 1.0).abs() < 0.01);

        // Mix (percent)
        ring_mod.set_param(3, 80.0);
        assert!(
            (ring_mod.get_param(3) - 80.0).abs() < 0.01,
            "Mix roundtrip failed"
        );

        // Output (dB)
        ring_mod.set_param(4, -6.0);
        assert!(
            (ring_mod.get_param(4) - (-6.0)).abs() < 0.1,
            "Output dB roundtrip failed"
        );
    }
}
