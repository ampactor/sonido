//! 3-band parametric equalizer.
//!
//! Uses RBJ cookbook peaking EQ filters for precise frequency shaping.

use sonido_core::{
    Biquad, Effect, ParameterInfo, ParamDescriptor, ParamUnit, SmoothedParam,
    peaking_eq_coefficients,
};

/// 3-band parametric equalizer.
///
/// Each band has independent frequency, gain, and Q (bandwidth) controls.
/// Uses cascaded biquad filters in peaking EQ mode for high-quality equalization.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Low Frequency | 20.0–500.0 Hz | 100.0 |
/// | 1 | Low Gain | -12.0–12.0 dB | 0.0 |
/// | 2 | Low Q | 0.5–5.0 | 1.0 |
/// | 3 | Mid Frequency | 200.0–5000.0 Hz | 1000.0 |
/// | 4 | Mid Gain | -12.0–12.0 dB | 0.0 |
/// | 5 | Mid Q | 0.5–5.0 | 1.0 |
/// | 6 | High Frequency | 1000.0–15000.0 Hz | 5000.0 |
/// | 7 | High Gain | -12.0–12.0 dB | 0.0 |
/// | 8 | High Q | 0.5–5.0 | 1.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::ParametricEq;
/// use sonido_core::Effect;
///
/// let mut eq = ParametricEq::new(48000.0);
///
/// // Boost low mids, cut highs
/// eq.set_low_freq(150.0);
/// eq.set_low_gain(3.0);
///
/// eq.set_mid_freq(800.0);
/// eq.set_mid_gain(2.0);
///
/// eq.set_high_freq(4000.0);
/// eq.set_high_gain(-4.0);
///
/// let output = eq.process(0.5);
/// ```
#[derive(Debug, Clone)]
pub struct ParametricEq {
    /// Low band biquad filter
    low_filter: Biquad,
    /// Mid band biquad filter
    mid_filter: Biquad,
    /// High band biquad filter
    high_filter: Biquad,

    // Low band parameters
    low_freq: SmoothedParam,
    low_gain: SmoothedParam,
    low_q: SmoothedParam,

    // Mid band parameters
    mid_freq: SmoothedParam,
    mid_gain: SmoothedParam,
    mid_q: SmoothedParam,

    // High band parameters
    high_freq: SmoothedParam,
    high_gain: SmoothedParam,
    high_q: SmoothedParam,

    /// Sample rate
    sample_rate: f32,

    /// Flags for coefficient updates
    low_needs_update: bool,
    mid_needs_update: bool,
    high_needs_update: bool,
}

impl Default for ParametricEq {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl ParametricEq {
    /// Create a new 3-band parametric EQ with default settings.
    ///
    /// Defaults:
    /// - Low: 100 Hz, 0 dB, Q=1.0
    /// - Mid: 1000 Hz, 0 dB, Q=1.0
    /// - High: 5000 Hz, 0 dB, Q=1.0
    pub fn new(sample_rate: f32) -> Self {
        let mut eq = Self {
            low_filter: Biquad::new(),
            mid_filter: Biquad::new(),
            high_filter: Biquad::new(),

            low_freq: SmoothedParam::with_config(100.0, sample_rate, 20.0),
            low_gain: SmoothedParam::with_config(0.0, sample_rate, 10.0),
            low_q: SmoothedParam::with_config(1.0, sample_rate, 20.0),

            mid_freq: SmoothedParam::with_config(1000.0, sample_rate, 20.0),
            mid_gain: SmoothedParam::with_config(0.0, sample_rate, 10.0),
            mid_q: SmoothedParam::with_config(1.0, sample_rate, 20.0),

            high_freq: SmoothedParam::with_config(5000.0, sample_rate, 20.0),
            high_gain: SmoothedParam::with_config(0.0, sample_rate, 10.0),
            high_q: SmoothedParam::with_config(1.0, sample_rate, 20.0),

            sample_rate,
            low_needs_update: true,
            mid_needs_update: true,
            high_needs_update: true,
        };

        eq.update_all_coefficients();
        eq
    }

