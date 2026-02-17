//! Wah effect with auto-wah (envelope) and manual modes.
//!
//! Uses a state variable filter in bandpass mode with envelope follower
//! for classic auto-wah functionality.

use sonido_core::{
    Effect, EnvelopeFollower, ParamDescriptor, ParamFlags, ParamId, ParamScale, ParamUnit,
    ParameterInfo, SmoothedParam, StateVariableFilter, SvfOutput,
};

/// Wah mode selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WahMode {
    /// Auto-wah: envelope controls filter frequency
    #[default]
    Auto,
    /// Manual: frequency controlled by parameter
    Manual,
}

/// Wah effect with auto-wah and manual modes.
///
/// In auto mode, the envelope follower tracks the input signal amplitude
/// and sweeps the filter frequency accordingly. In manual mode, the
/// frequency is controlled directly via the frequency parameter.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Frequency | 200.0–2000.0 Hz | 800.0 |
/// | 1 | Resonance | 1.0–10.0 | 5.0 |
/// | 2 | Sensitivity | 0–100% | 50.0 |
/// | 3 | Mode | 0–1 (Auto, Manual) | 0 |
/// | 4 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Wah;
/// use sonido_core::Effect;
///
/// let mut wah = Wah::new(48000.0);
/// wah.set_frequency(800.0);
/// wah.set_resonance(5.0);
/// wah.set_sensitivity(0.7);
///
/// let output = wah.process(0.5);
/// ```
#[derive(Debug, Clone)]
pub struct Wah {
    /// Bandpass filter for wah sound (left channel)
    filter: StateVariableFilter,
    /// Bandpass filter for wah sound (right channel)
    filter_r: StateVariableFilter,
    /// Envelope follower for auto-wah
    envelope: EnvelopeFollower,
    /// Base/center frequency
    frequency: SmoothedParam,
    /// Resonance (Q factor)
    resonance: SmoothedParam,
    /// Envelope sensitivity (how much envelope affects frequency)
    sensitivity: SmoothedParam,
    /// Current mode (auto or manual)
    mode: WahMode,
    /// Output level
    output_level: SmoothedParam,
    /// Sample rate
    sample_rate: f32,
    /// Minimum frequency for sweep
    min_freq: f32,
    /// Maximum frequency for sweep
    max_freq: f32,
}

impl Default for Wah {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Wah {
    /// Create a new wah effect with default settings.
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = StateVariableFilter::new(sample_rate);
        filter.set_output_type(SvfOutput::Bandpass);
        filter.set_resonance(5.0);
        filter.set_cutoff(800.0);

        let mut filter_r = StateVariableFilter::new(sample_rate);
        filter_r.set_output_type(SvfOutput::Bandpass);
        filter_r.set_resonance(5.0);
        filter_r.set_cutoff(800.0);

        let mut envelope = EnvelopeFollower::new(sample_rate);
        envelope.set_attack_ms(5.0);
        envelope.set_release_ms(50.0);

        Self {
            filter,
            filter_r,
            envelope,
            frequency: SmoothedParam::fast(800.0, sample_rate),
            resonance: SmoothedParam::standard(5.0, sample_rate),
            sensitivity: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            mode: WahMode::Auto,
            sample_rate,
            min_freq: 200.0,
            max_freq: 2000.0,
        }
    }

    /// Set the base/center frequency in Hz (200-2000).
    pub fn set_frequency(&mut self, freq: f32) {
        self.frequency.set_target(freq.clamp(200.0, 2000.0));
    }

    /// Get the current frequency target.
    pub fn frequency(&self) -> f32 {
        self.frequency.target()
    }

    /// Set the resonance/Q factor (1-10).
    pub fn set_resonance(&mut self, q: f32) {
        self.resonance.set_target(q.clamp(1.0, 10.0));
    }

    /// Get the current resonance target.
    pub fn resonance(&self) -> f32 {
        self.resonance.target()
    }

    /// Set envelope sensitivity (0-1).
    ///
    /// Higher values cause more dramatic frequency sweeps in auto mode.
    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity.set_target(sensitivity.clamp(0.0, 1.0));
    }

    /// Get the current sensitivity target.
    pub fn sensitivity(&self) -> f32 {
        self.sensitivity.target()
    }

    /// Set the wah mode.
    pub fn set_mode(&mut self, mode: WahMode) {
        self.mode = mode;
    }

    /// Get the current mode.
    pub fn mode(&self) -> WahMode {
        self.mode
    }

    /// Set mode from integer (0=Auto, 1=Manual).
    pub fn set_mode_index(&mut self, index: usize) {
        self.mode = match index {
            0 => WahMode::Auto,
            _ => WahMode::Manual,
        };
    }
}

