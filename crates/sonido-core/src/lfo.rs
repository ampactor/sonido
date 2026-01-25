//! Low Frequency Oscillator for modulation effects.
//!
//! Provides smooth, periodic modulation signals used in chorus, flanger,
//! tremolo, vibrato, and other time-based effects.

use core::f32::consts::PI;
use libm::{sinf, floorf};

/// LFO waveform type
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LfoWaveform {
    #[default]
    Sine,
    Triangle,
    Saw,
    Square,
    SampleAndHold,
}

/// Low Frequency Oscillator for generating modulation signals.
///
/// Generates periodic waveforms at sub-audio frequencies (typically 0.1-20 Hz).
/// Uses phase accumulation for efficient, alias-free oscillation.
///
/// # Waveforms
///
/// - **Sine**: Smooth, natural modulation
/// - **Triangle**: Linear ramps, harder corners than sine
/// - **Saw**: Rising ramp, abrupt reset
/// - **Square**: Binary on/off modulation
/// - **SampleAndHold**: Random stepped values
///
/// # Example
///
/// ```rust
/// use sonido_core::{Lfo, LfoWaveform};
///
/// let mut lfo = Lfo::new(44100.0, 2.0); // 2 Hz
/// lfo.set_waveform(LfoWaveform::Triangle);
///
/// // Generate modulation values in [-1.0, 1.0]
/// let value = lfo.next();
/// ```
#[derive(Debug, Clone)]
pub struct Lfo {
    /// Current phase position [0.0, 1.0)
    phase: f32,
    /// Phase increment per sample
    phase_inc: f32,
    /// Sample rate in Hz
    sample_rate: f32,
    /// Waveform type
    waveform: LfoWaveform,
    /// For Sample & Hold: current held value
    sh_value: f32,
    /// Previous phase (for detecting wrap)
    prev_phase: f32,
}

impl Default for Lfo {
    fn default() -> Self {
        Self::new(48000.0, 1.0)
    }
}

impl Lfo {
    /// Create new LFO with given sample rate and frequency
    pub fn new(sample_rate: f32, freq_hz: f32) -> Self {
        Self {
            phase: 0.0,
            phase_inc: freq_hz / sample_rate,
            sample_rate,
            waveform: LfoWaveform::Sine,
            sh_value: 0.0,
            prev_phase: 0.0,
        }
    }

    /// Set frequency in Hz
    pub fn set_frequency(&mut self, freq_hz: f32) {
        self.phase_inc = freq_hz / self.sample_rate;
    }

    /// Get current frequency in Hz
    pub fn frequency(&self) -> f32 {
        self.phase_inc * self.sample_rate
    }

    /// Set waveform
    pub fn set_waveform(&mut self, waveform: LfoWaveform) {
        self.waveform = waveform;
    }

    /// Get current waveform
    pub fn waveform(&self) -> LfoWaveform {
        self.waveform
    }

    /// Reset phase to 0
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.prev_phase = 0.0;
    }

    /// Sync phase to a specific value (0.0 - 1.0)
    ///
    /// Useful for phase-offset LFOs in multi-voice effects.
    /// 0.0 = 0°, 0.25 = 90°, 0.5 = 180°, 0.75 = 270°
    pub fn set_phase(&mut self, phase: f32) {
        self.phase = phase.clamp(0.0, 1.0);
        self.prev_phase = self.phase;
    }

    /// Get current phase (0.0 - 1.0)
    pub fn phase(&self) -> f32 {
        self.phase
    }

    /// Get next LFO value (-1.0 to 1.0)
    #[inline]
    pub fn next(&mut self) -> f32 {
        let output = match self.waveform {
            LfoWaveform::Sine => sinf(self.phase * 2.0 * PI),

            LfoWaveform::Triangle => {
                if self.phase < 0.5 {
                    4.0 * self.phase - 1.0
                } else {
                    3.0 - 4.0 * self.phase
                }
            }

            LfoWaveform::Saw => 2.0 * self.phase - 1.0,

            LfoWaveform::Square => {
                if self.phase < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }

            LfoWaveform::SampleAndHold => {
                // New random value when phase wraps
                if self.phase < self.prev_phase {
                    // Simple pseudo-random using phase
                    let x = sinf(self.phase * 12345.6789) * 43758.5453;
                    self.sh_value = (x - floorf(x)) * 2.0 - 1.0;
                }
                self.sh_value
            }
        };

        self.prev_phase = self.phase;
        self.phase += self.phase_inc;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }

        output
    }

    /// Get next value scaled to range (0.0 to 1.0 for unipolar)
    pub fn next_unipolar(&mut self) -> f32 {
        (self.next() + 1.0) * 0.5
    }

    /// Set sample rate
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let freq = self.phase_inc * self.sample_rate;
        self.sample_rate = sample_rate;
        self.set_frequency(freq);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lfo_phase_accumulation() {
        let mut lfo = Lfo::new(44100.0, 1.0); // 1 Hz = one cycle per second

        // After 44100 samples (1 second), should complete one cycle
        for _ in 0..44100 {
            lfo.next();
        }

        // Phase should be very close to 0 or 1 (wrapped around)
        let phase_error = lfo.phase.min((lfo.phase - 1.0).abs());
        assert!(phase_error < 0.01);
    }

    #[test]
    fn test_lfo_output_range() {
        let mut lfo = Lfo::new(44100.0, 5.0);

        // Check all waveforms stay in [-1.0, 1.0]
        for waveform in [LfoWaveform::Sine, LfoWaveform::Triangle, LfoWaveform::Saw, LfoWaveform::Square] {
            lfo.set_waveform(waveform);
            lfo.reset();

            for _ in 0..1000 {
                let value = lfo.next();
                assert!(
                    value >= -1.0 && value <= 1.0,
                    "Waveform {:?} out of range: {}",
                    waveform,
                    value
                );
            }
        }
    }

    #[test]
    fn test_lfo_phase_offset() {
        let mut lfo1 = Lfo::new(44100.0, 2.0);
        let mut lfo2 = Lfo::new(44100.0, 2.0);

        lfo2.set_phase(0.5); // 180° offset

        let val1 = lfo1.next();
        let val2 = lfo2.next();

        // Should be approximately opposite for sine
        assert!(
            (val1 + val2).abs() < 0.01,
            "Expected opposite values, got {} and {}",
            val1,
            val2
        );
    }

    #[test]
    fn test_lfo_sample_rate_change() {
        let mut lfo = Lfo::new(44100.0, 440.0);

        let phase_inc_44k = lfo.phase_inc;

        lfo.set_sample_rate(48000.0);
        let phase_inc_48k = lfo.phase_inc;

        // Phase increment should scale inversely with sample rate
        let ratio = 48000.0 / 44100.0;
        assert!((phase_inc_44k / phase_inc_48k - ratio).abs() < 0.0001);
    }

    #[test]
    fn test_lfo_unipolar() {
        let mut lfo = Lfo::new(44100.0, 5.0);

        for _ in 0..1000 {
            let value = lfo.next_unipolar();
            assert!(
                value >= 0.0 && value <= 1.0,
                "Unipolar value out of range: {}",
                value
            );
        }
    }
}