    // Low band setters

    /// Set low band center frequency in Hz (20-500).
    pub fn set_low_freq(&mut self, freq: f32) {
        self.low_freq.set_target(freq.clamp(20.0, 500.0));
        self.low_needs_update = true;
    }

    /// Get low band frequency.
    pub fn low_freq(&self) -> f32 {
        self.low_freq.target()
    }

    /// Set low band gain in dB (-12 to +12).
    pub fn set_low_gain(&mut self, gain_db: f32) {
        self.low_gain.set_target(gain_db.clamp(-12.0, 12.0));
        self.low_needs_update = true;
    }

    /// Get low band gain.
    pub fn low_gain(&self) -> f32 {
        self.low_gain.target()
    }

    /// Set low band Q (0.5-5.0).
    pub fn set_low_q(&mut self, q: f32) {
        self.low_q.set_target(q.clamp(0.5, 5.0));
        self.low_needs_update = true;
    }

    /// Get low band Q.
    pub fn low_q(&self) -> f32 {
        self.low_q.target()
    }

    // Mid band setters

    /// Set mid band center frequency in Hz (200-5000).
    pub fn set_mid_freq(&mut self, freq: f32) {
        self.mid_freq.set_target(freq.clamp(200.0, 5000.0));
        self.mid_needs_update = true;
    }

    /// Get mid band frequency.
    pub fn mid_freq(&self) -> f32 {
        self.mid_freq.target()
    }

    /// Set mid band gain in dB (-12 to +12).
    pub fn set_mid_gain(&mut self, gain_db: f32) {
        self.mid_gain.set_target(gain_db.clamp(-12.0, 12.0));
        self.mid_needs_update = true;
    }

    /// Get mid band gain.
    pub fn mid_gain(&self) -> f32 {
        self.mid_gain.target()
    }

    /// Set mid band Q (0.5-5.0).
    pub fn set_mid_q(&mut self, q: f32) {
        self.mid_q.set_target(q.clamp(0.5, 5.0));
        self.mid_needs_update = true;
    }

    /// Get mid band Q.
    pub fn mid_q(&self) -> f32 {
        self.mid_q.target()
    }

    // High band setters

    /// Set high band center frequency in Hz (1000-15000).
    pub fn set_high_freq(&mut self, freq: f32) {
        self.high_freq.set_target(freq.clamp(1000.0, 15000.0));
        self.high_needs_update = true;
    }

    /// Get high band frequency.
    pub fn high_freq(&self) -> f32 {
        self.high_freq.target()
    }

    /// Set high band gain in dB (-12 to +12).
    pub fn set_high_gain(&mut self, gain_db: f32) {
        self.high_gain.set_target(gain_db.clamp(-12.0, 12.0));
        self.high_needs_update = true;
    }

    /// Get high band gain.
    pub fn high_gain(&self) -> f32 {
        self.high_gain.target()
    }

    /// Set high band Q (0.5-5.0).
    pub fn set_high_q(&mut self, q: f32) {
        self.high_q.set_target(q.clamp(0.5, 5.0));
        self.high_needs_update = true;
    }

    /// Get high band Q.
    pub fn high_q(&self) -> f32 {
        self.high_q.target()
    }

    /// Clamp frequency to stay below Nyquist (with margin) to prevent
    /// unstable biquad coefficients when sample rate is low.
    fn clamp_to_nyquist(&self, freq: f32) -> f32 {
        // Clamp to 95% of Nyquist to avoid numerical instability near the limit
        let max_freq = self.sample_rate * 0.475;
        if freq > max_freq { max_freq } else { freq }
    }

    fn update_low_coefficients(&mut self) {
        let freq = self.clamp_to_nyquist(self.low_freq.get());
        let gain = self.low_gain.get();
        let q = self.low_q.get();

        let (b0, b1, b2, a0, a1, a2) = peaking_eq_coefficients(freq, q, gain, self.sample_rate);
        self.low_filter.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.low_needs_update = false;
    }

