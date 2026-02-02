//! Tremolo effect with multiple waveform shapes.
//!
//! Classic amplitude modulation effect that creates rhythmic volume variations.

use sonido_core::{Effect, SmoothedParam, Lfo, LfoWaveform, ParameterInfo, ParamDescriptor, ParamUnit};

/// Tremolo waveform type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TremoloWaveform {
    #[default]
    Sine,
    Triangle,
    Square,
    SampleHold,
}

impl TremoloWaveform {
    /// Convert to internal LFO waveform.
    fn to_lfo_waveform(self) -> LfoWaveform {
        match self {
            TremoloWaveform::Sine => LfoWaveform::Sine,
            TremoloWaveform::Triangle => LfoWaveform::Triangle,
            TremoloWaveform::Square => LfoWaveform::Square,
            TremoloWaveform::SampleHold => LfoWaveform::SampleAndHold,
        }
    }

    /// Get waveform from index (for parameter control).
    fn from_index(index: usize) -> Self {
        match index {
            0 => TremoloWaveform::Sine,
            1 => TremoloWaveform::Triangle,
            2 => TremoloWaveform::Square,
            3 => TremoloWaveform::SampleHold,
            _ => TremoloWaveform::Sine,
        }
    }

    /// Get index for waveform (for parameter control).
    fn to_index(self) -> usize {
        match self {
            TremoloWaveform::Sine => 0,
            TremoloWaveform::Triangle => 1,
            TremoloWaveform::Square => 2,
            TremoloWaveform::SampleHold => 3,
        }
    }
}

/// Tremolo effect with configurable waveform.
///
/// Classic amplitude modulation effect using an LFO to modulate gain.
///
/// # Example
///
/// ```rust
/// use sonido_effects::{Tremolo, TremoloWaveform};
/// use sonido_core::Effect;
///
/// let mut tremolo = Tremolo::new(44100.0);
/// tremolo.set_rate(5.0);  // 5 Hz
/// tremolo.set_depth(0.7); // 70% depth
/// tremolo.set_waveform(TremoloWaveform::Triangle);
///
/// let input = 0.5;
/// let output = tremolo.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Tremolo {
    lfo: Lfo,
    rate: SmoothedParam,
    depth: SmoothedParam,
    waveform: TremoloWaveform,
    sample_rate: f32,
}

impl Tremolo {
    /// Create a new tremolo effect.
    pub fn new(sample_rate: f32) -> Self {
        let mut lfo = Lfo::new(sample_rate, 5.0);
        lfo.set_waveform(LfoWaveform::Sine);

        Self {
            lfo,
            rate: SmoothedParam::with_config(5.0, sample_rate, 10.0),
            depth: SmoothedParam::with_config(0.5, sample_rate, 10.0),
            waveform: TremoloWaveform::Sine,
            sample_rate,
        }
    }

    /// Set LFO rate in Hz (0.5-20 Hz).
    pub fn set_rate(&mut self, rate_hz: f32) {
        self.rate.set_target(rate_hz.clamp(0.5, 20.0));
    }

    /// Get current rate in Hz.
    pub fn rate(&self) -> f32 {
        self.rate.target()
    }

    /// Set modulation depth (0-1).
    pub fn set_depth(&mut self, depth: f32) {
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    /// Get current depth.
    pub fn depth(&self) -> f32 {
        self.depth.target()
    }

    /// Set waveform type.
    pub fn set_waveform(&mut self, waveform: TremoloWaveform) {
        self.waveform = waveform;
        self.lfo.set_waveform(waveform.to_lfo_waveform());
    }

    /// Get current waveform.
    pub fn waveform(&self) -> TremoloWaveform {
        self.waveform
    }
}

impl Effect for Tremolo {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let rate = self.rate.advance();
        let depth = self.depth.advance();

        self.lfo.set_frequency(rate);

        // Get unipolar LFO value (0 to 1)
        let lfo_unipolar = self.lfo.advance_unipolar();