impl Effect for Wah {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // Advance smoothed parameters
        let base_freq = self.frequency.advance();
        let resonance = self.resonance.advance();
        let sensitivity = self.sensitivity.advance();

        // Calculate target frequency based on mode
        let target_freq = match self.mode {
            WahMode::Auto => {
                // Get envelope level
                let env_level = self.envelope.process(input);

                // Map envelope to frequency range
                // Sensitivity controls the range of the sweep
                let freq_range = (self.max_freq - self.min_freq) * sensitivity;
                let freq_offset = env_level * freq_range;

                // Start from base frequency and sweep up
                (base_freq + freq_offset).clamp(self.min_freq, self.max_freq)
            }
            WahMode::Manual => {
                // In manual mode, just use the base frequency directly
                // Still process envelope to keep state updated
                self.envelope.process(input);
                base_freq
            }
        };

        // Update filter parameters
        self.filter.set_cutoff(target_freq);
        self.filter.set_resonance(resonance);

        // Process through bandpass filter
        let filtered = self.filter.process(input);

        // SVF bandpass peak gain = Q at center frequency. Normalize for unity gain.
        let normalized = filtered / resonance;

        // Mix filtered signal with dry for body (common in real wah pedals)
        let out = normalized * 0.8 + input * 0.2;
        out * self.output_level.advance()
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let base_freq = self.frequency.advance();
        let resonance = self.resonance.advance();
        let sensitivity = self.sensitivity.advance();

        // Linked envelope detection from combined signal
        let combined = (left + right) * 0.5;

        let target_freq = match self.mode {
            WahMode::Auto => {
                let env_level = self.envelope.process(combined);
                let freq_range = (self.max_freq - self.min_freq) * sensitivity;
                let freq_offset = env_level * freq_range;
                (base_freq + freq_offset).clamp(self.min_freq, self.max_freq)
            }
            WahMode::Manual => {
                self.envelope.process(combined);
                base_freq
            }
        };

        self.filter.set_cutoff(target_freq);
        self.filter.set_resonance(resonance);
        self.filter_r.set_cutoff(target_freq);
        self.filter_r.set_resonance(resonance);

        let filtered_l = self.filter.process(left);
        let out_l = (filtered_l / resonance) * 0.8 + left * 0.2;

        let filtered_r = self.filter_r.process(right);
        let out_r = (filtered_r / resonance) * 0.8 + right * 0.2;

        let output_gain = self.output_level.advance();
        (out_l * output_gain, out_r * output_gain)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.filter.set_sample_rate(sample_rate);
        self.filter_r.set_sample_rate(sample_rate);
        self.envelope.set_sample_rate(sample_rate);
        self.frequency.set_sample_rate(sample_rate);
        self.resonance.set_sample_rate(sample_rate);
        self.sensitivity.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.filter.reset();
        self.filter_r.reset();
        self.envelope.reset();
        self.frequency.snap_to_target();
        self.resonance.snap_to_target();
        self.sensitivity.snap_to_target();
        self.output_level.snap_to_target();
    }
}