    fn update_mid_coefficients(&mut self) {
        let freq = self.clamp_to_nyquist(self.mid_freq.get());
        let gain = self.mid_gain.get();
        let q = self.mid_q.get();

        let (b0, b1, b2, a0, a1, a2) = peaking_eq_coefficients(freq, q, gain, self.sample_rate);
        self.mid_filter.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.mid_needs_update = false;
    }

    fn update_high_coefficients(&mut self) {
        let freq = self.clamp_to_nyquist(self.high_freq.get());
        let gain = self.high_gain.get();
        let q = self.high_q.get();

        let (b0, b1, b2, a0, a1, a2) = peaking_eq_coefficients(freq, q, gain, self.sample_rate);
        self.high_filter.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.high_needs_update = false;
    }

    fn update_all_coefficients(&mut self) {
        self.update_low_coefficients();
        self.update_mid_coefficients();
        self.update_high_coefficients();
    }
}

impl Effect for ParametricEq {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // Advance smoothed parameters
        self.low_freq.advance();
        self.low_gain.advance();
        self.low_q.advance();
        self.mid_freq.advance();
        self.mid_gain.advance();
        self.mid_q.advance();
        self.high_freq.advance();
        self.high_gain.advance();
        self.high_q.advance();

        // Update coefficients if needed
        if self.low_needs_update || !self.low_freq.is_settled() || !self.low_gain.is_settled() || !self.low_q.is_settled() {
            self.update_low_coefficients();
        }
        if self.mid_needs_update || !self.mid_freq.is_settled() || !self.mid_gain.is_settled() || !self.mid_q.is_settled() {
            self.update_mid_coefficients();
        }
        if self.high_needs_update || !self.high_freq.is_settled() || !self.high_gain.is_settled() || !self.high_q.is_settled() {
            self.update_high_coefficients();
        }

        // Process through cascaded filters
        let after_low = self.low_filter.process(input);
        let after_mid = self.mid_filter.process(after_low);
        self.high_filter.process(after_mid)
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Dual-mono: process each channel through the same filter cascade
        // Note: The biquad filters share state, so processing is interleaved.
        // For true dual-mono EQ, separate filter instances would be needed per channel.

        // Advance smoothed parameters once
        self.low_freq.advance();
        self.low_gain.advance();
        self.low_q.advance();
        self.mid_freq.advance();
        self.mid_gain.advance();
        self.mid_q.advance();
        self.high_freq.advance();
        self.high_gain.advance();
        self.high_q.advance();

        // Update coefficients if needed
        if self.low_needs_update || !self.low_freq.is_settled() || !self.low_gain.is_settled() || !self.low_q.is_settled() {
            self.update_low_coefficients();
        }
        if self.mid_needs_update || !self.mid_freq.is_settled() || !self.mid_gain.is_settled() || !self.mid_q.is_settled() {
            self.update_mid_coefficients();
        }
        if self.high_needs_update || !self.high_freq.is_settled() || !self.high_gain.is_settled() || !self.high_q.is_settled() {
            self.update_high_coefficients();
        }

        // Process left channel
        let left_low = self.low_filter.process(left);
        let left_mid = self.mid_filter.process(left_low);
        let left_out = self.high_filter.process(left_mid);

        // Process right channel (filter state is shared)
        let right_low = self.low_filter.process(right);
        let right_mid = self.mid_filter.process(right_low);
        let right_out = self.high_filter.process(right_mid);

        (left_out, right_out)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        self.low_freq.set_sample_rate(sample_rate);
        self.low_gain.set_sample_rate(sample_rate);
        self.low_q.set_sample_rate(sample_rate);
        self.mid_freq.set_sample_rate(sample_rate);
        self.mid_gain.set_sample_rate(sample_rate);
        self.mid_q.set_sample_rate(sample_rate);
        self.high_freq.set_sample_rate(sample_rate);
        self.high_gain.set_sample_rate(sample_rate);
        self.high_q.set_sample_rate(sample_rate);

