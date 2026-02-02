//! Voice management for polyphonic synthesis.
//!
//! Provides voice structures and allocation strategies for building
//! monophonic and polyphonic synthesizers.

use crate::oscillator::{Oscillator, OscillatorWaveform};
use crate::envelope::AdsrEnvelope;
use sonido_core::{StateVariableFilter, Effect};

/// Voice allocation modes for polyphonic synthesizers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VoiceAllocationMode {
    /// Cycle through voices in order (default)
    #[default]
    RoundRobin,
    /// Steal the oldest active note
    OldestNote,
    /// Steal the lowest pitch voice
    LowestNote,
    /// Steal the highest pitch voice
    HighestNote,
}

/// A single synthesizer voice.
///
/// Contains an oscillator, filter, and envelopes for amplitude and filter.
/// This is the basic building block for polyphonic synthesizers.
///
/// # Example
///
/// ```rust
/// use sonido_synth::Voice;
///
/// let mut voice = Voice::new(48000.0);
/// voice.note_on(60, 100); // Middle C, velocity 100
///
/// // Generate samples
/// for _ in 0..1000 {
///     let sample = voice.process();
/// }
///
/// voice.note_off();
/// ```
#[derive(Debug, Clone)]
pub struct Voice {
    /// Primary oscillator
    pub osc1: Oscillator,
    /// Secondary oscillator (for detuning, sync, etc.)
    pub osc2: Oscillator,
    /// Filter
    pub filter: StateVariableFilter,
    /// Amplitude envelope
    pub amp_env: AdsrEnvelope,
    /// Filter envelope
    pub filter_env: AdsrEnvelope,

    /// Current MIDI note number
    note: u8,
    /// Current velocity (0-127)
    velocity: u8,
    /// Voice age (for voice stealing)
    age: u64,
    /// Whether this voice is currently active
    active: bool,

    /// Sample rate
    sample_rate: f32,

    // Voice parameters
    /// Oscillator 2 detune in cents
    osc2_detune: f32,
    /// Oscillator mix (0 = osc1 only, 1 = osc2 only)
    osc_mix: f32,
    /// Filter envelope amount
    filter_env_amount: f32,
    /// Base filter cutoff frequency
    filter_cutoff: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Voice {
    /// Create a new voice at the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let mut voice = Self {
            osc1: Oscillator::new(sample_rate),
            osc2: Oscillator::new(sample_rate),
            filter: StateVariableFilter::new(sample_rate),
            amp_env: AdsrEnvelope::new(sample_rate),
            filter_env: AdsrEnvelope::new(sample_rate),
            note: 0,
            velocity: 0,
            age: 0,
            active: false,
            sample_rate,
            osc2_detune: 0.0,
            osc_mix: 0.0,
            filter_env_amount: 0.0,
            filter_cutoff: 1000.0,
        };

        // Set some reasonable defaults
        voice.osc1.set_waveform(OscillatorWaveform::Saw);
        voice.osc2.set_waveform(OscillatorWaveform::Saw);
        voice.filter.set_cutoff(1000.0);
        voice.filter.set_resonance(1.0);

