//! Allpass filter for reverb diffusion.
//!
//! A Schroeder allpass filter that adds diffusion without coloring the
//! frequency response. Essential for creating dense, smooth reverb tails.

use crate::InterpolatedDelay;
use crate::flush_denormal;

/// Schroeder allpass filter for diffusion.
///
/// Allpass filters pass all frequencies at equal amplitude but modify
/// the phase. In reverb, they "smear" the impulse response, creating
/// a denser, more diffuse sound.
///
/// # Example
///
/// ```rust
/// use sonido_core::AllpassFilter;
///
/// let mut allpass = AllpassFilter::new(500);
/// allpass.set_feedback(0.5);
///
/// let output = allpass.process(1.0);
/// ```
#[derive(Debug, Clone)]
pub struct AllpassFilter {
    delay: InterpolatedDelay,
    feedback: f32,
}

impl AllpassFilter {
    /// Create a new allpass filter with the given delay size in samples.
    ///
    /// # Arguments
    ///
    /// * `delay_samples` - The delay length in samples
    pub fn new(delay_samples: usize) -> Self {
        Self {
            delay: InterpolatedDelay::new(delay_samples),
            feedback: 0.5,
        }
    }

    /// Set the feedback coefficient.
    ///
    /// Typical values are around 0.5 for reverb diffusion.
    /// The allpass is stable for |feedback| < 1.0.
    #[inline]
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = feedback.clamp(-0.99, 0.99);
    }

    /// Get the current feedback value.
    #[inline]
    pub fn feedback(&self) -> f32 {
        self.feedback
    }

    /// Process a single sample through the allpass filter.
    ///
    /// Uses the Schroeder allpass structure:
    /// output = -input + delayed
    /// delay_input = input + delayed * feedback
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let delay_samples = (self.delay.capacity() - 1) as f32;
        let delayed = self.delay.read(delay_samples);

        // Schroeder allpass: output = -input + delayed
        let output = -input + delayed;

        // Feed forward: input + delayed * feedback
        self.delay
            .write(flush_denormal(input + delayed * self.feedback));

        output
    }

    /// Clear the allpass filter state.
    pub fn clear(&mut self) {
        self.delay.clear();
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
    fn test_allpass_basic() {
        let mut allpass = AllpassFilter::new(100);
        allpass.set_feedback(0.5);

        for _ in 0..200 {
            let out = allpass.process(0.5);
            assert!(out.is_finite());
        }
    }

    #[test]
    fn test_allpass_energy_conservation() {
        // Allpass should preserve energy (approximately)
        let mut allpass = AllpassFilter::new(50);
        allpass.set_feedback(0.5);

        let input_energy: f32 = (0..500)
            .map(|i| {
                let x = if i < 100 { 1.0 } else { 0.0 };
                x * x
            })
            .sum();

        let output_energy: f32 = (0..500)
            .map(|i| {
                let x = if i < 100 { 1.0 } else { 0.0 };
                let y = allpass.process(x);
                y * y
            })
            .sum();

        // Should be within 20% (not exact due to transient behavior)
        let ratio = output_energy / input_energy;
        assert!(
            ratio > 0.5 && ratio < 2.0,
            "Energy ratio {} should be close to 1.0",
            ratio
        );
    }

    #[test]
    fn test_allpass_clear() {
        let mut allpass = AllpassFilter::new(10);

        // Fill with signal
        for _ in 0..20 {
            allpass.process(1.0);
        }

        allpass.clear();

        // After clear, processing zeros should give small output
        // (due to -input term, first output will be -0 = 0)
        let out = allpass.process(0.0);
        assert!(out.abs() < 1e-10, "Should be silent after clear");
    }

    #[test]
    fn test_allpass_impulse_response() {
        let mut allpass = AllpassFilter::new(10);
        allpass.set_feedback(0.5);

        // Impulse
        let first = allpass.process(1.0);
        assert!(
            (first - (-1.0)).abs() < 0.01,
            "First output should be -input"
        );

        // Wait for delay
        for _ in 0..9 {
            allpass.process(0.0);
        }

        // Delayed impulse should appear
        let delayed = allpass.process(0.0);
        assert!(delayed.abs() > 0.3, "Should have delayed output");
    }

    #[test]
    fn test_no_denormals_after_silence() {
        let mut allpass = AllpassFilter::new(100);
        allpass.set_feedback(0.7);

        // Feed signal for 1000 samples to build up internal state
        for _ in 0..1000 {
            allpass.process(0.5);
        }

        // Feed silence for 100k samples -- output should decay cleanly without
        // producing IEEE 754 subnormal values (which start below ~1.2e-38 and
        // cause severe CPU performance degradation on most architectures).
        for i in 0..100_000 {
            let out = allpass.process(0.0);
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
