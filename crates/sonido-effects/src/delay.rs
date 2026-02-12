//! Classic delay effect with feedback control and stereo ping-pong mode.

use libm::ceilf;
use sonido_core::{
    Effect, InterpolatedDelay, ParamDescriptor, ParameterInfo, SmoothedParam, flush_denormal,
    wet_dry_mix, wet_dry_mix_stereo,
};

/// Classic delay effect with feedback and optional ping-pong stereo mode.
///
/// In mono mode, operates as a standard feedback delay.
/// In stereo mode with ping_pong enabled, creates alternating L/R repeats.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Delay Time | 1.0–2000.0 ms | 300.0 |
/// | 1 | Feedback | 0–95% | 40.0 |
/// | 2 | Mix | 0–100% | 50.0 |
/// | 3 | Ping Pong | 0–1 | 0 |
/// | 4 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Delay;
/// use sonido_core::Effect;
///
/// let mut delay = Delay::new(44100.0);
/// delay.set_delay_time_ms(375.0);
/// delay.set_feedback(0.5);
/// delay.set_mix(0.3);
/// delay.set_ping_pong(true); // Enable stereo ping-pong
///
/// let input = 0.5;
/// let output = delay.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Delay {
    delay_line: InterpolatedDelay,
    delay_line_r: InterpolatedDelay, // Second delay line for stereo/ping-pong
    max_delay_samples: f32,
    delay_time: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    output_level: SmoothedParam,
    sample_rate: f32,
    /// Ping-pong mode: feedback crosses between L/R channels
    ping_pong: bool,
}

impl Delay {
    /// Create a new delay with 2-second maximum delay.
    pub fn new(sample_rate: f32) -> Self {
        Self::with_max_delay_ms(sample_rate, 2000.0)
    }

    /// Create a new delay with custom maximum delay time.
    pub fn with_max_delay_ms(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_delay_samples = ceilf((max_delay_ms / 1000.0) * sample_rate) as usize;
        let max_delay_samples_f32 = max_delay_samples as f32;
        let default_delay_samples = ((300.0 / 1000.0) * sample_rate).min(max_delay_samples_f32);

        Self {
            delay_line: InterpolatedDelay::new(max_delay_samples),
            delay_line_r: InterpolatedDelay::new(max_delay_samples),
            max_delay_samples: max_delay_samples_f32,
            delay_time: SmoothedParam::interpolated(default_delay_samples, sample_rate),
            feedback: SmoothedParam::standard(0.4, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            sample_rate,
            ping_pong: false,
        }
    }

    /// Set delay time in milliseconds.
    pub fn set_delay_time_ms(&mut self, delay_ms: f32) {
        let delay_samples = (delay_ms / 1000.0) * self.sample_rate;
        let clamped = delay_samples.clamp(1.0, self.max_delay_samples - 1.0);
        self.delay_time.set_target(clamped);
    }

    /// Set feedback amount (0-0.95).
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback.set_target(feedback.clamp(0.0, 0.95));
    }

    /// Set wet/dry mix (0-1).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Enable or disable ping-pong stereo mode.
    ///
    /// In ping-pong mode, feedback alternates between left and right channels,
    /// creating a bouncing stereo delay effect.
    pub fn set_ping_pong(&mut self, enabled: bool) {
        self.ping_pong = enabled;
    }

    /// Get current ping-pong mode state.
    pub fn ping_pong(&self) -> bool {
        self.ping_pong
    }
}

impl Effect for Delay {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delay_samples = self.delay_time.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        let delayed = self.delay_line.read(delay_samples);
        let feedback_signal = flush_denormal(input + (delayed * feedback));
        self.delay_line.write(feedback_signal);

        wet_dry_mix(input, delayed, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let delay_samples = self.delay_time.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Read from both delay lines
        let delayed_l = self.delay_line.read(delay_samples);
        let delayed_r = self.delay_line_r.read(delay_samples);

        if self.ping_pong {
            // Ping-pong: feedback crosses channels
            // Left delay feeds back to right, right feeds back to left
            let feedback_l = flush_denormal(left + (delayed_r * feedback));
            let feedback_r = flush_denormal(right + (delayed_l * feedback));
            self.delay_line.write(feedback_l);
            self.delay_line_r.write(feedback_r);
        } else {
            // Standard stereo: independent delay lines
            let feedback_l = flush_denormal(left + (delayed_l * feedback));
            let feedback_r = flush_denormal(right + (delayed_r * feedback));
            self.delay_line.write(feedback_l);
            self.delay_line_r.write(feedback_r);
        }

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, delayed_l, delayed_r, mix);

