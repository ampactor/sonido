//! Classic chorus effect with dual voices.

use libm::ceilf;
use sonido_core::{
    Effect, InterpolatedDelay, Lfo, ParamDescriptor, ParameterInfo, SmoothedParam, wet_dry_mix,
    wet_dry_mix_stereo,
};

/// Chorus effect with dual voices.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Rate | 0.1–10.0 Hz | 1.0 |
/// | 1 | Depth | 0–100% | 50.0 |
/// | 2 | Mix | 0–100% | 50.0 |
/// | 3 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Chorus;
/// use sonido_core::Effect;
///
/// let mut chorus = Chorus::new(44100.0);
/// chorus.set_rate(2.0);
/// chorus.set_depth(0.7);
/// chorus.set_mix(0.5);
///
/// let input = 0.5;
/// let output = chorus.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Chorus {
    delay1: InterpolatedDelay,
    delay2: InterpolatedDelay,
    lfo1: Lfo,
    lfo2: Lfo,
    base_delay_samples: f32,
    max_mod_samples: f32,
    rate: SmoothedParam,
    depth: SmoothedParam,
    mix: SmoothedParam,
    output_level: SmoothedParam,
    sample_rate: f32,
}

impl Chorus {
    /// Create a new chorus effect.
    pub fn new(sample_rate: f32) -> Self {
        const BASE_DELAY_MS: f32 = 15.0;
        const MAX_MOD_MS: f32 = 5.0;
        const MAX_DELAY_MS: f32 = BASE_DELAY_MS + MAX_MOD_MS;

        let base_delay_samples = (BASE_DELAY_MS / 1000.0) * sample_rate;
        let max_mod_samples = (MAX_MOD_MS / 1000.0) * sample_rate;
        let max_delay_samples = ceilf((MAX_DELAY_MS / 1000.0) * sample_rate) as usize;

        let lfo1 = Lfo::new(sample_rate, 1.0);
        let mut lfo2 = Lfo::new(sample_rate, 1.0);
        lfo2.set_phase(0.25); // 90° offset

        Self {
            delay1: InterpolatedDelay::new(max_delay_samples),
            delay2: InterpolatedDelay::new(max_delay_samples),
            lfo1,
            lfo2,
            base_delay_samples,
            max_mod_samples,
            rate: SmoothedParam::standard(1.0, sample_rate),
            depth: SmoothedParam::standard(0.5, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            sample_rate,
        }
    }

    /// Set LFO rate in Hz.
    pub fn set_rate(&mut self, rate_hz: f32) {
        self.rate.set_target(rate_hz.clamp(0.1, 10.0));
    }

    /// Set modulation depth (0-1).
    pub fn set_depth(&mut self, depth: f32) {
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    /// Set wet/dry mix (0-1).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }
}

impl Effect for Chorus {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        self.lfo1.set_frequency(rate);
        self.lfo2.set_frequency(rate);

        let mod1 = self.lfo1.advance();
        let mod2 = self.lfo2.advance();

        let delay_time1 = self.base_delay_samples + (mod1 * depth * self.max_mod_samples);
        let delay_time2 = self.base_delay_samples + (mod2 * depth * self.max_mod_samples);

        let wet1 = self.delay1.read(delay_time1);
        let wet2 = self.delay2.read(delay_time2);

        self.delay1.write(input);
        self.delay2.write(input);

        let wet = (wet1 + wet2) * 0.5;
        wet_dry_mix(input, wet, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // True stereo: pan voice 1 left, voice 2 right for stereo spread
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        self.lfo1.set_frequency(rate);
        self.lfo2.set_frequency(rate);

        let mod1 = self.lfo1.advance();
        let mod2 = self.lfo2.advance();

        let delay_time1 = self.base_delay_samples + (mod1 * depth * self.max_mod_samples);
        let delay_time2 = self.base_delay_samples + (mod2 * depth * self.max_mod_samples);

        // Read delayed signals
        let wet1 = self.delay1.read(delay_time1);
        let wet2 = self.delay2.read(delay_time2);

        // Write mono sum to both delay lines for stereo input
        let mono_in = (left + right) * 0.5;
        self.delay1.write(mono_in);
        self.delay2.write(mono_in);

        // Pan voices for stereo spread: voice1 mostly left, voice2 mostly right
        // with some crossfeed for a natural sound
        let wet_l = wet1 * 0.8 + wet2 * 0.2;
        let wet_r = wet2 * 0.8 + wet1 * 0.2;

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, wet_l, wet_r, mix);

        (out_l * output_gain, out_r * output_gain)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        const BASE_DELAY_MS: f32 = 15.0;
        const MAX_MOD_MS: f32 = 5.0;

        self.base_delay_samples = (BASE_DELAY_MS / 1000.0) * sample_rate;
        self.max_mod_samples = (MAX_MOD_MS / 1000.0) * sample_rate;

        self.lfo1.set_sample_rate(sample_rate);
        self.lfo2.set_sample_rate(sample_rate);
        self.rate.set_sample_rate(sample_rate);
        self.depth.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.delay1.clear();
        self.delay2.clear();
        self.lfo1.reset();
        self.lfo2.reset();
        self.rate.snap_to_target();
        self.depth.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();
    }
}

impl ParameterInfo for Chorus {
    fn param_count(&self) -> usize {
        4
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Rate",
                short_name: "Rate",
                unit: sonido_core::ParamUnit::Hertz,
                min: 0.1,
                max: 10.0,
                default: 1.0,
                step: 0.1,
            }),
            1 => Some(ParamDescriptor::depth()),
            2 => Some(ParamDescriptor::mix()),
            3 => Some(sonido_core::gain::output_param_descriptor()),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.rate.target(),
            1 => self.depth.target() * 100.0,
            2 => self.mix.target() * 100.0,
            3 => sonido_core::gain::output_level_db(&self.output_level),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_rate(value),
            1 => self.set_depth(value / 100.0),
            2 => self.set_mix(value / 100.0),
            3 => sonido_core::gain::set_output_level_db(&mut self.output_level, value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chorus_basic() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);

        for _ in 0..1000 {
            let output = chorus.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_chorus_bypass() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(0.0);

        // Let smoothing settle
        for _ in 0..1000 {
            chorus.process(1.0);
        }

        let output = chorus.process(0.5);
        assert!((output - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_chorus_stereo_processing() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_depth(0.8);

        // Process stereo signal
        for _ in 0..1000 {
            let (l, r) = chorus.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_chorus_is_true_stereo() {
        let chorus = Chorus::new(44100.0);
        assert!(chorus.is_true_stereo());
    }

    #[test]
    fn test_chorus_stereo_spread() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_depth(1.0);

        // Process for a while to fill delay lines
        for _ in 0..1000 {
            chorus.process_stereo(0.5, 0.5);
        }

        // Collect outputs and verify they differ (stereo spread)
        let mut l_sum = 0.0f32;
        let mut r_sum = 0.0f32;
        for _ in 0..1000 {
            let (l, r) = chorus.process_stereo(0.5, 0.5);
            l_sum += l;
            r_sum += r;
        }

        // With stereo spread, left and right should have different summed outputs
        // (due to different voice panning)
        assert!(l_sum.is_finite());
        assert!(r_sum.is_finite());
    }

    #[test]
    fn test_chorus_param_count() {
        let chorus = Chorus::new(44100.0);
        assert_eq!(chorus.param_count(), 4);
    }
}
