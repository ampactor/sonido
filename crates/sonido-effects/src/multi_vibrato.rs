//! Multi-Vibrato tape simulation
//!
//! 10 simultaneous subtle vibratos at different frequencies and waveforms
//! to simulate authentic tape wow and flutter.

use sonido_core::{
    Effect, FixedDelayLine, Lfo, LfoWaveform, ParamDescriptor, ParamUnit, ParameterInfo,
    SmoothedParam, wet_dry_mix, wet_dry_mix_stereo,
};

/// Number of vibrato units in MultiVibrato
pub const NUM_VIBRATOS: usize = 10;

/// Single vibrato unit with its own LFO
struct VibratoUnit {
    lfo: Lfo,
    /// Depth in cents (very subtle: 0.1-2 cents typical)
    depth_cents: f32,
    /// Delay line for pitch modulation
    delay: FixedDelayLine<512>,
    /// Base delay in samples
    base_delay: f32,
}

impl VibratoUnit {
    fn new(sample_rate: f32, rate_hz: f32, depth_cents: f32, waveform: LfoWaveform) -> Self {
        let mut lfo = Lfo::new(sample_rate, rate_hz);
        lfo.set_waveform(waveform);

        Self {
            lfo,
            depth_cents,
            delay: FixedDelayLine::new(),
            base_delay: 128.0, // ~2.7ms base delay for modulation headroom
        }
    }

    fn process(&mut self, input: f32, sample_rate: f32, depth_scale: f32) -> f32 {
        let lfo_val = self.lfo.advance();

        // Convert cents to delay modulation, scaled by master depth
        let cents_to_samples = self.depth_cents * depth_scale * sample_rate / 44100.0 * 0.01;
        let delay_mod = lfo_val * cents_to_samples;

        let delay_samples = self.base_delay + delay_mod;
        self.delay.read_write(input, delay_samples)
    }

    fn reset(&mut self) {
        self.lfo.reset();
        self.delay.clear();
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.lfo.set_sample_rate(sample_rate);
    }
}

/// Multi-Vibrato tape simulation
///
/// Each individual vibrato is nearly imperceptible, but combined they
/// create the organic, living quality of real tape.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Depth | 0–200% | 100.0 |
/// | 1 | Mix | 0–100% | 100.0 |
/// | 2 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::MultiVibrato;
/// use sonido_core::Effect;
///
/// let mut vibrato = MultiVibrato::new(48000.0);
/// vibrato.set_mix(0.8);
/// vibrato.set_depth(1.2);
///
/// let input = 0.5;
/// let output = vibrato.process(input);
/// ```
pub struct MultiVibrato {
    vibratos: [VibratoUnit; NUM_VIBRATOS],
    vibratos_r: [VibratoUnit; NUM_VIBRATOS],
    sample_rate: f32,
    /// Overall mix (0.0 = dry, 1.0 = full effect)
    mix: f32,
    /// Master depth control (scales all vibrato depths) with smoothing
    depth_scale: SmoothedParam,
    /// Output level (linear gain)
    output_level: SmoothedParam,
}

impl Default for MultiVibrato {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl MultiVibrato {
    /// Create a new MultiVibrato with default settings.
    pub fn new(sample_rate: f32) -> Self {
        // Carefully chosen rates and waveforms for organic tape character
        let configs: [(f32, f32, LfoWaveform); NUM_VIBRATOS] = [
            (0.13, 0.8, LfoWaveform::Sine),     // Very slow drift
            (0.31, 1.2, LfoWaveform::Triangle), // Slow wobble
            (0.67, 0.6, LfoWaveform::Sine),     // Medium drift
            (1.1, 0.9, LfoWaveform::Triangle),  // Flutter component
            (1.7, 0.5, LfoWaveform::Sine),      // Higher flutter
            (2.3, 0.4, LfoWaveform::Triangle),  // Subtle fast
            (0.23, 1.5, LfoWaveform::Sine),     // Another slow
            (3.1, 0.3, LfoWaveform::Triangle),  // Fast, subtle
            (0.47, 1.0, LfoWaveform::Sine),     // Medium
            (4.7, 0.2, LfoWaveform::Triangle),  // Fastest, most subtle
        ];

        let vibratos = configs
            .map(|(rate, depth, waveform)| VibratoUnit::new(sample_rate, rate, depth, waveform));
        let vibratos_r = configs
            .map(|(rate, depth, waveform)| VibratoUnit::new(sample_rate, rate, depth, waveform));

        Self {
            vibratos,
            vibratos_r,
            sample_rate,
            mix: 1.0,
            depth_scale: SmoothedParam::standard(1.0, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
        }
    }

    /// Set overall mix (0.0 = dry, 1.0 = full effect)
    pub fn set_mix(&mut self, mix: f32) {
        self.mix = mix.clamp(0.0, 1.0);
    }

    /// Get current mix
    pub fn mix(&self) -> f32 {
        self.mix
    }

    /// Set master depth scale (multiplies all vibrato depths)
    pub fn set_depth(&mut self, scale: f32) {
        self.depth_scale.set_target(scale.clamp(0.0, 2.0));
    }

    /// Get current depth scale target
    pub fn depth(&self) -> f32 {
        self.depth_scale.target()
    }
}

impl Effect for MultiVibrato {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let depth_scale = self.depth_scale.advance();
        let output_gain = self.output_level.advance();

