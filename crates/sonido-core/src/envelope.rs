//! Envelope follower for tracking signal amplitude.
//!
//! Used for dynamics processing (compressors, gates, ducking),
//! auto-wah effects, and sidechain applications.

use libm::expf;

/// Envelope follower for tracking signal amplitude.
///
/// Uses peak detection with separate attack and release times
/// for natural-sounding dynamics response.
///
/// # Example
///
/// ```rust
/// use sonido_core::EnvelopeFollower;
///
/// let mut env = EnvelopeFollower::new(48000.0);
/// env.set_attack_ms(10.0);
/// env.set_release_ms(100.0);
///
/// let input_sample = 0.5;
/// let envelope_level = env.process(input_sample);
/// ```
#[derive(Debug, Clone)]
pub struct EnvelopeFollower {
    /// Current envelope level (linear)
    envelope: f32,
    /// Attack coefficient
    attack_coeff: f32,
    /// Release coefficient
    release_coeff: f32,
    /// Sample rate
    sample_rate: f32,
    /// Attack time in ms (for recalculation)
    attack_ms: f32,
    /// Release time in ms (for recalculation)
    release_ms: f32,
}

impl EnvelopeFollower {
    /// Create a new envelope follower with default attack/release times.
    ///
    /// Defaults:
    /// - Attack: 10ms
    /// - Release: 100ms
    pub fn new(sample_rate: f32) -> Self {
        let mut follower = Self {
            envelope: 0.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            sample_rate,
            attack_ms: 10.0,
            release_ms: 100.0,
        };
        follower.recalculate_coefficients();
        follower
    }

    /// Create with specified attack and release times.
    pub fn with_times(sample_rate: f32, attack_ms: f32, release_ms: f32) -> Self {
        let mut follower = Self::new(sample_rate);
        follower.attack_ms = attack_ms;
        follower.release_ms = release_ms;
        follower.recalculate_coefficients();
        follower
    }

    /// Set the attack time in milliseconds.
    ///
    /// Attack is how quickly the envelope rises to match input level.
    /// - Fast (< 5ms): Catch all transients, can sound pumpy
    /// - Medium (5-20ms): General purpose
    /// - Slow (> 20ms): Smooth, may miss fast transients
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.attack_ms = attack_ms.max(0.1);
        self.recalculate_coefficients();
    }

    /// Get current attack time in milliseconds.
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    /// Set the release time in milliseconds.
    ///
    /// Release is how quickly the envelope falls after input decreases.
    /// - Fast (< 50ms): Pumping effect, follows dynamics closely
    /// - Medium (50-200ms): General purpose
    /// - Slow (> 200ms): Smooth, transparent
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.release_ms = release_ms.max(1.0);
        self.recalculate_coefficients();
    }

    /// Get current release time in milliseconds.
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }

    /// Update sample rate and recalculate coefficients.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.recalculate_coefficients();
    }

    /// Process a sample and return the current envelope level.
    ///
    /// Returns the envelope amplitude (always positive).
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let input_abs = input.abs();

        // Choose attack or release based on whether signal is rising or falling
        let coeff = if input_abs > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };

        // Exponential smoothing: y[n] = coeff * y[n-1] + (1 - coeff) * x[n]
        self.envelope = coeff * self.envelope + (1.0 - coeff) * input_abs;
        self.envelope
    }

    /// Get current envelope level without processing new input.
    pub fn level(&self) -> f32 {
        self.envelope
    }

    /// Reset the envelope to zero.
    pub fn reset(&mut self) {
        self.envelope = 0.0;
    }

    fn recalculate_coefficients(&mut self) {
        // Time constant for exponential smoothing
        // coeff = exp(-1 / (time_ms * sample_rate / 1000))
        self.attack_coeff = expf(-1.0 / (self.attack_ms * self.sample_rate / 1000.0));
        self.release_coeff = expf(-1.0 / (self.release_ms * self.sample_rate / 1000.0));
    }
}

impl Default for EnvelopeFollower {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_attack() {
        let mut env = EnvelopeFollower::new(48000.0);
        env.set_attack_ms(1.0); // Fast attack
        env.reset();

        // Feed constant signal
        let mut envelope = 0.0;
        for _ in 0..500 {
            envelope = env.process(1.0);
        }

        // Should have risen close to 1.0
        assert!(envelope > 0.9, "Envelope should rise, got {}", envelope);
    }

    #[test]
    fn test_envelope_release() {
        let mut env = EnvelopeFollower::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_release_ms(10.0);

        // Fill with signal
        for _ in 0..500 {
            env.process(1.0);
        }

        // Now silence
        let mut envelope = 0.0;
        for _ in 0..1000 {
            envelope = env.process(0.0);
        }

        // Should have fallen (after ~2 time constants, expect e^-2 ≈ 0.135)
        assert!(envelope < 0.15, "Envelope should fall, got {}", envelope);
    }

    #[test]
    fn test_envelope_follows_amplitude() {
        let mut env = EnvelopeFollower::new(48000.0);
        env.set_attack_ms(1.0);

        // Negative input should be rectified
        let level = env.process(-0.5);
        assert!(level > 0.0);
    }

    #[test]
    fn test_envelope_reset() {
        let mut env = EnvelopeFollower::new(48000.0);

        for _ in 0..100 {
            env.process(1.0);
        }

        env.reset();
        assert_eq!(env.level(), 0.0);
    }
}
