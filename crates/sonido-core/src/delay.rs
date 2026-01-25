//! Delay line implementations
//!
//! Circular buffer-based delay lines with various interpolation methods.
//! Essential for delay effects, reverbs, chorus, flangers, and more.

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std as alloc;

use alloc::vec;
use alloc::vec::Vec;

/// Interpolation method for fractional delay
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Interpolation {
    /// No interpolation (truncate to nearest sample)
    None,
    /// Linear interpolation between two samples
    #[default]
    Linear,
    /// Cubic interpolation (4-point, smoother)
    Cubic,
}

/// Interpolated delay line using a circular buffer (heap-allocated).
///
/// Supports fractional delay times through linear interpolation,
/// allowing smooth modulation of delay time without artifacts.
///
/// # Memory
///
/// The buffer is heap-allocated during construction but never reallocates.
/// No allocations occur during audio processing.
///
/// # Example
///
/// ```rust
/// use sonido_core::InterpolatedDelay;
///
/// // 50ms max delay at 44.1kHz
/// let max_delay_samples = (0.05 * 44100.0) as usize;
/// let mut delay = InterpolatedDelay::new(max_delay_samples);
///
/// // Read with 10.5 sample delay (fractional)
/// let output = delay.read(10.5);
/// delay.write(1.0);
/// ```
#[derive(Debug, Clone)]
pub struct InterpolatedDelay {
    /// Circular buffer storage
    buffer: Vec<f32>,
    /// Write position in buffer
    write_pos: usize,
}

impl InterpolatedDelay {
    /// Creates a new delay line with the given maximum delay in samples.
    ///
    /// # Arguments
    ///
    /// * `max_delay_samples` - Maximum delay capacity in samples
    ///
    /// # Panics
    ///
    /// Panics if `max_delay_samples` is 0.
    pub fn new(max_delay_samples: usize) -> Self {
        assert!(max_delay_samples > 0, "Delay size must be > 0");

        Self {
            buffer: vec![0.0; max_delay_samples],
            write_pos: 0,
        }
    }

    /// Creates a delay line from sample rate and max delay time in seconds.
    pub fn from_time(sample_rate: f32, max_seconds: f32) -> Self {
        let max_samples = (sample_rate * max_seconds) as usize + 1;
        Self::new(max_samples)
    }

    /// Reads a delayed sample with linear interpolation.
    ///
    /// # Arguments
    ///
    /// * `delay_samples` - Delay time in samples (can be fractional)
    ///
    /// Returns the interpolated sample from the delay line.
    #[inline]
    pub fn read(&self, delay_samples: f32) -> f32 {
        debug_assert!(delay_samples >= 0.0);

        let buffer_len = self.buffer.len();
        let delay_clamped = delay_samples.min((buffer_len - 1) as f32);

        // Calculate read position (going backwards from last written sample)
        let delay_int = delay_clamped as usize;
        let delay_frac = delay_clamped - delay_int as f32;

        // Calculate position of last written sample
        let last_written = if self.write_pos == 0 {
            buffer_len - 1
        } else {
            self.write_pos - 1
        };

        // Read position (going back from last written)
        let read_pos = if last_written >= delay_int {
            last_written - delay_int
        } else {
            buffer_len + last_written - delay_int
        };

        // Next sample position for interpolation (one sample further back)
        let next_pos = if read_pos == 0 {
            buffer_len - 1
        } else {
            read_pos - 1
        };

        // Linear interpolation: y = y0 + (y1 - y0) * frac
        let sample0 = self.buffer[read_pos];
        let sample1 = self.buffer[next_pos];

        sample0 + (sample1 - sample0) * delay_frac
    }

    /// Writes a sample to the delay line and advances the write position.
    ///
    /// # Arguments
    ///
    /// * `sample` - The sample to write
    #[inline]
    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % self.buffer.len();
    }

    /// Combined read and write operation.
    #[inline]
    pub fn read_write(&mut self, sample: f32, delay_samples: f32) -> f32 {
        let output = self.read(delay_samples);
        self.write(sample);
        output
    }

    /// Clears the delay line (sets all samples to 0).
    pub fn clear(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    /// Returns the maximum delay capacity in samples.
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }
}

/// Fixed-size delay line for embedded systems (no heap allocation).
///
/// Uses const generics for compile-time size specification.
///
/// # Example
///
/// ```rust
/// use sonido_core::FixedDelayLine;
///
/// // Create a delay line with max 4096 samples
/// let mut delay: FixedDelayLine<4096> = FixedDelayLine::new();
///
/// // Write and read with 100 sample delay
/// delay.write(1.0);
/// let output = delay.read(100.0);
/// ```
pub struct FixedDelayLine<const N: usize> {
    buffer: [f32; N],
    write_pos: usize,
    interpolation: Interpolation,
}

