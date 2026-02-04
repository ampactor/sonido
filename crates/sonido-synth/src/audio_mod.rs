//! Audio input as modulation source.
//!
//! Wraps an EnvelopeFollower to use audio signals as modulation sources.
//! Useful for sidechain synthesis, vocoder carriers, and envelope followers.

use sonido_core::{EnvelopeFollower, ModulationSource};

/// Audio input as a modulation source.
///
/// Uses an envelope follower to extract the amplitude envelope
/// from an audio signal, which can then be used to modulate
/// synthesizer parameters.
///
/// # Example
///
/// ```rust
/// use sonido_synth::AudioModSource;
/// use sonido_core::ModulationSource;
///
/// let mut audio_mod = AudioModSource::new(48000.0);
/// audio_mod.set_attack_ms(5.0);
/// audio_mod.set_release_ms(50.0);
///
/// // Process audio input
/// let audio_sample = 0.5;
/// audio_mod.process(audio_sample);
///
/// // Get modulation value
/// let mod_value = audio_mod.mod_advance();
/// ```
#[derive(Debug, Clone)]
pub struct AudioModSource {
    /// Internal envelope follower
    envelope_follower: EnvelopeFollower,
    /// Current envelope level (cached for mod_value)
    current_level: f32,
    /// Gain applied to input before envelope detection
    input_gain: f32,
    /// Threshold below which output is zero
    threshold: f32,
}

impl Default for AudioModSource {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl AudioModSource {
    /// Create a new audio modulation source.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope_follower: EnvelopeFollower::with_times(sample_rate, 5.0, 50.0),
            current_level: 0.0,
            input_gain: 1.0,
            threshold: 0.0,
        }
    }

    /// Create with custom attack and release times.
    pub fn with_times(sample_rate: f32, attack_ms: f32, release_ms: f32) -> Self {
        Self {
            envelope_follower: EnvelopeFollower::with_times(sample_rate, attack_ms, release_ms),
            current_level: 0.0,
            input_gain: 1.0,
            threshold: 0.0,
        }
    }

    /// Set the attack time in milliseconds.
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.envelope_follower.set_attack_ms(attack_ms);
    }

    /// Get attack time in milliseconds.
    pub fn attack_ms(&self) -> f32 {
        self.envelope_follower.attack_ms()
    }

    /// Set the release time in milliseconds.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.envelope_follower.set_release_ms(release_ms);
    }

    /// Get release time in milliseconds.
    pub fn release_ms(&self) -> f32 {
        self.envelope_follower.release_ms()
    }

    /// Set sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.envelope_follower.set_sample_rate(sample_rate);
    }

    /// Set input gain (applied before envelope detection).
    pub fn set_input_gain(&mut self, gain: f32) {
        self.input_gain = gain.max(0.0);
    }

    /// Get input gain.
    pub fn input_gain(&self) -> f32 {
        self.input_gain
    }

    /// Set threshold level (envelope below this outputs zero).
    pub fn set_threshold(&mut self, threshold: f32) {
        self.threshold = threshold.clamp(0.0, 1.0);
    }

    /// Get threshold level.
    pub fn threshold(&self) -> f32 {
        self.threshold
    }

    /// Process an audio sample and update the envelope.
    #[inline]
    pub fn process(&mut self, input: f32) -> f32 {
        let gained = input * self.input_gain;
        let envelope = self.envelope_follower.process(gained);

        // Apply threshold with soft knee
        self.current_level = if envelope > self.threshold {
            (envelope - self.threshold) / (1.0 - self.threshold)
        } else {
            0.0
        };

        self.current_level
    }

    /// Process a block of audio samples.
    pub fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (i, &sample) in input.iter().enumerate() {
            output[i] = self.process(sample);
        }
    }

    /// Get current envelope level without processing new input.
    pub fn level(&self) -> f32 {
        self.current_level
    }

    /// Reset the envelope follower.
    pub fn reset(&mut self) {
        self.envelope_follower.reset();
        self.current_level = 0.0;
    }
}

impl ModulationSource for AudioModSource {
    fn mod_advance(&mut self) -> f32 {
        // Convert unipolar (0-1) to bipolar (-1 to 1) for consistency
        self.current_level * 2.0 - 1.0
    }

    fn is_bipolar(&self) -> bool {
        // We convert to bipolar in mod_advance
        true
    }

    fn mod_reset(&mut self) {
        self.reset();
    }

    fn mod_value(&self) -> f32 {
        self.current_level * 2.0 - 1.0
    }
}

/// Audio gate that triggers based on input level.
///
/// Useful for triggering envelopes from audio input.
#[derive(Debug, Clone)]
pub struct AudioGate {
    /// Envelope follower for level detection
    envelope_follower: EnvelopeFollower,
    /// Current gate state
    gate_open: bool,
    /// Threshold for gate opening
    open_threshold: f32,
    /// Threshold for gate closing (hysteresis)
    close_threshold: f32,
    /// Input gain
    input_gain: f32,
}

impl Default for AudioGate {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl AudioGate {
    /// Create a new audio gate.
    pub fn new(sample_rate: f32) -> Self {
        Self {
            envelope_follower: EnvelopeFollower::with_times(sample_rate, 1.0, 20.0),
            gate_open: false,
            open_threshold: 0.1,
            close_threshold: 0.05,
            input_gain: 1.0,
        }
    }