        self.low_needs_update = true;
        self.mid_needs_update = true;
        self.high_needs_update = true;
        self.update_all_coefficients();
    }

    fn reset(&mut self) {
        self.low_filter.clear();
        self.mid_filter.clear();
        self.high_filter.clear();

        self.low_freq.snap_to_target();
        self.low_gain.snap_to_target();
        self.low_q.snap_to_target();
        self.mid_freq.snap_to_target();
        self.mid_gain.snap_to_target();
        self.mid_q.snap_to_target();
        self.high_freq.snap_to_target();
        self.high_gain.snap_to_target();
        self.high_q.snap_to_target();

        self.low_needs_update = true;
        self.mid_needs_update = true;
        self.high_needs_update = true;
        self.update_all_coefficients();
    }
}

impl ParameterInfo for ParametricEq {
    fn param_count(&self) -> usize {
        9 // 3 params per band x 3 bands
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            // Low band
            0 => Some(ParamDescriptor {
                name: "Low Frequency",
                short_name: "LowFreq",
                unit: ParamUnit::Hertz,
                min: 20.0,
                max: 500.0,
                default: 100.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Low Gain",
                short_name: "LowGain",
                unit: ParamUnit::Decibels,
                min: -12.0,
                max: 12.0,
                default: 0.0,
                step: 0.5,
            }),
            2 => Some(ParamDescriptor {
                name: "Low Q",
                short_name: "LowQ",
                unit: ParamUnit::None,
                min: 0.5,
                max: 5.0,
                default: 1.0,
                step: 0.1,
            }),
            // Mid band
            3 => Some(ParamDescriptor {
                name: "Mid Frequency",
                short_name: "MidFreq",
                unit: ParamUnit::Hertz,
                min: 200.0,
                max: 5000.0,
                default: 1000.0,
                step: 10.0,
            }),
            4 => Some(ParamDescriptor {
                name: "Mid Gain",
                short_name: "MidGain",
                unit: ParamUnit::Decibels,
                min: -12.0,
                max: 12.0,
                default: 0.0,
                step: 0.5,
            }),
            5 => Some(ParamDescriptor {
                name: "Mid Q",
                short_name: "MidQ",
                unit: ParamUnit::None,
                min: 0.5,
                max: 5.0,
                default: 1.0,
                step: 0.1,
            }),
            // High band
            6 => Some(ParamDescriptor {
                name: "High Frequency",
                short_name: "HighFreq",
                unit: ParamUnit::Hertz,
                min: 1000.0,
                max: 15000.0,
                default: 5000.0,
                step: 100.0,
            }),
            7 => Some(ParamDescriptor {
                name: "High Gain",
                short_name: "HighGain",
                unit: ParamUnit::Decibels,
                min: -12.0,
                max: 12.0,
                default: 0.0,
                step: 0.5,
            }),
            8 => Some(ParamDescriptor {
                name: "High Q",
                short_name: "HighQ",
                unit: ParamUnit::None,
                min: 0.5,
                max: 5.0,
                default: 1.0,
                step: 0.1,
            }),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.low_freq.target(),
            1 => self.low_gain.target(),
            2 => self.low_q.target(),
            3 => self.mid_freq.target(),
            4 => self.mid_gain.target(),
            5 => self.mid_q.target(),
            6 => self.high_freq.target(),
            7 => self.high_gain.target(),
            8 => self.high_q.target(),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_low_freq(value),
            1 => self.set_low_gain(value),
            2 => self.set_low_q(value),
            3 => self.set_mid_freq(value),
            4 => self.set_mid_gain(value),
            5 => self.set_mid_q(value),
            6 => self.set_high_freq(value),
            7 => self.set_high_gain(value),
            8 => self.set_high_q(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eq_flat_response() {
        let mut eq = ParametricEq::new(48000.0);

        // With all gains at 0, DC should pass through unchanged
        let mut output = 0.0;
        for _ in 0..1000 {
            output = eq.process(1.0);
        }

        assert!((output - 1.0).abs() < 0.05, "Flat EQ should pass DC unchanged, got {}", output);
    }

    #[test]
    fn test_eq_basic_processing() {
        let mut eq = ParametricEq::new(48000.0);
        eq.set_low_gain(6.0);
        eq.set_mid_gain(-3.0);
        eq.set_high_gain(3.0);

        for _ in 0..1000 {
            let output = eq.process(0.5);
            assert!(output.is_finite(), "Output should be finite");
        }
    }

    #[test]
    fn test_eq_parameter_clamping() {
        let mut eq = ParametricEq::new(48000.0);

        // Test frequency clamping
        eq.set_low_freq(1.0);
        assert_eq!(eq.low_freq(), 20.0);

        eq.set_low_freq(1000.0);
        assert_eq!(eq.low_freq(), 500.0);

        // Test gain clamping
        eq.set_mid_gain(-20.0);
        assert_eq!(eq.mid_gain(), -12.0);

        eq.set_mid_gain(20.0);
        assert_eq!(eq.mid_gain(), 12.0);

        // Test Q clamping
        eq.set_high_q(0.1);
        assert_eq!(eq.high_q(), 0.5);

        eq.set_high_q(10.0);
        assert_eq!(eq.high_q(), 5.0);
    }

    #[test]
    fn test_eq_reset() {
        let mut eq = ParametricEq::new(48000.0);
        eq.set_low_gain(6.0);

        // Process some samples
        for _ in 0..100 {
            eq.process(1.0);
        }

        // Reset
        eq.reset();

        // Should work normally after reset
        let output = eq.process(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_eq_sample_rate_change() {
        let mut eq = ParametricEq::new(44100.0);
        eq.set_sample_rate(96000.0);

        let output = eq.process(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_eq_parameter_info() {
        let eq = ParametricEq::new(48000.0);

        assert_eq!(eq.param_count(), 9);

        // Check low band params
        let low_freq = eq.param_info(0).unwrap();
        assert_eq!(low_freq.name, "Low Frequency");
        assert_eq!(low_freq.min, 20.0);
        assert_eq!(low_freq.max, 500.0);

        // Check mid band params
        let mid_gain = eq.param_info(4).unwrap();
        assert_eq!(mid_gain.name, "Mid Gain");
        assert_eq!(mid_gain.unit, ParamUnit::Decibels);

        // Check high band params
        let high_q = eq.param_info(8).unwrap();
        assert_eq!(high_q.name, "High Q");
    }

    #[test]
    fn test_eq_get_set_param() {
        let mut eq = ParametricEq::new(48000.0);

        // Test all 9 parameters
        eq.set_param(0, 150.0);
        assert_eq!(eq.get_param(0), 150.0);

        eq.set_param(1, 3.0);
        assert_eq!(eq.get_param(1), 3.0);

        eq.set_param(2, 2.0);
        assert_eq!(eq.get_param(2), 2.0);

        eq.set_param(3, 800.0);
        assert_eq!(eq.get_param(3), 800.0);

        eq.set_param(4, -6.0);
        assert_eq!(eq.get_param(4), -6.0);

        eq.set_param(5, 1.5);
        assert_eq!(eq.get_param(5), 1.5);

        eq.set_param(6, 8000.0);
        assert_eq!(eq.get_param(6), 8000.0);

        eq.set_param(7, 4.0);
        assert_eq!(eq.get_param(7), 4.0);

        eq.set_param(8, 3.0);
        assert_eq!(eq.get_param(8), 3.0);
    }

    #[test]
    fn test_eq_out_of_bounds_param() {
        let mut eq = ParametricEq::new(48000.0);

        // Out of bounds should return 0.0 for get
        assert_eq!(eq.get_param(99), 0.0);

        // Out of bounds set should do nothing (no panic)
        eq.set_param(99, 42.0);
    }
}
