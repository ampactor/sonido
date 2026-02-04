//! DC blocking filter for removing DC offset from audio signals.
//!
//! Uses a first-order highpass filter (Julius O. Smith's DC blocker).
//! Transfer function: H(z) = (1 - z^-1) / (1 - R*z^-1)
//!
//! where R is a coefficient close to 1.0 that controls the cutoff frequency.
//! At 48 kHz, R = 0.995 gives a cutoff of approximately 7.6 Hz.
//!
//! Reference: Julius O. Smith, "Introduction to Digital Filters with Audio
//! Applications", Chapter on DC Blocker.

use core::f32::consts::PI;

/// DC blocking filter using a first-order highpass.
///
/// Removes DC offset from audio signals while preserving all audible content.
/// The cutoff frequency is typically below 10 Hz, well below the audible range.
///
/// ## Parameters
/// - `coeff`: R coefficient controlling cutoff (~0.995 for ~7 Hz at 48 kHz)
///
/// ## Transfer Function
///
/// ```text
/// H(z) = (1 - z^-1) / (1 - R * z^-1)
/// ```
///
/// The -3 dB cutoff frequency is: f_c = (1 - R) / (2 * pi) * f_s
///
/// ## Example
///
/// ```rust
/// use sonido_core::DcBlocker;
///
/// let mut blocker = DcBlocker::new(48000.0);
///
/// // Process a signal with DC offset
/// let input = 0.5 + 0.1; // 0.1 DC offset
/// let output = blocker.process(input);
/// ```
///
/// Reference: Julius O. Smith, "Introduction to Digital Filters"
pub struct DcBlocker {
    /// R coefficient (pole position, controls cutoff frequency)
    coeff: f32,
    /// Previous input sample x[n-1]
    x_prev: f32,
    /// Previous output sample y[n-1]
    y_prev: f32,
}

impl DcBlocker {
    /// Default cutoff frequency target in Hz.
    const DEFAULT_CUTOFF_HZ: f32 = 7.0;

    /// Create a new DC blocker for the given sample rate.
    ///
    /// The cutoff frequency defaults to approximately 7 Hz, which is well
    /// below the audible range but effectively removes DC offset.
    ///
    /// # Arguments
    /// * `sample_rate` - Sample rate in Hz
    pub fn new(sample_rate: f32) -> Self {
        let coeff = Self::calculate_coeff(Self::DEFAULT_CUTOFF_HZ, sample_rate);
        Self {
            coeff,
            x_prev: 0.0,
            y_prev: 0.0,
        }
    }

    /// Create a new DC blocker with a specific R coefficient.
    ///
    /// # Arguments
    /// * `coeff` - R coefficient (typically 0.99 to 0.999). Higher values
    ///   give a lower cutoff frequency. Values are clamped to [0.9, 0.9999].
    pub fn with_coeff(coeff: f32) -> Self {
        Self {
            coeff: coeff.clamp(0.9, 0.9999),
            x_prev: 0.0,
            y_prev: 0.0,
        }
    }

    /// Process a single sample through the DC blocker.
    ///
    /// Implements: y[n] = x[n] - x[n-1] + R * y[n-1]
    ///
    /// # Arguments
    /// * `input` - Input sample
    ///
    /// # Returns
    /// Filtered output with DC removed
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let output = input - self.x_prev + self.coeff * self.y_prev;
        self.x_prev = input;
        self.y_prev = output;
        output
    }

    /// Reset the filter state to zero.
    pub fn reset(&mut self) {
        self.x_prev = 0.0;
        self.y_prev = 0.0;
    }

    /// Update the sample rate, recalculating the R coefficient to maintain
    /// the same cutoff frequency (~7 Hz).
    ///
    /// # Arguments
    /// * `sample_rate` - New sample rate in Hz
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.coeff = Self::calculate_coeff(Self::DEFAULT_CUTOFF_HZ, sample_rate);
    }

    /// Get the current R coefficient.
    pub fn coeff(&self) -> f32 {
        self.coeff
    }

    /// Calculate the R coefficient for a desired cutoff frequency.
    ///
    /// Formula: R = 1 - 2*pi*fc/fs
    ///
    /// # Arguments
    /// * `cutoff_hz` - Desired cutoff frequency in Hz
    /// * `sample_rate` - Sample rate in Hz
    fn calculate_coeff(cutoff_hz: f32, sample_rate: f32) -> f32 {
        let r = 1.0 - (2.0 * PI * cutoff_hz / sample_rate);
        r.clamp(0.9, 0.9999)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dc_blocker_removes_dc() {
        let mut blocker = DcBlocker::new(48000.0);

        // Process a constant DC signal for long enough to settle
        let mut output = 0.0;
        for _ in 0..48000 {
            output = blocker.process(1.0);
        }

        // DC component should be largely removed
        assert!(
            output.abs() < 0.01,
            "DC should be removed, got {}",
            output
        );
    }

    #[test]
    fn test_dc_blocker_passes_ac() {
        let mut blocker = DcBlocker::new(48000.0);
        let freq = 1000.0; // 1 kHz test tone
        let sample_rate = 48000.0;

        // Let the filter settle with the tone
        for i in 0..48000 {
            let t = i as f32 / sample_rate;
            let input = libm::sinf(2.0 * PI * freq * t);
            blocker.process(input);
        }

        // Measure output amplitude over one cycle
        let mut max_output = 0.0f32;
        for i in 0..48 {
            let t = (48000 + i) as f32 / sample_rate;
            let input = libm::sinf(2.0 * PI * freq * t);
            let output = blocker.process(input);
            max_output = max_output.max(output.abs());
        }

        // AC signal should pass through with near-unity gain
        assert!(
            max_output > 0.95,
            "1 kHz should pass through, max output was {}",
            max_output
        );
    }

    #[test]
    fn test_dc_blocker_reset() {
        let mut blocker = DcBlocker::new(48000.0);

        // Process some signal
        for _ in 0..1000 {
            blocker.process(1.0);
        }

        blocker.reset();

        assert_eq!(blocker.x_prev, 0.0);
        assert_eq!(blocker.y_prev, 0.0);
    }

    #[test]
    fn test_dc_blocker_with_coeff() {
        let blocker = DcBlocker::with_coeff(0.995);
        assert!((blocker.coeff() - 0.995).abs() < 1e-6);
    }

    #[test]
    fn test_dc_blocker_coeff_clamping() {
        let blocker = DcBlocker::with_coeff(0.5);
        assert!((blocker.coeff() - 0.9).abs() < 1e-6);

        let blocker = DcBlocker::with_coeff(1.0);
        assert!((blocker.coeff() - 0.9999).abs() < 1e-6);
    }

    #[test]
    fn test_dc_blocker_finite_output() {
        let mut blocker = DcBlocker::new(48000.0);

        for i in 0..10000 {
            let input = if i % 2 == 0 { 1.0 } else { -1.0 };
            let output = blocker.process(input);
            assert!(output.is_finite(), "Output must be finite");
        }
    }
}