impl<const N: usize> FixedDelayLine<N> {
    /// Create a new fixed-size delay line
    pub fn new() -> Self {
        Self {
            buffer: [0.0; N],
            write_pos: 0,
            interpolation: Interpolation::Linear,
        }
    }

    /// Set interpolation method
    pub fn set_interpolation(&mut self, interp: Interpolation) {
        self.interpolation = interp;
    }

    /// Get the maximum delay in samples
    pub const fn max_delay(&self) -> usize {
        N - 1
    }

    /// Write a sample to the delay line
    #[inline]
    pub fn write(&mut self, sample: f32) {
        self.buffer[self.write_pos] = sample;
        self.write_pos = (self.write_pos + 1) % N;
    }

    /// Read from the delay line with fractional delay
    #[inline]
    pub fn read(&self, delay_samples: f32) -> f32 {
        let delay_int = (delay_samples as usize).min(N - 1);
        let frac = delay_samples - delay_int as f32;

        let read_pos = (self.write_pos + N - delay_int - 1) % N;

        match self.interpolation {
            Interpolation::None => self.buffer[read_pos],

            Interpolation::Linear => {
                let next_pos = (read_pos + N - 1) % N;
                let a = self.buffer[read_pos];
                let b = self.buffer[next_pos];
                a + (b - a) * frac
            }

            Interpolation::Cubic => {
                // 4-point cubic interpolation
                let p0 = (read_pos + 1) % N;
                let p1 = read_pos;
                let p2 = (read_pos + N - 1) % N;
                let p3 = (read_pos + N - 2) % N;

                let y0 = self.buffer[p0];
                let y1 = self.buffer[p1];
                let y2 = self.buffer[p2];
                let y3 = self.buffer[p3];

                let t = frac;
                let t2 = t * t;
                let t3 = t2 * t;

                let a0 = y3 - y2 - y0 + y1;
                let a1 = y0 - y1 - a0;
                let a2 = y2 - y0;
                let a3 = y1;

                a0 * t3 + a1 * t2 + a2 * t + a3
            }
        }
    }

    /// Combined read and write operation
    #[inline]
    pub fn read_write(&mut self, sample: f32, delay_samples: f32) -> f32 {
        let output = self.read(delay_samples);
        self.write(sample);
        output
    }

    /// Clear the delay line
    pub fn clear(&mut self) {
        self.buffer = [0.0; N];
        self.write_pos = 0;
    }
}

impl<const N: usize> Default for FixedDelayLine<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolated_delay_basic() {
        let mut delay = InterpolatedDelay::new(10);

        // Write some samples
        for i in 1..=5 {
            delay.write(i as f32);
        }

        // Read with 3 sample delay
        delay.write(6.0);
        let output = delay.read(3.0);
        assert_eq!(output, 3.0);
    }

    #[test]
    fn test_interpolated_delay_interpolation() {
        let mut delay = InterpolatedDelay::new(10);

        // Write 0, 1, 2, 3
        delay.write(0.0);
        delay.write(1.0);
        delay.write(2.0);
        delay.write(3.0);

        // Read with 1.5 sample delay - should interpolate
        let output = delay.read(1.5);
        assert!((output - 1.5).abs() < 0.01, "Expected ~1.5, got {}", output);
    }

    #[test]
    fn test_interpolated_delay_wrap() {
        let mut delay = InterpolatedDelay::new(4);

        // Fill buffer completely
        delay.write(1.0);
        delay.write(2.0);
        delay.write(3.0);
        delay.write(4.0);

        // Now write_pos wraps to 0
        delay.write(5.0);

        // Read with delay that crosses the wrap boundary
        let output = delay.read(3.0);
        assert_eq!(output, 2.0);
    }

    #[test]
    fn test_fixed_delay_line() {
        let mut delay: FixedDelayLine<128> = FixedDelayLine::new();

        delay.write(1.0);
        for _ in 0..50 {
            delay.write(0.0);
        }

        let output = delay.read(50.0);
        assert_eq!(output, 1.0);
    }

    #[test]
    fn test_fixed_delay_interpolation() {
        let mut delay: FixedDelayLine<128> = FixedDelayLine::new();

        delay.write(0.0);
        delay.write(1.0);

        let output = delay.read(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    #[should_panic]
    fn test_delay_zero_size_panics() {
        let _delay = InterpolatedDelay::new(0);
    }
}
