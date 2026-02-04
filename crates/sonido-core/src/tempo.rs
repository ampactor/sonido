//! Tempo and clock management for tempo-synced effects.
//!
//! Provides musical timing primitives for LFO sync, delay times, and
//! other tempo-synchronized processing.

use libm::floorf;

/// Musical note divisions for tempo sync.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NoteDivision {
    /// Whole note (4 beats)
    Whole,
    /// Half note (2 beats)
    Half,
    /// Quarter note (1 beat)
    #[default]
    Quarter,
    /// Eighth note (1/2 beat)
    Eighth,
    /// Sixteenth note (1/4 beat)
    Sixteenth,
    /// Thirty-second note (1/8 beat)
    ThirtySecond,
    /// Dotted half note (3 beats)
    DottedHalf,
    /// Dotted quarter note (1.5 beats)
    DottedQuarter,
    /// Dotted eighth note (3/4 beat)
    DottedEighth,
    /// Triplet quarter note (2/3 beat)
    TripletQuarter,
    /// Triplet eighth note (1/3 beat)
    TripletEighth,
    /// Triplet sixteenth note (1/6 beat)
    TripletSixteenth,
}

impl NoteDivision {
    /// Convert note division to frequency in Hz at given BPM.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::NoteDivision;
    ///
    /// // At 120 BPM, quarter note = 2 Hz
    /// let freq = NoteDivision::Quarter.to_hz(120.0);
    /// assert!((freq - 2.0).abs() < 0.001);
    ///
    /// // At 120 BPM, eighth note = 4 Hz
    /// let freq = NoteDivision::Eighth.to_hz(120.0);
    /// assert!((freq - 4.0).abs() < 0.001);
    /// ```
    pub fn to_hz(&self, bpm: f32) -> f32 {
        let beats_per_second = bpm / 60.0;
        let beats = self.beats();
        beats_per_second / beats
    }

    /// Convert note division to time in milliseconds at given BPM.
    ///
    /// # Example
    ///
    /// ```rust
    /// use sonido_core::NoteDivision;
    ///
    /// // At 120 BPM, quarter note = 500ms
    /// let ms = NoteDivision::Quarter.to_ms(120.0);
    /// assert!((ms - 500.0).abs() < 0.1);
    /// ```
    pub fn to_ms(&self, bpm: f32) -> f32 {
        let beats = self.beats();
        let ms_per_beat = 60000.0 / bpm;
        beats * ms_per_beat
    }

    /// Convert note division to samples at given BPM and sample rate.
    pub fn to_samples(&self, bpm: f32, sample_rate: f32) -> f32 {
        let seconds = self.to_ms(bpm) / 1000.0;
        seconds * sample_rate
    }

    /// Get the number of beats this division represents.
    pub fn beats(&self) -> f32 {
        match self {
            NoteDivision::Whole => 4.0,
            NoteDivision::Half => 2.0,
            NoteDivision::Quarter => 1.0,
            NoteDivision::Eighth => 0.5,
            NoteDivision::Sixteenth => 0.25,
            NoteDivision::ThirtySecond => 0.125,
            NoteDivision::DottedHalf => 3.0,
            NoteDivision::DottedQuarter => 1.5,
            NoteDivision::DottedEighth => 0.75,
            NoteDivision::TripletQuarter => 2.0 / 3.0,
            NoteDivision::TripletEighth => 1.0 / 3.0,
            NoteDivision::TripletSixteenth => 1.0 / 6.0,
        }
    }
}

/// Transport state for tempo-synced processing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TransportState {
    #[default]
    Stopped,
    Playing,
}

/// Manages tempo and transport for synchronized effects.
///
/// Tracks musical position and provides timing utilities for
/// tempo-synced LFOs, delays, and other time-based effects.
///
/// # Example
///
/// ```rust
/// use sonido_core::{TempoManager, NoteDivision, TransportState};
///
/// let mut tempo = TempoManager::new(48000.0, 120.0);
/// tempo.play();
///
/// // Get LFO frequency for eighth notes
/// let lfo_freq = tempo.division_to_hz(NoteDivision::Eighth);
/// assert!((lfo_freq - 4.0).abs() < 0.001);
///
/// // Advance transport
/// for _ in 0..48000 {
///     tempo.advance();
/// }
///
/// // Should have advanced 1 second = 2 beats at 120 BPM
/// assert!((tempo.beat_position() - 2.0).abs() < 0.01);
/// ```
#[derive(Debug, Clone)]
pub struct TempoManager {
    bpm: f32,
    sample_rate: f32,
    samples_per_beat: f32,
    /// Current position in samples
    position: u64,
    transport: TransportState,
}

impl TempoManager {
    /// Create a new tempo manager.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz
    /// * `bpm` - Tempo in beats per minute
    pub fn new(sample_rate: f32, bpm: f32) -> Self {
        let samples_per_beat = sample_rate * 60.0 / bpm;
        Self {
            bpm,
            sample_rate,
            samples_per_beat,
            position: 0,
            transport: TransportState::Stopped,
        }
    }