        // Calculate gain: 1.0 - (depth * (1.0 - lfo_unipolar))
        // When lfo_unipolar = 1.0, gain = 1.0
        // When lfo_unipolar = 0.0, gain = 1.0 - depth
        let gain = 1.0 - (depth * (1.0 - lfo_unipolar));

        input * gain
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.lfo.set_sample_rate(sample_rate);
        self.rate.set_sample_rate(sample_rate);
        self.depth.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.lfo.reset();
        self.rate.snap_to_target();
        self.depth.snap_to_target();
    }
}

impl ParameterInfo for Tremolo {
    fn param_count(&self) -> usize {
        3
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Rate",
                short_name: "Rate",
                unit: ParamUnit::Hertz,
                min: 0.5,
                max: 20.0,
                default: 5.0,
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
                name: "Waveform",
                short_name: "Wave",
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
            0 => self.rate.target(),
            1 => self.depth.target() * 100.0,
            2 => self.waveform.to_index() as f32,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_rate(value),
            1 => self.set_depth(value / 100.0),
            2 => self.set_waveform(TremoloWaveform::from_index(value as usize)),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tremolo_basic() {
        let mut tremolo = Tremolo::new(44100.0);
        tremolo.set_depth(1.0);

        for _ in 0..1000 {
            let output = tremolo.process(1.0);
            assert!(output.is_finite());
            assert!(output >= 0.0 && output <= 1.0);
        }
    }

    #[test]
    fn test_tremolo_bypass() {
        let mut tremolo = Tremolo::new(44100.0);
        tremolo.set_depth(0.0);

        // Let smoothing settle
        for _ in 0..1000 {
            tremolo.process(1.0);
        }

        let output = tremolo.process(0.5);
        assert!((output - 0.5).abs() < 0.01, "Zero depth should pass signal unchanged");
    }

    #[test]
    fn test_tremolo_full_depth() {
        let mut tremolo = Tremolo::new(44100.0);
        tremolo.set_depth(1.0);
        tremolo.set_rate(10.0);

        // Let smoothing settle
        for _ in 0..1000 {
            tremolo.process(1.0);
        }

        // Collect many samples to find min and max
        let mut min_gain = 1.0f32;
        let mut max_gain = 0.0f32;

        for _ in 0..4410 {
            let output = tremolo.process(1.0);
            min_gain = min_gain.min(output);
            max_gain = max_gain.max(output);
        }

        assert!(min_gain < 0.1, "Full depth should reach near zero, got {}", min_gain);
        assert!(max_gain > 0.9, "Full depth should reach near 1.0, got {}", max_gain);
    }

    #[test]
    fn test_tremolo_waveforms() {
        for waveform in [
            TremoloWaveform::Sine,
            TremoloWaveform::Triangle,
            TremoloWaveform::Square,
            TremoloWaveform::SampleHold,
        ] {
            let mut tremolo = Tremolo::new(44100.0);
            tremolo.set_waveform(waveform);
            tremolo.set_depth(0.5);

            for _ in 0..1000 {
                let output = tremolo.process(1.0);
                assert!(output.is_finite(), "Waveform {:?} produced non-finite output", waveform);
                assert!(output >= 0.0 && output <= 1.0, "Waveform {:?} out of range", waveform);
            }
        }
    }

    #[test]
    fn test_tremolo_parameters() {
        let mut tremolo = Tremolo::new(44100.0);

        assert_eq!(tremolo.param_count(), 3);

        // Test rate parameter
        tremolo.set_param(0, 10.0);
        assert!((tremolo.get_param(0) - 10.0).abs() < 0.01);

        // Test depth parameter (percent)
        tremolo.set_param(1, 75.0);
        assert!((tremolo.get_param(1) - 75.0).abs() < 0.01);

        // Test waveform parameter
        tremolo.set_param(2, 2.0); // Square
        assert!((tremolo.get_param(2) - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_tremolo_reset() {
        let mut tremolo = Tremolo::new(44100.0);

        for _ in 0..1000 {
            tremolo.process(1.0);
        }

        tremolo.reset();

        // After reset, LFO should be at phase 0
        // First output should be predictable
        let output = tremolo.process(1.0);
        assert!(output.is_finite());
    }
}
