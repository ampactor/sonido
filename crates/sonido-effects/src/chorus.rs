//! Classic chorus effect with dual voices.

use sonido_core::{Effect, SmoothedParam, InterpolatedDelay, Lfo, ParameterInfo, ParamDescriptor, ParamUnit};

/// Chorus effect with dual voices.
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
        let max_delay_samples = ((MAX_DELAY_MS / 1000.0) * sample_rate).ceil() as usize;

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
            rate: SmoothedParam::with_config(1.0, sample_rate, 10.0),
            depth: SmoothedParam::with_config(0.5, sample_rate, 10.0),
            mix: SmoothedParam::with_config(0.5, sample_rate, 10.0),
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
        input * (1.0 - mix) + wet * mix
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
    }

    fn reset(&mut self) {
        self.delay1.clear();
        self.delay2.clear();
        self.lfo1.reset();
        self.lfo2.reset();
    }
}

impl ParameterInfo for Chorus {
    fn param_count(&self) -> usize {
        3
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Rate",
                short_name: "Rate",
                unit: ParamUnit::Hertz,
                min: 0.1,
                max: 10.0,
                default: 1.0,
                step: 0.1,
            }),
            1 => Some(ParamDescriptor {
                name: "Depth",
                short_name: "Depth",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 50.0,
                step: 1.0,
            }),
            2 => Some(ParamDescriptor {
                name: "Mix",
                short_name: "Mix",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 50.0,
                step: 1.0,
            }),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.rate.target(),
            1 => self.depth.target() * 100.0,
            2 => self.mix.target() * 100.0,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_rate(value),
            1 => self.set_depth(value / 100.0),
            2 => self.set_mix(value / 100.0),
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
}
