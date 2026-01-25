//! Clean preamp stage
//!
//! High-headroom, zero-latency preamp that handles hot signals
//! without clipping until you want it to.

use sonido_core::Effect;
use libm::{powf, tanhf};

/// Clean preamp stage
///
/// # Example
///
/// ```rust
/// use sonido_effects::CleanPreamp;
/// use sonido_core::Effect;
///
/// let mut preamp = CleanPreamp::new();
/// preamp.set_gain_db(12.0);
/// preamp.set_output_db(-6.0);
///
/// let input = 0.5;
/// let output = preamp.process(input);
/// ```
pub struct CleanPreamp {
    /// Input gain
    gain: f32,
    /// Output level
    output: f32,
    /// Headroom in dB before clipping
    headroom_db: f32,
}

impl Default for CleanPreamp {
    fn default() -> Self {
        Self {
            gain: 1.0,
            output: 1.0,
            headroom_db: 20.0, // +20dB headroom
        }
    }
}

impl CleanPreamp {
    /// Create new clean preamp
    pub fn new() -> Self {
        Self::default()
    }

    /// Set gain in dB
    pub fn set_gain_db(&mut self, db: f32) {
        self.gain = powf(10.0, db / 20.0);
    }

    /// Get current gain in dB
    pub fn gain_db(&self) -> f32 {
        20.0 * libm::log10f(self.gain)
    }

    /// Set output in dB
    pub fn set_output_db(&mut self, db: f32) {
        self.output = powf(10.0, db / 20.0);
    }

    /// Get current output in dB
    pub fn output_db(&self) -> f32 {
        20.0 * libm::log10f(self.output)
    }

    /// Set headroom in dB
    pub fn set_headroom_db(&mut self, db: f32) {
        self.headroom_db = db.clamp(6.0, 40.0);
    }

    /// Get current headroom in dB
    pub fn headroom_db(&self) -> f32 {
        self.headroom_db
    }
}

impl Effect for CleanPreamp {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        // Simple gain stage - clean until clipping threshold
        let gained = input * self.gain;
        let threshold = powf(10.0, self.headroom_db / 20.0);

        // Soft clip only at extreme levels
        let output = if gained.abs() > threshold {
            threshold * gained.signum() * (1.0 + tanhf(gained.abs() / threshold - 1.0))
        } else {
            gained
        };

        output * self.output
    }

    fn reset(&mut self) {}

    fn set_sample_rate(&mut self, _sample_rate: f32) {}

    fn latency_samples(&self) -> usize {
        0 // Zero latency
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preamp_unity() {
        let mut preamp = CleanPreamp::new();
        preamp.set_gain_db(0.0);
        preamp.set_output_db(0.0);

        let output = preamp.process(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_preamp_gain() {
        let mut preamp = CleanPreamp::new();
        preamp.set_gain_db(6.0); // ~2x gain
        preamp.set_output_db(0.0);

        let output = preamp.process(0.25);
        assert!((output - 0.5).abs() < 0.05, "Expected ~0.5, got {}", output);
    }

    #[test]
    fn test_preamp_soft_clip() {
        let mut preamp = CleanPreamp::new();
        preamp.set_gain_db(40.0); // Heavy gain
        preamp.set_headroom_db(6.0); // Low headroom

        // Should soft clip, not hard clip
        let output = preamp.process(0.5);
        assert!(output.is_finite());
        // Should be limited but not exactly at threshold
    }

    #[test]
    fn test_preamp_zero_latency() {
        let preamp = CleanPreamp::new();
        assert_eq!(preamp.latency_samples(), 0);
    }
}