        voice
    }

    /// Set sample rate for all components.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.osc1.set_sample_rate(sample_rate);
        self.osc2.set_sample_rate(sample_rate);
        self.filter.set_sample_rate(sample_rate);
        self.amp_env.set_sample_rate(sample_rate);
        self.filter_env.set_sample_rate(sample_rate);
    }

    /// Trigger note on.
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        self.note = note;
        self.velocity = velocity;
        self.active = true;

        let freq = midi_to_freq(note);
        self.osc1.set_frequency(freq);
        self.osc2.set_frequency(freq * cents_to_ratio(self.osc2_detune));

        self.osc1.reset();
        self.osc2.reset();
        self.amp_env.gate_on();
        self.filter_env.gate_on();
    }

    /// Trigger note off.
    pub fn note_off(&mut self) {
        self.amp_env.gate_off();
        self.filter_env.gate_off();
    }

    /// Force voice to stop immediately.
    pub fn kill(&mut self) {
        self.active = false;
        self.amp_env.reset();
        self.filter_env.reset();
    }

    /// Reset voice to initial state.
    pub fn reset(&mut self) {
        self.kill();
        self.note = 0;
        self.velocity = 0;
        self.age = 0;
        self.osc1.reset();
        self.osc2.reset();
        self.filter.reset();
    }

    /// Check if voice is currently producing sound.
    pub fn is_active(&self) -> bool {
        self.active && self.amp_env.is_active()
    }

    /// Get the current note number.
    pub fn note(&self) -> u8 {
        self.note
    }

    /// Get the current velocity.
    pub fn velocity(&self) -> u8 {
        self.velocity
    }

    /// Get voice age.
    pub fn age(&self) -> u64 {
        self.age
    }

    /// Set voice age.
    pub fn set_age(&mut self, age: u64) {
        self.age = age;
    }

    /// Set oscillator 2 detune in cents.
    pub fn set_osc2_detune(&mut self, cents: f32) {
        self.osc2_detune = cents;
        if self.active {
            let base_freq = midi_to_freq(self.note);
            self.osc2.set_frequency(base_freq * cents_to_ratio(cents));
        }
    }

    /// Set oscillator mix (0 = osc1 only, 1 = osc2 only).
    pub fn set_osc_mix(&mut self, mix: f32) {
        self.osc_mix = mix.clamp(0.0, 1.0);
    }

    /// Set filter envelope amount (in Hz).
    pub fn set_filter_env_amount(&mut self, amount: f32) {
        self.filter_env_amount = amount;
    }

    /// Set base filter cutoff frequency.
    pub fn set_filter_cutoff(&mut self, freq: f32) {
        self.filter_cutoff = freq;
        self.filter.set_cutoff(freq);
    }

    /// Process one sample.
    #[inline]
    pub fn process(&mut self) -> f32 {
        if !self.is_active() {
            // Check if envelope just finished
            if self.active && !self.amp_env.is_active() {
                self.active = false;
            }
            return 0.0;
        }

        // Generate oscillator output
        let osc1_out = self.osc1.advance();
        let osc2_out = self.osc2.advance();
        let osc_out = osc1_out * (1.0 - self.osc_mix) + osc2_out * self.osc_mix;

        // Apply filter with envelope modulation
        let filter_env = self.filter_env.advance();
        let modulated_cutoff = self.filter_cutoff + filter_env * self.filter_env_amount;
        self.filter.set_cutoff(modulated_cutoff.clamp(20.0, 20000.0));
        let filtered = self.filter.process(osc_out);

        // Apply amplitude envelope with velocity scaling
        let amp_env = self.amp_env.advance();
        let velocity_scale = self.velocity as f32 / 127.0;

        filtered * amp_env * velocity_scale
    }
}

/// Voice manager for polyphonic synthesis.
///
/// Manages a pool of voices and handles note allocation/stealing.
///
/// # Example
///
/// ```rust
/// use sonido_synth::{VoiceManager, VoiceAllocationMode};
///
/// let mut manager: VoiceManager<8> = VoiceManager::new(48000.0);
/// manager.set_allocation_mode(VoiceAllocationMode::OldestNote);
///
/// // Play notes
/// manager.note_on(60, 100);
/// manager.note_on(64, 100);
/// manager.note_on(67, 100);
///
/// // Process audio
/// for _ in 0..1000 {
///     let sample = manager.process();
/// }
/// ```
#[derive(Debug)]
pub struct VoiceManager<const N: usize> {
    voices: [Voice; N],
    allocation_mode: VoiceAllocationMode,
    sample_rate: f32,
    /// Global voice age counter
    age_counter: u64,
    /// Round-robin index
    round_robin_idx: usize,
}

