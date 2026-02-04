//! Classic delay effect with feedback control and stereo ping-pong mode.

use sonido_core::{Effect, SmoothedParam, InterpolatedDelay, ParameterInfo, ParamDescriptor, ParamUnit, flush_denormal};
use libm::ceilf;

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
    delay_line_r: InterpolatedDelay,  // Second delay line for stereo/ping-pong
    max_delay_samples: f32,
    delay_time: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
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
        let default_delay_samples = ((500.0 / 1000.0) * sample_rate).min(max_delay_samples_f32);

        Self {
            delay_line: InterpolatedDelay::new(max_delay_samples),
            delay_line_r: InterpolatedDelay::new(max_delay_samples),
            max_delay_samples: max_delay_samples_f32,
            delay_time: SmoothedParam::with_config(default_delay_samples, sample_rate, 50.0),
            feedback: SmoothedParam::with_config(0.3, sample_rate, 10.0),
            mix: SmoothedParam::with_config(0.5, sample_rate, 10.0),
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

        let delayed = self.delay_line.read(delay_samples);
        let feedback_signal = flush_denormal(input + (delayed * feedback));
        self.delay_line.write(feedback_signal);

        input * (1.0 - mix) + delayed * mix
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let delay_samples = self.delay_time.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();

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

        let out_l = left * (1.0 - mix) + delayed_l * mix;
        let out_r = right * (1.0 - mix) + delayed_r * mix;

        (out_l, out_r)
    }

    fn is_true_stereo(&self) -> bool {
        self.ping_pong
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.delay_time.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.delay_line.clear();
        self.delay_line_r.clear();
        self.delay_time.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
    }
}

impl ParameterInfo for Delay {
    fn param_count(&self) -> usize {
        3
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Delay Time",
                short_name: "Time",
                unit: ParamUnit::Milliseconds,
                min: 1.0,
                max: 2000.0,
                default: 300.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Feedback",
                short_name: "Feedback",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 95.0,
                default: 40.0,
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
            0 => self.delay_time.target() / self.sample_rate * 1000.0,
            1 => self.feedback.target() * 100.0,
            2 => self.mix.target() * 100.0,
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_delay_time_ms(value),
            1 => self.set_feedback(value / 100.0),
            2 => self.set_mix(value / 100.0),
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
        // First echo appears on LEFT after 100ms (from left delay line)
        // Then the feedback crosses to RIGHT delay line
        // Second echo appears on RIGHT after another 100ms
        // So we need to wait 2x delay time (200ms = 8820 samples) to see right echo
        let mut first_l_echo = false;
        let mut first_r_echo = false;
        for _i in 0..15000 {
            let (l, r) = delay.process_stereo(0.0, 0.0);
            if !first_l_echo && l.abs() > 0.5 {
                first_l_echo = true;
            }
            // Right echo should come after left echo (from ping-pong cross-feed)
            if first_l_echo && r.abs() > 0.3 {
                first_r_echo = true;
                break;
            }
            // Don't check too many if we found both
            if first_l_echo && first_r_echo {
                break;
            }
        }
        assert!(first_l_echo, "Should find first echo on left");
        assert!(first_r_echo, "Ping-pong should produce echo on right channel from left input");
    }
}