        (out_l * output_gain, out_r * output_gain)
    }

    fn is_true_stereo(&self) -> bool {
        self.ping_pong
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.delay_time.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.delay_line.clear();
        self.delay_line_r.clear();
        self.delay_time.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();
    }
}

impl ParameterInfo for Delay {
    fn param_count(&self) -> usize {
        5
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor::time_ms(
                "Delay Time",
                "Time",
                1.0,
                2000.0,
                300.0,
            )),
            1 => Some(ParamDescriptor {
                name: "Feedback",
                short_name: "Feedback",
                unit: sonido_core::ParamUnit::Percent,
                min: 0.0,
                max: 95.0,
                default: 40.0,
                step: 1.0,
            }),
            2 => Some(ParamDescriptor::mix()),
            3 => Some(ParamDescriptor {
                name: "Ping Pong",
                short_name: "PngPng",
                unit: sonido_core::ParamUnit::None,
                min: 0.0,
                max: 1.0,
                default: 0.0,
                step: 1.0,
            }),
            4 => Some(sonido_core::gain::output_param_descriptor()),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.delay_time.target() / self.sample_rate * 1000.0,
            1 => self.feedback.target() * 100.0,
            2 => self.mix.target() * 100.0,
            3 => {
                if self.ping_pong {
                    1.0
                } else {
                    0.0
                }
            }
            4 => sonido_core::gain::output_level_db(&self.output_level),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_delay_time_ms(value),
            1 => self.set_feedback(value / 100.0),
            2 => self.set_mix(value / 100.0),
            3 => self.set_ping_pong(value > 0.5),
            4 => sonido_core::gain::set_output_level_db(&mut self.output_level, value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_basic() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_mix(1.0);
        delay.reset();

        // Process impulse
        delay.process(1.0);

        // Look for delayed impulse
        let mut found = false;
        for _ in 0..5000 {
            if delay.process(0.0) > 0.9 {
                found = true;
                break;
            }
        }
        assert!(found, "Should find delayed impulse");
    }

    #[test]
    fn test_delay_bypass() {
        let mut delay = Delay::new(44100.0);
        delay.set_mix(0.0);
        delay.reset();

        let output = delay.process(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_delay_stereo_processing() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_mix(1.0);
        delay.reset();

        // Process stereo impulse
        delay.process_stereo(1.0, 0.5);

        // Look for delayed impulse on both channels
        let mut found_l = false;
        let mut found_r = false;
        for _ in 0..5000 {
            let (l, r) = delay.process_stereo(0.0, 0.0);
            if l > 0.9 {
                found_l = true;
            }
            if r > 0.4 {
                found_r = true;
            }
            if found_l && found_r {
                break;
            }
        }
        assert!(found_l, "Should find delayed impulse on left");
        assert!(found_r, "Should find delayed impulse on right");
    }

    #[test]
    fn test_delay_ping_pong() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_feedback(0.8);
        delay.set_mix(1.0);
        delay.set_ping_pong(true);
        delay.reset();

        assert!(delay.ping_pong());
        assert!(delay.is_true_stereo());

        // Process impulse on left only
        delay.process_stereo(1.0, 0.0);

        // With ping-pong, the feedback should cross channels
        let mut first_l_echo = false;
        let mut first_r_echo = false;
        for _i in 0..15000 {
            let (l, r) = delay.process_stereo(0.0, 0.0);
            if !first_l_echo && l.abs() > 0.5 {
                first_l_echo = true;
            }
            if first_l_echo && r.abs() > 0.3 {
                first_r_echo = true;
                break;
            }
            if first_l_echo && first_r_echo {
                break;
            }
        }
        assert!(first_l_echo, "Should find first echo on left");
        assert!(
            first_r_echo,
            "Ping-pong should produce echo on right channel from left input"
        );
    }

    #[test]
    fn test_delay_param_count() {
        let delay = Delay::new(44100.0);
        assert_eq!(delay.param_count(), 5);
    }
}