impl ParameterInfo for Wah {
    fn param_count(&self) -> usize {
        5
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(
                ParamDescriptor {
                    name: "Frequency",
                    short_name: "Freq",
                    unit: ParamUnit::Hertz,
                    min: 200.0,
                    max: 2000.0,
                    default: 800.0,
                    step: 10.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(600), "wah_freq")
                .with_scale(ParamScale::Logarithmic),
            ),
            1 => Some(
                ParamDescriptor {
                    name: "Resonance",
                    short_name: "Reso",
                    unit: ParamUnit::None,
                    min: 1.0,
                    max: 10.0,
                    default: 5.0,
                    step: 0.1,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(601), "wah_reso"),
            ),
            2 => Some(
                ParamDescriptor {
                    name: "Sensitivity",
                    short_name: "Sens",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(602), "wah_sens"),
            ),
            3 => Some(
                ParamDescriptor {
                    name: "Mode",
                    short_name: "Mode",
                    unit: ParamUnit::None,
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(603), "wah_mode")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED)),
            ),
            4 => Some(
                sonido_core::gain::output_param_descriptor().with_id(ParamId(604), "wah_output"),
            ),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.frequency.target(),
            1 => self.resonance.target(),
            2 => self.sensitivity.target() * 100.0,
            3 => match self.mode {
                WahMode::Auto => 0.0,
                WahMode::Manual => 1.0,
            },
            4 => sonido_core::gain::output_level_db(&self.output_level),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_frequency(value),
            1 => self.set_resonance(value),
            2 => self.set_sensitivity(value / 100.0),
            3 => self.set_mode_index(value as usize),
            4 => sonido_core::gain::set_output_level_db(&mut self.output_level, value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;
    #[cfg(feature = "std")]
    use std::vec::Vec;

    #[test]
    fn test_wah_basic_processing() {
        let mut wah = Wah::new(48000.0);

        // Process some samples
        for _ in 0..1000 {
            let output = wah.process(0.5);
            assert!(output.is_finite(), "Output should be finite");
            assert!(output.abs() < 10.0, "Output should be reasonable");
        }
    }

    #[test]
    fn test_wah_auto_mode() {
        let mut wah = Wah::new(48000.0);
        wah.set_mode(WahMode::Auto);
        wah.set_sensitivity(1.0);

        // Process with varying input levels
        let mut outputs = Vec::new();
        for i in 0..100 {
            let input = if i < 50 { 0.8 } else { 0.1 };
            outputs.push(wah.process(input));
        }

        // All outputs should be finite
        assert!(outputs.iter().all(|x| x.is_finite()));
    }

    #[test]
    fn test_wah_manual_mode() {
        let mut wah = Wah::new(48000.0);
        wah.set_mode(WahMode::Manual);
        wah.set_frequency(1000.0);

        for _ in 0..100 {
            let output = wah.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_wah_parameter_clamping() {
        let mut wah = Wah::new(48000.0);

        // Test frequency clamping
        wah.set_frequency(50.0);
        assert_eq!(wah.frequency(), 200.0);

        wah.set_frequency(5000.0);
        assert_eq!(wah.frequency(), 2000.0);

        // Test resonance clamping
        wah.set_resonance(0.1);
        assert_eq!(wah.resonance(), 1.0);

        wah.set_resonance(20.0);
        assert_eq!(wah.resonance(), 10.0);

        // Test sensitivity clamping
        wah.set_sensitivity(-0.5);
        assert_eq!(wah.sensitivity(), 0.0);

        wah.set_sensitivity(2.0);
        assert_eq!(wah.sensitivity(), 1.0);
    }

    #[test]
    fn test_wah_reset() {
        let mut wah = Wah::new(48000.0);

        // Process some samples
        for _ in 0..100 {
            wah.process(1.0);
        }

        // Reset
        wah.reset();

        // Should work normally after reset
        let output = wah.process(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_wah_sample_rate_change() {
        let mut wah = Wah::new(44100.0);
        wah.set_sample_rate(96000.0);

        let output = wah.process(0.5);
        assert!(output.is_finite());
    }

    #[test]
    fn test_wah_parameter_info() {
        let wah = Wah::new(48000.0);

        assert_eq!(wah.param_count(), 5);

        let freq_info = wah.param_info(0).unwrap();
        assert_eq!(freq_info.name, "Frequency");
        assert_eq!(freq_info.min, 200.0);
        assert_eq!(freq_info.max, 2000.0);

        let mode_info = wah.param_info(3).unwrap();
        assert_eq!(mode_info.name, "Mode");
    }

    #[test]
    fn test_wah_get_set_param() {
        let mut wah = Wah::new(48000.0);

        // Test frequency
        wah.set_param(0, 1000.0);
        assert_eq!(wah.get_param(0), 1000.0);

        // Test resonance
        wah.set_param(1, 7.0);
        assert_eq!(wah.get_param(1), 7.0);

        // Test sensitivity (stored as 0-1, displayed as 0-100)
        wah.set_param(2, 75.0);
        assert_eq!(wah.get_param(2), 75.0);

        // Test mode
        wah.set_param(3, 1.0);
        assert_eq!(wah.get_param(3), 1.0);
        assert_eq!(wah.mode(), WahMode::Manual);
    }
}
