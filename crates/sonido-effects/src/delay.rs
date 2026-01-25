//! Classic delay effect with feedback control.

use sonido_core::{Effect, SmoothedParam, InterpolatedDelay};

/// Classic delay effect with feedback.
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
///
/// let input = 0.5;
/// let output = delay.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Delay {
    delay_line: InterpolatedDelay,
    max_delay_samples: f32,
    delay_time: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    sample_rate: f32,
}

impl Delay {
    /// Create a new delay with 2-second maximum delay.
    pub fn new(sample_rate: f32) -> Self {
        Self::with_max_delay_ms(sample_rate, 2000.0)
    }

    /// Create a new delay with custom maximum delay time.
    pub fn with_max_delay_ms(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_delay_samples = ((max_delay_ms / 1000.0) * sample_rate).ceil() as usize;
        let max_delay_samples_f32 = max_delay_samples as f32;
        let default_delay_samples = ((500.0 / 1000.0) * sample_rate).min(max_delay_samples_f32);

        Self {
            delay_line: InterpolatedDelay::new(max_delay_samples),
            max_delay_samples: max_delay_samples_f32,
            delay_time: SmoothedParam::with_config(default_delay_samples, sample_rate, 50.0),
            feedback: SmoothedParam::with_config(0.3, sample_rate, 10.0),
            mix: SmoothedParam::with_config(0.5, sample_rate, 10.0),
            sample_rate,
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
}

impl Effect for Delay {
    fn process(&mut self, input: f32) -> f32 {
        let delay_samples = self.delay_time.next();
        let feedback = self.feedback.next();
        let mix = self.mix.next();

        let delayed = self.delay_line.read(delay_samples);
        let feedback_signal = input + (delayed * feedback);
        self.delay_line.write(feedback_signal);

        input * (1.0 - mix) + delayed * mix
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.delay_time.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.delay_line.clear();
        self.delay_time.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
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
}