impl<const N: usize> VoiceManager<N> {
    /// Create a new voice manager with the specified number of voices.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            voices: core::array::from_fn(|_| Voice::new(sample_rate)),
            allocation_mode: VoiceAllocationMode::RoundRobin,
            sample_rate,
            age_counter: 0,
            round_robin_idx: 0,
        }
    }

    /// Set sample rate for all voices.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        for voice in &mut self.voices {
            voice.set_sample_rate(sample_rate);
        }
    }

    /// Set voice allocation mode.
    pub fn set_allocation_mode(&mut self, mode: VoiceAllocationMode) {
        self.allocation_mode = mode;
    }

    /// Get current allocation mode.
    pub fn allocation_mode(&self) -> VoiceAllocationMode {
        self.allocation_mode
    }

    /// Get number of voices.
    pub fn voice_count(&self) -> usize {
        N
    }

    /// Get number of active voices.
    pub fn active_voice_count(&self) -> usize {
        self.voices.iter().filter(|v| v.is_active()).count()
    }

    /// Get mutable access to all voices (for setting parameters).
    pub fn voices_mut(&mut self) -> &mut [Voice; N] {
        &mut self.voices
    }

    /// Get read access to all voices.
    pub fn voices(&self) -> &[Voice; N] {
        &self.voices
    }

    /// Trigger a note on.
    pub fn note_on(&mut self, note: u8, velocity: u8) {
        let voice_idx = self.allocate_voice(note);
        self.age_counter += 1;
        self.voices[voice_idx].set_age(self.age_counter);
        self.voices[voice_idx].note_on(note, velocity);
    }

    /// Trigger a note off.
    pub fn note_off(&mut self, note: u8) {
        for voice in &mut self.voices {
            if voice.is_active() && voice.note() == note {
                voice.note_off();
                return;
            }
        }
    }

    /// Stop all notes immediately.
    pub fn all_notes_off(&mut self) {
        for voice in &mut self.voices {
            voice.kill();
        }
    }

    /// Reset all voices.
    pub fn reset(&mut self) {
        for voice in &mut self.voices {
            voice.reset();
        }
        self.age_counter = 0;
        self.round_robin_idx = 0;
    }

    /// Process one sample from all voices.
    #[inline]
    pub fn process(&mut self) -> f32 {
        let mut output = 0.0;
        for voice in &mut self.voices {
            output += voice.process();
        }
        output
    }

    /// Process stereo output from all voices.
    #[inline]
    pub fn process_stereo(&mut self) -> (f32, f32) {
        let mono = self.process();
        (mono, mono)
    }

    fn allocate_voice(&mut self, _note: u8) -> usize {
        // First, try to find a free voice
        for (i, voice) in self.voices.iter().enumerate() {
            if !voice.is_active() {
                return i;
            }
        }

        // All voices are active, need to steal one
        match self.allocation_mode {
            VoiceAllocationMode::RoundRobin => {
                let idx = self.round_robin_idx;
                self.round_robin_idx = (self.round_robin_idx + 1) % N;
                idx
            }
            VoiceAllocationMode::OldestNote => {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.age())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            VoiceAllocationMode::LowestNote => {
                self.voices
                    .iter()
                    .enumerate()
                    .min_by_key(|(_, v)| v.note())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
            VoiceAllocationMode::HighestNote => {
                self.voices
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, v)| v.note())
                    .map(|(i, _)| i)
                    .unwrap_or(0)
            }
        }
    }
}

/// Convert MIDI note number to frequency in Hz.
///
/// Uses standard tuning: A4 (note 69) = 440 Hz.
#[inline]
pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * libm::powf(2.0, (note as f32 - 69.0) / 12.0)
}

/// Convert frequency in Hz to MIDI note number.
#[inline]
pub fn freq_to_midi(freq: f32) -> f32 {
    69.0 + 12.0 * libm::log2f(freq / 440.0)
}