        // Process through all vibratos and average
        let mut wet = 0.0f32;
        for vib in &mut self.vibratos {
            wet += vib.process(input, self.sample_rate, depth_scale);
        }
        wet /= NUM_VIBRATOS as f32;

        wet_dry_mix(input, wet, self.mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let depth_scale = self.depth_scale.advance();
        let output_gain = self.output_level.advance();

        let mut wet_l = 0.0f32;
        for vib in &mut self.vibratos {
            wet_l += vib.process(left, self.sample_rate, depth_scale);
        }
        wet_l /= NUM_VIBRATOS as f32;

        let mut wet_r = 0.0f32;
        for vib in &mut self.vibratos_r {
            wet_r += vib.process(right, self.sample_rate, depth_scale);
        }
        wet_r /= NUM_VIBRATOS as f32;

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, wet_l, wet_r, self.mix);

        (out_l * output_gain, out_r * output_gain)
    }

    fn reset(&mut self) {
        for vib in &mut self.vibratos {
            vib.reset();
        }
        for vib in &mut self.vibratos_r {
            vib.reset();
        }
        self.depth_scale.snap_to_target();
        self.output_level.snap_to_target();
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for vib in &mut self.vibratos {
            vib.set_sample_rate(sample_rate);
        }
        for vib in &mut self.vibratos_r {
            vib.set_sample_rate(sample_rate);
        }
        self.depth_scale.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    fn latency_samples(&self) -> usize {
        // Report minimal latency from base delay
        128
    }
}

impl ParameterInfo for MultiVibrato {
    fn param_count(&self) -> usize {
        3
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Depth",
                short_name: "Depth",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 200.0,
                default: 100.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Mix",
                short_name: "Mix",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 100.0,
                step: 1.0,
            }),
            2 => Some(sonido_core::gain::output_param_descriptor()),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.depth_scale.target() * 100.0,
            1 => self.mix * 100.0,
            2 => sonido_core::gain::output_level_db(&self.output_level),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_depth(value / 100.0),
            1 => self.set_mix(value / 100.0),
            2 => sonido_core::gain::set_output_level_db(&mut self.output_level, value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_vibrato_basic() {
        let mut vibrato = MultiVibrato::new(48000.0);
        vibrato.set_mix(1.0);

        for _ in 0..1000 {
            let output = vibrato.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_multi_vibrato_bypass() {
        let mut vibrato = MultiVibrato::new(48000.0);
        vibrato.set_mix(0.0);

        let output = vibrato.process(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_multi_vibrato_latency() {
        let vibrato = MultiVibrato::new(48000.0);
        assert_eq!(vibrato.latency_samples(), 128);
    }

    #[test]
    fn test_multi_vibrato_depth_smoothing() {
        let mut vibrato = MultiVibrato::new(48000.0);
        vibrato.set_mix(1.0);
        vibrato.set_depth(0.0);
        vibrato.reset();

        // Process some samples to fill delay lines
        for _ in 0..200 {
            vibrato.process(0.5);
        }

        // Set new depth target
        vibrato.set_depth(2.0);

        // Verify target is set
        assert!((vibrato.depth() - 2.0).abs() < 0.01, "Target should be 2.0");

        // Process samples - the depth smoothing takes effect
        for _ in 0..1000 {
            let output = vibrato.process(0.5);
            assert!(output.is_finite());
        }
    }
}