    /// Set the tempo in BPM.
    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.max(1.0);
        self.samples_per_beat = self.sample_rate * 60.0 / self.bpm;
    }

    /// Get the current tempo in BPM.
    pub fn bpm(&self) -> f32 {
        self.bpm
    }

    /// Set the sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.samples_per_beat = self.sample_rate * 60.0 / self.bpm;
    }

    /// Get the sample rate.
    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Start transport.
    pub fn play(&mut self) {
        self.transport = TransportState::Playing;
    }

    /// Stop transport.
    pub fn stop(&mut self) {
        self.transport = TransportState::Stopped;
    }

    /// Get current transport state.
    pub fn transport(&self) -> TransportState {
        self.transport
    }

    /// Check if transport is playing.
    pub fn is_playing(&self) -> bool {
        self.transport == TransportState::Playing
    }

    /// Reset position to zero.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Advance transport by one sample.
    ///
    /// Only advances if transport is playing.
    pub fn advance(&mut self) {
        if self.transport == TransportState::Playing {
            self.position = self.position.wrapping_add(1);
        }
    }

    /// Get current position in beats.
    pub fn beat_position(&self) -> f32 {
        self.position as f32 / self.samples_per_beat
    }

    /// Get current position in bars (assuming 4/4 time).
    pub fn bar_position(&self) -> f32 {
        self.beat_position() / 4.0
    }

    /// Get fractional position within current beat (0.0 to 1.0).
    pub fn beat_phase(&self) -> f32 {
        let beat_pos = self.beat_position();
        beat_pos - floorf(beat_pos)
    }

    /// Get fractional position within current bar (0.0 to 1.0, assuming 4/4).
    pub fn bar_phase(&self) -> f32 {
        let bar_pos = self.bar_position();
        bar_pos - floorf(bar_pos)
    }

    /// Get LFO frequency for a note division.
    pub fn division_to_hz(&self, division: NoteDivision) -> f32 {
        division.to_hz(self.bpm)
    }

    /// Get delay time in milliseconds for a note division.
    pub fn division_to_ms(&self, division: NoteDivision) -> f32 {
        division.to_ms(self.bpm)
    }

    /// Get delay time in samples for a note division.
    pub fn division_to_samples(&self, division: NoteDivision) -> f32 {
        division.to_samples(self.bpm, self.sample_rate)
    }

    /// Check if we're on a beat boundary (within tolerance).
    pub fn is_on_beat(&self, tolerance_samples: u64) -> bool {
        let samples_into_beat = self.position % (self.samples_per_beat as u64);
        samples_into_beat < tolerance_samples
            || samples_into_beat > (self.samples_per_beat as u64 - tolerance_samples)
    }
}

impl Default for TempoManager {
    fn default() -> Self {
        Self::new(48000.0, 120.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_division_to_hz() {
        // At 120 BPM:
        // - Quarter note = 2 Hz (one per beat, 2 beats per second)
        // - Eighth note = 4 Hz
        // - Half note = 1 Hz
        assert!((NoteDivision::Quarter.to_hz(120.0) - 2.0).abs() < 0.001);
        assert!((NoteDivision::Eighth.to_hz(120.0) - 4.0).abs() < 0.001);
        assert!((NoteDivision::Half.to_hz(120.0) - 1.0).abs() < 0.001);
        assert!((NoteDivision::Sixteenth.to_hz(120.0) - 8.0).abs() < 0.001);
    }

    #[test]
    fn test_note_division_to_ms() {
        // At 120 BPM:
        // - Quarter note = 500ms (60000 / 120)
        // - Eighth note = 250ms
        // - Half note = 1000ms
        assert!((NoteDivision::Quarter.to_ms(120.0) - 500.0).abs() < 0.1);
        assert!((NoteDivision::Eighth.to_ms(120.0) - 250.0).abs() < 0.1);
        assert!((NoteDivision::Half.to_ms(120.0) - 1000.0).abs() < 0.1);
    }

    #[test]
    fn test_dotted_divisions() {
        // Dotted quarter = 1.5 beats = 750ms at 120 BPM
        assert!((NoteDivision::DottedQuarter.to_ms(120.0) - 750.0).abs() < 0.1);
        // Dotted eighth = 0.75 beats = 375ms at 120 BPM
        assert!((NoteDivision::DottedEighth.to_ms(120.0) - 375.0).abs() < 0.1);
    }

    #[test]
    fn test_triplet_divisions() {
        // Triplet eighth = 1/3 beat = 166.67ms at 120 BPM
        let triplet_ms = NoteDivision::TripletEighth.to_ms(120.0);
        assert!((triplet_ms - 166.667).abs() < 0.1);
    }

    #[test]
    fn test_tempo_manager_position() {
        let mut tempo = TempoManager::new(48000.0, 120.0);
        tempo.play();

        // At 120 BPM, 48000 samples/sec: samples_per_beat = 48000 * 60 / 120 = 24000
        assert!((tempo.samples_per_beat - 24000.0).abs() < 0.1);

        // Advance one beat worth of samples
        for _ in 0..24000 {
            tempo.advance();
        }

        assert!((tempo.beat_position() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_tempo_manager_stopped() {
        let mut tempo = TempoManager::new(48000.0, 120.0);
        // Transport is stopped by default

        for _ in 0..1000 {
            tempo.advance();
        }

        // Position should not have changed
        assert_eq!(tempo.position, 0);
    }

    #[test]
    fn test_beat_phase() {
        let mut tempo = TempoManager::new(48000.0, 120.0);
        tempo.play();

        // Advance half a beat (12000 samples at 120 BPM)
        for _ in 0..12000 {
            tempo.advance();
        }

        assert!((tempo.beat_phase() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_bpm_change() {
        let mut tempo = TempoManager::new(48000.0, 120.0);
        tempo.set_bpm(60.0);

        // At 60 BPM, quarter note should be 1 Hz
        assert!((tempo.division_to_hz(NoteDivision::Quarter) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_division_to_samples() {
        let tempo = TempoManager::new(48000.0, 120.0);

        // At 120 BPM, 48kHz: quarter note = 24000 samples
        let samples = tempo.division_to_samples(NoteDivision::Quarter);
        assert!((samples - 24000.0).abs() < 0.1);
    }
}
