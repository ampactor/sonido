//! Comb filter for reverb algorithms.
//!
//! A comb filter with feedback and damping (one-pole lowpass in feedback path).
//! Essential building block for Schroeder and Freeverb-style reverbs.

use crate::InterpolatedDelay;
use crate::flush_denormal;

/// Comb filter with feedback and damping.
///
/// The feedback path includes a one-pole lowpass filter for high-frequency
/// damping, simulating the absorption of high frequencies in real acoustic spaces.
///
/// # Example
///
/// ```rust
/// use sonido_core::CombFilter;
///
/// let mut comb = CombFilter::new(1000);
/// comb.set_feedback(0.8);
/// comb.set_damp(0.3);
///
/// let output = comb.process(1.0);
/// ```
#[derive(Debug, Clone)]
pub struct CombFilter {
    delay: InterpolatedDelay,
    feedback: f32,
    damp1: f32,
    damp2: f32,
    filterstore: f32,
}

impl CombFilter {
    /// Create a new comb filter with the given delay size in samples.
    ///
    /// # Arguments
    ///
    /// * `delay_samples` - The delay length in samples
    pub fn new(delay_samples: usize) -> Self {
        Self {
            delay: InterpolatedDelay::new(delay_samples),
            feedback: 0.5,
            damp1: 0.5,
            damp2: 0.5,
            filterstore: 0.0,
        }
    }

    /// Set the feedback amount (0.0 to ~0.98).
    ///
    /// Higher values create longer decay times.
    /// Values above 0.98 may cause instability.
    #[inline]
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(0.0, 0.99);
    }

    /// Get the current feedback value.
    #[inline]
    pub fn feedback(&self) -> f32 {
        self.feedback
    }

    /// Set the damping amount (0.0 to 1.0).
    ///
    /// - 0.0 = no damping (bright)
    /// - 1.0 = full damping (dark/muffled)
    #[inline]
    pub fn set_damp(&mut self, damp: f32) {
        self.damp1 = damp.clamp(0.0, 1.0);
        self.damp2 = 1.0 - self.damp1;
    }

    /// Get the current damping value.
    #[inline]
    pub fn damp(&self) -> f32 {
        self.damp1
    }

    /// Process a single sample through the comb filter.
    ///
    /// The output is the delayed signal, which is then fed back through
    /// a one-pole lowpass filter and into the delay line.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        // Read from the end of the delay line
        let delay_samples = (self.delay.capacity() - 1) as f32;
        let output = self.delay.read(delay_samples);

        // One-pole lowpass in feedback path (damping)
        // filterstore = output * (1 - damp) + filterstore * damp
        self.filterstore = flush_denormal(output * self.damp2 + self.filterstore * self.damp1);

        // Write input + filtered feedback to delay line
        self.delay.write(input + self.filterstore * self.feedback);

        output
    }

    /// Clear the comb filter state.
    pub fn clear(&mut self) {
        self.delay.clear();
        self.filterstore = 0.0;
    }

    /// Get the delay capacity in samples.
    pub fn capacity(&self) -> usize {
        self.delay.capacity()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comb_basic() {
        let mut comb = CombFilter::new(100);
        comb.set_feedback(0.5);
        comb.set_damp(0.2);

        // Process impulse
        let first = comb.process(1.0);
        assert_eq!(first, 0.0); // First output is from empty delay

        // Process silence, wait for echo
        for _ in 0..99 {
            comb.process(0.0);
        }

        // Now we should see the delayed impulse
        let echo = comb.process(0.0);
        assert!(echo.abs() > 0.1, "Should have echo, got {}", echo);
    }

    #[test]
    fn test_comb_feedback_decay() {
        let mut comb = CombFilter::new(10);
        comb.set_feedback(0.8);
        comb.set_damp(0.0);

        // Impulse
        comb.process(1.0);

        // Process many samples
        let mut last_peak = 0.0f32;
        for _ in 0..100 {
            let out = comb.process(0.0);
            if out.abs() > 0.01 {
                // Each echo should be smaller than the last
                if last_peak > 0.0 {
                    assert!(out.abs() <= last_peak + 0.01, "Echo should decay");
                }
                last_peak = out.abs();
            }
        }
    }

    #[test]
    fn test_comb_clear() {
        let mut comb = CombFilter::new(10);

        // Fill with signal
        for _ in 0..20 {
            comb.process(1.0);
        }

        comb.clear();

        // Should output zeros
        for _ in 0..20 {
            let out = comb.process(0.0);
            assert!(out.abs() < 1e-10, "Should be silent after clear");
        }
    }

    #[test]
    fn test_comb_damping() {
        // Compare bright vs damped
        let mut bright = CombFilter::new(20);
        bright.set_feedback(0.8);
        bright.set_damp(0.0);

        let mut dark = CombFilter::new(20);
        dark.set_feedback(0.8);
        dark.set_damp(0.8);

        // Impulse
        bright.process(1.0);
        dark.process(1.0);

        // Collect output
        let mut bright_sum = 0.0f32;
        let mut dark_sum = 0.0f32;

        for _ in 0..200 {
            bright_sum += bright.process(0.0).abs();
            dark_sum += dark.process(0.0).abs();
        }

        // Damped should have less total energy (due to HF loss)
        assert!(dark_sum < bright_sum, "Damped should have less energy");
    }

    #[test]
    fn test_no_denormals_after_silence() {
        let mut comb = CombFilter::new(100);
        comb.set_feedback(0.9);
        comb.set_damp(0.3);

        // Feed signal for 1000 samples to fill delay line and build up feedback
        for _ in 0..1000 {
            comb.process(0.5);
        }

        // Feed silence for 100k samples -- signal should decay cleanly without
        // producing IEEE 754 subnormal values (which start below ~1.2e-38 and
        // cause severe CPU performance degradation on most architectures).
        // The flush_denormal() guard in the feedback path uses a 1e-20 threshold,
        // so we check that no output falls into the actual subnormal range.
        for i in 0..100_000 {
            let out = comb.process(0.0);
            assert!(
                out == 0.0 || out.abs() > f32::MIN_POSITIVE,
                "Denormal detected at sample {}: {:.2e} (below f32::MIN_POSITIVE {:.2e})",
                i,
                out,
                f32::MIN_POSITIVE
            );
        }
    }
}
