//! Envelope follower for tracking signal amplitude.
//!
//! Used for dynamics processing (compressors, gates, ducking),
//! auto-wah effects, and sidechain applications.
//!
//! Supports both peak and RMS detection modes. Peak detection follows
//! instantaneous amplitude; RMS detection follows signal power
//! (root-mean-square) for a smoother, more averaged response.

use libm::{expf, sqrtf};

/// Detection mode for envelope following.
///
/// Determines how the input signal is measured before smoothing.
///
/// - **Peak**: Tracks instantaneous amplitude `|x[n]|`. Faster response,
///   catches transients accurately. Standard for compressors and gates.
/// - **RMS**: Tracks signal power `sqrt(mean(x²))`. Smoother response,
///   better represents perceived loudness. Common in bus compressors
///   and mastering dynamics.
///
/// Reference: Giannoulis et al., "Digital Dynamic Range Compressor Design —
/// A Tutorial and Analysis", JAES 2012.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetectionMode {
    /// Peak detection — follows instantaneous amplitude.
    #[default]
    Peak,
    /// RMS detection — follows signal power (root-mean-square).
    Rms,
}

/// Envelope follower for tracking signal amplitude.
///
/// Uses configurable detection (peak or RMS) with separate attack and
/// release times for natural-sounding dynamics response.
///
/// In **Peak** mode, the follower tracks `|x[n]|` directly.
/// In **RMS** mode, it smooths `x[n]²` then takes `sqrt()` on output,
/// giving a power-based measurement that better represents perceived loudness.
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
    /// Current envelope level (linear amplitude in Peak mode, squared power in RMS mode)
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
    /// Detection mode (Peak or RMS)
    detection_mode: DetectionMode,
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
            detection_mode: DetectionMode::Peak,
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

    /// Set the detection mode (Peak or RMS).
    ///
    /// Changing mode resets the envelope to zero to avoid discontinuities
    /// from the domain change (amplitude vs. squared power).
    pub fn set_detection_mode(&mut self, mode: DetectionMode) {
        if self.detection_mode != mode {
            self.detection_mode = mode;
            self.envelope = 0.0;
        }
    }

    /// Get the current detection mode.
    pub fn detection_mode(&self) -> DetectionMode {
        self.detection_mode
    }

    /// Process a sample and return the current envelope level.
    ///
    /// In Peak mode, returns the smoothed absolute amplitude.
    /// In RMS mode, smooths `x²` internally and returns `sqrt(smoothed)`,
    /// giving the root-mean-square envelope.
    ///
    /// Returns the envelope amplitude (always non-negative).
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let detection = match self.detection_mode {
            DetectionMode::Peak => input.abs(),
            DetectionMode::Rms => input * input,
        };

        // Choose attack or release based on whether signal is rising or falling
        let coeff = if detection > self.envelope {
            self.attack_coeff
        } else {
            self.release_coeff
        };

        // Exponential smoothing: y[n] = coeff * y[n-1] + (1 - coeff) * x[n]
        self.envelope = coeff * self.envelope + (1.0 - coeff) * detection;

        match self.detection_mode {
            DetectionMode::Peak => self.envelope,
            DetectionMode::Rms => sqrtf(self.envelope),
        }
    }

    /// Get current envelope level without processing new input.
    ///
    /// In RMS mode, returns `sqrt(internal_state)` to convert from
    /// squared power domain to amplitude domain.
    pub fn level(&self) -> f32 {
        match self.detection_mode {
            DetectionMode::Peak => self.envelope,
            DetectionMode::Rms => sqrtf(self.envelope),
        }
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

    #[test]
    fn test_rms_detection() {
        use super::DetectionMode;

        let mut env = EnvelopeFollower::new(48000.0);
        env.set_detection_mode(DetectionMode::Rms);
        env.set_attack_ms(1.0);

        // Feed constant signal — RMS of constant = amplitude
        for _ in 0..500 {
            env.process(1.0);
        }
        let level = env.level();
        assert!(
            (level - 1.0).abs() < 0.1,
            "RMS of constant 1.0 should approach 1.0, got {level}"
        );
    }

    #[test]
    fn test_rms_vs_peak_different() {
        use super::DetectionMode;

        // Peak and RMS detection should produce different envelope values
        // for the same signal. With symmetric (equal attack/release) smoothing,
        // RMS of a sine settles to sqrt(0.5) ≈ 0.707 while peak tracks |sin|.
        let sr = 48000.0;
        let time_ms = 200.0; // symmetric — acts as a simple average

        let mut peak_env = EnvelopeFollower::new(sr);
        peak_env.set_attack_ms(time_ms);
        peak_env.set_release_ms(time_ms);

        let mut rms_env = EnvelopeFollower::new(sr);
        rms_env.set_detection_mode(DetectionMode::Rms);
        rms_env.set_attack_ms(time_ms);
        rms_env.set_release_ms(time_ms);

        // 200 Hz sine, 2 seconds — well settled with long time constants
        let n = (sr * 2.0) as usize;
        for i in 0..n {
            let input = libm::sinf(i as f32 * 2.0 * core::f32::consts::PI * 200.0 / sr);
            peak_env.process(input);
            rms_env.process(input);
        }

        let peak_level = peak_env.level();
        let rms_level = rms_env.level();

        // With symmetric smoothing:
        // peak envelope → mean(|sin|) = 2/π ≈ 0.637
        // rms envelope → sqrt(mean(sin²)) = sqrt(0.5) ≈ 0.707
        // Both should be positive and in a reasonable range
        assert!(
            peak_level > 0.5 && peak_level < 1.0,
            "Peak level should be reasonable, got {peak_level:.4}"
        );
        assert!(
            rms_level > 0.5 && rms_level < 1.0,
            "RMS level should be reasonable, got {rms_level:.4}"
        );
        assert!(
            (rms_level - peak_level).abs() > 0.01,
            "Peak ({peak_level:.4}) and RMS ({rms_level:.4}) should differ"
        );
    }

    #[test]
    fn test_detection_mode_switch_resets() {
        use super::DetectionMode;

        let mut env = EnvelopeFollower::new(48000.0);
        env.set_attack_ms(1.0);

        for _ in 0..500 {
            env.process(1.0);
        }
        assert!(env.level() > 0.5);

        env.set_detection_mode(DetectionMode::Rms);
        assert_eq!(env.level(), 0.0, "Switching mode should reset envelope");
    }
}