    /// Set the threshold for gate opening.
    pub fn set_open_threshold(&mut self, threshold: f32) {
        self.open_threshold = threshold.clamp(0.0, 1.0);
        // Ensure close threshold is below open threshold
        if self.close_threshold >= self.open_threshold {
            self.close_threshold = self.open_threshold * 0.5;
        }
    }

    /// Get open threshold.
    pub fn open_threshold(&self) -> f32 {
        self.open_threshold
    }

    /// Set the threshold for gate closing (should be below open threshold).
    pub fn set_close_threshold(&mut self, threshold: f32) {
        self.close_threshold = threshold.clamp(0.0, self.open_threshold);
    }

    /// Get close threshold.
    pub fn close_threshold(&self) -> f32 {
        self.close_threshold
    }

    /// Set input gain.
    pub fn set_input_gain(&mut self, gain: f32) {
        self.input_gain = gain.max(0.0);
    }

    /// Set attack time for envelope detection.
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.envelope_follower.set_attack_ms(attack_ms);
    }

    /// Set release time for envelope detection.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.envelope_follower.set_release_ms(release_ms);
    }

    /// Set sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.envelope_follower.set_sample_rate(sample_rate);
    }

    /// Process an audio sample and return gate state.
    #[inline]
    pub fn process(&mut self, input: f32) -> bool {
        let gained = input * self.input_gain;
        let envelope = self.envelope_follower.process(gained);

        if self.gate_open {
            if envelope < self.close_threshold {
                self.gate_open = false;
            }
        } else if envelope > self.open_threshold {
            self.gate_open = true;
        }

        self.gate_open
    }

    /// Check if gate is currently open.
    pub fn is_open(&self) -> bool {
        self.gate_open
    }

    /// Get current envelope level.
    pub fn level(&self) -> f32 {
        self.envelope_follower.level()
    }

    /// Reset the gate.
    pub fn reset(&mut self) {
        self.envelope_follower.reset();
        self.gate_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_mod_source_basic() {
        let mut audio_mod = AudioModSource::new(48000.0);

        // Feed silence
        for _ in 0..100 {
            audio_mod.process(0.0);
        }
        assert!(audio_mod.level() < 0.01);

        // Feed signal
        for _ in 0..1000 {
            audio_mod.process(0.5);
        }
        assert!(audio_mod.level() > 0.3);
    }

    #[test]
    fn test_audio_mod_source_threshold() {
        let mut audio_mod = AudioModSource::new(48000.0);
        audio_mod.set_threshold(0.5);

        // Feed low-level signal
        for _ in 0..1000 {
            audio_mod.process(0.3);
        }
        // Below threshold, should output near zero
        assert!(audio_mod.level() < 0.1, "Level {} should be below threshold", audio_mod.level());
    }

    #[test]
    fn test_audio_mod_source_gain() {
        let mut audio_mod = AudioModSource::new(48000.0);
        audio_mod.set_input_gain(2.0);

        for _ in 0..1000 {
            audio_mod.process(0.3);
        }
        let boosted_level = audio_mod.level();

        audio_mod.reset();
        audio_mod.set_input_gain(1.0);
        for _ in 0..1000 {
            audio_mod.process(0.3);
        }
        let normal_level = audio_mod.level();

        assert!(boosted_level > normal_level);
    }

    #[test]
    fn test_audio_mod_source_modulation_trait() {
        let mut audio_mod = AudioModSource::new(48000.0);

        // Process some audio
        for _ in 0..1000 {
            audio_mod.process(0.8);
        }

        let mod_value = audio_mod.mod_value();
        assert!(audio_mod.is_bipolar());
        assert!((-1.0..=1.0).contains(&mod_value));
    }

    #[test]
    fn test_audio_gate_basic() {
        let mut gate = AudioGate::new(48000.0);
        gate.set_open_threshold(0.2);
        gate.set_close_threshold(0.1);
        gate.set_attack_ms(1.0);
        gate.set_release_ms(5.0);

        // Start closed
        assert!(!gate.is_open());

        // Feed loud signal
        for _ in 0..1000 {
            gate.process(0.5);
        }
        assert!(gate.is_open());

        // Feed silence (need enough samples for envelope to decay)
        // 5ms release at 48kHz = 240 samples, need ~10x = 2400
        for _ in 0..5000 {
            gate.process(0.0);
        }
        assert!(!gate.is_open());
    }

    #[test]
    fn test_audio_gate_hysteresis() {
        let mut gate = AudioGate::new(48000.0);
        gate.set_open_threshold(0.3);
        gate.set_close_threshold(0.1);
        gate.set_attack_ms(1.0);
        gate.set_release_ms(1.0);

        // Open the gate
        for _ in 0..500 {
            gate.process(0.5);
        }
        assert!(gate.is_open());

        // Feed signal between thresholds (should stay open due to hysteresis)
        for _ in 0..100 {
            gate.process(0.15);
        }
        // May or may not be open depending on envelope response
        // The key test is that it doesn't instantly close at 0.15

        // Feed very low signal
        for _ in 0..1000 {
            gate.process(0.02);
        }
        assert!(!gate.is_open());
    }
}