/// Convert cents to frequency ratio.
///
/// 100 cents = 1 semitone.
#[inline]
pub fn cents_to_ratio(cents: f32) -> f32 {
    libm::powf(2.0, cents / 1200.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_midi_to_freq_a4() {
        let freq = midi_to_freq(69);
        assert!(
            (freq - 440.0).abs() < 0.01,
            "A4 should be 440 Hz, got {}",
            freq
        );
    }

    #[test]
    fn test_midi_to_freq_middle_c() {
        let freq = midi_to_freq(60);
        assert!(
            (freq - 261.63).abs() < 0.1,
            "C4 should be ~261.63 Hz, got {}",
            freq
        );
    }

    #[test]
    fn test_cents_to_ratio() {
        // 1200 cents = 1 octave = ratio of 2
        let ratio = cents_to_ratio(1200.0);
        assert!(
            (ratio - 2.0).abs() < 0.001,
            "1200 cents should be ratio 2, got {}",
            ratio
        );

        // 0 cents = ratio of 1
        let ratio = cents_to_ratio(0.0);
        assert!(
            (ratio - 1.0).abs() < 0.001,
            "0 cents should be ratio 1, got {}",
            ratio
        );
    }

    #[test]
    fn test_voice_note_on_off() {
        let mut voice = Voice::new(48000.0);

        assert!(!voice.is_active());

        voice.note_on(60, 100);
        assert!(voice.is_active());
        assert_eq!(voice.note(), 60);
        assert_eq!(voice.velocity(), 100);

        voice.note_off();
        // Voice should still be active during release
        // but will become inactive after envelope completes

        voice.kill();
        assert!(!voice.is_active());
    }

    #[test]
    fn test_voice_process() {
        let mut voice = Voice::new(48000.0);
        voice.note_on(69, 100); // A4

        // Should produce non-zero output
        let mut sum = 0.0;
        for _ in 0..1000 {
            sum += voice.process().abs();
        }

        assert!(sum > 0.0, "Voice should produce output");
    }

    #[test]
    fn test_voice_manager_allocation() {
        let mut manager: VoiceManager<4> = VoiceManager::new(48000.0);

        // Play 4 notes
        manager.note_on(60, 100);
        manager.note_on(64, 100);
        manager.note_on(67, 100);
        manager.note_on(72, 100);

        assert_eq!(manager.active_voice_count(), 4);

        // 5th note should steal a voice
        manager.note_on(76, 100);
        assert_eq!(manager.active_voice_count(), 4);
    }

    #[test]
    fn test_voice_manager_oldest_note_stealing() {
        let mut manager: VoiceManager<2> = VoiceManager::new(48000.0);
        manager.set_allocation_mode(VoiceAllocationMode::OldestNote);

        manager.note_on(60, 100);
        manager.note_on(64, 100);

        // Third note should steal the oldest (60)
        manager.note_on(67, 100);

        // Voice playing 64 should still be active
        let has_64 = manager.voices().iter().any(|v| v.is_active() && v.note() == 64);
        assert!(has_64, "Note 64 should still be playing");
    }

    #[test]
    fn test_voice_manager_note_off() {
        let mut manager: VoiceManager<4> = VoiceManager::new(48000.0);

        manager.note_on(60, 100);
        manager.note_on(64, 100);

        manager.note_off(60);

        // Voice should still be in release phase (active but releasing)
        // For this test, we'll check that only one voice is playing note 64
        let playing_64 = manager
            .voices()
            .iter()
            .filter(|v| v.is_active() && v.note() == 64)
            .count();
        assert_eq!(playing_64, 1);
    }

    #[test]
    fn test_voice_manager_all_notes_off() {
        let mut manager: VoiceManager<4> = VoiceManager::new(48000.0);

        manager.note_on(60, 100);
        manager.note_on(64, 100);
        manager.note_on(67, 100);

        manager.all_notes_off();
        assert_eq!(manager.active_voice_count(), 0);
    }

    #[test]
    fn test_voice_manager_process() {
        let mut manager: VoiceManager<4> = VoiceManager::new(48000.0);

        manager.note_on(69, 100); // A4

        let mut sum = 0.0;
        for _ in 0..1000 {
            sum += manager.process().abs();
        }

        assert!(sum > 0.0, "Manager should produce output");
    }
}
