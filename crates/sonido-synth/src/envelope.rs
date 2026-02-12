//! ADSR envelope generator for synthesis.
//!
//! Provides attack-decay-sustain-release envelopes with exponential curves
//! for natural-sounding amplitude and filter modulation.

use libm::expf;

/// ADSR envelope states
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvelopeState {
    /// Envelope is inactive — output is zero.
    #[default]
    Idle,
    /// Attack phase — output ramps up toward peak level.
    Attack,
    /// Decay phase — output falls from peak toward sustain level.
    Decay,
    /// Sustain phase — output holds at sustain level while gate is held.
    Sustain,
    /// Release phase — output decays to zero after gate release.
    Release,
}

/// ADSR envelope generator.
///
/// Generates attack-decay-sustain-release envelopes for controlling
/// amplitude, filter cutoff, or other parameters.
///
/// # Features
///
/// - Exponential curves for natural sound
/// - Retriggering support
/// - Configurable attack/decay/release times
/// - Adjustable sustain level
///
/// # Example
///
/// ```rust
/// use sonido_synth::{AdsrEnvelope, EnvelopeState};
///
/// let mut env = AdsrEnvelope::new(48000.0);
/// env.set_attack_ms(10.0);
/// env.set_decay_ms(100.0);
/// env.set_sustain(0.7);
/// env.set_release_ms(200.0);
///
/// // Trigger the envelope
/// env.gate_on();
///
/// // Process samples
/// for _ in 0..1000 {
///     let level = env.advance();
///     // Use level to modulate amplitude, filter, etc.
/// }
///
/// // Release
/// env.gate_off();
/// ```
#[derive(Debug, Clone)]
pub struct AdsrEnvelope {
    /// Current state
    state: EnvelopeState,
    /// Current output level
    level: f32,
    /// Sample rate
    sample_rate: f32,

    // Time parameters (in milliseconds)
    attack_ms: f32,
    decay_ms: f32,
    release_ms: f32,
    sustain: f32,

    // Coefficients (pre-calculated)
    attack_coeff: f32,
    decay_coeff: f32,
    release_coeff: f32,

    // Target levels for exponential curves
    attack_target: f32,
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl AdsrEnvelope {
    /// Create a new ADSR envelope with default settings.
    ///
    /// Default values:
    /// - Attack: 10ms
    /// - Decay: 100ms
    /// - Sustain: 0.7
    /// - Release: 200ms
    pub fn new(sample_rate: f32) -> Self {
        let mut env = Self {
            state: EnvelopeState::Idle,
            level: 0.0,
            sample_rate,
            attack_ms: 10.0,
            decay_ms: 100.0,
            release_ms: 200.0,
            sustain: 0.7,
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            release_coeff: 0.0,
            attack_target: 1.2, // Overshoot for snappier attack
        };
        env.recalculate_coefficients();
        env
    }

    /// Set attack time in milliseconds.
    pub fn set_attack_ms(&mut self, ms: f32) {
        self.attack_ms = ms.max(0.1);
        self.recalculate_attack_coeff();
    }

    /// Get attack time in milliseconds.
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    /// Set decay time in milliseconds.
    pub fn set_decay_ms(&mut self, ms: f32) {
        self.decay_ms = ms.max(0.1);
        self.recalculate_decay_coeff();
    }

    /// Get decay time in milliseconds.
    pub fn decay_ms(&self) -> f32 {
        self.decay_ms
    }

    /// Set sustain level (0.0 to 1.0).
    pub fn set_sustain(&mut self, level: f32) {
        self.sustain = level.clamp(0.0, 1.0);
    }

    /// Get sustain level.
    pub fn sustain(&self) -> f32 {
        self.sustain
    }

    /// Set release time in milliseconds.
    pub fn set_release_ms(&mut self, ms: f32) {
        self.release_ms = ms.max(0.1);
        self.recalculate_release_coeff();
    }

    /// Get release time in milliseconds.
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }

    /// Set sample rate.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.recalculate_coefficients();
    }

    /// Trigger the envelope (note on).
    pub fn gate_on(&mut self) {
        self.state = EnvelopeState::Attack;
        // Don't reset level for smooth retriggering
    }

    /// Release the envelope (note off).
    pub fn gate_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
        }
    }

    /// Force envelope to idle state.
    pub fn reset(&mut self) {
        self.state = EnvelopeState::Idle;
        self.level = 0.0;
    }

    /// Get current state.
    pub fn state(&self) -> EnvelopeState {
        self.state
    }

    /// Get current level without advancing.
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Check if envelope is active (not idle).
    pub fn is_active(&self) -> bool {
        self.state != EnvelopeState::Idle
    }

    /// Advance envelope by one sample and return current level.
    #[inline]
    pub fn advance(&mut self) -> f32 {
        match self.state {
            EnvelopeState::Idle => {
                self.level = 0.0;
            }

            EnvelopeState::Attack => {
                // Exponential approach to target (overshoots 1.0 for snappier attack)
                self.level =
                    self.attack_target + (self.level - self.attack_target) * self.attack_coeff;

                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.state = EnvelopeState::Decay;
                }
            }

            EnvelopeState::Decay => {
                // Exponential decay to sustain level
                self.level = self.sustain + (self.level - self.sustain) * self.decay_coeff;

                // Check if close enough to sustain
                if (self.level - self.sustain).abs() < 0.0001 {
                    self.level = self.sustain;
                    self.state = EnvelopeState::Sustain;
                }
            }

            EnvelopeState::Sustain => {
                self.level = self.sustain;
            }

            EnvelopeState::Release => {
                // Exponential decay to zero
                self.level *= self.release_coeff;

                if self.level < 0.0001 {
                    self.level = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
        }

        self.level
    }

    fn recalculate_coefficients(&mut self) {
        self.recalculate_attack_coeff();
        self.recalculate_decay_coeff();
        self.recalculate_release_coeff();
    }

    fn recalculate_attack_coeff(&mut self) {
        // Time constant for exponential: coefficient = exp(-1 / (time_samples))
        // We want to reach ~63% of target in the specified time
        let samples = self.attack_ms * self.sample_rate / 1000.0;
        self.attack_coeff = expf(-1.0 / samples.max(1.0));
    }

    fn recalculate_decay_coeff(&mut self) {
        let samples = self.decay_ms * self.sample_rate / 1000.0;
        self.decay_coeff = expf(-1.0 / samples.max(1.0));
    }

    fn recalculate_release_coeff(&mut self) {
        let samples = self.release_ms * self.sample_rate / 1000.0;
        self.release_coeff = expf(-1.0 / samples.max(1.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_envelope_idle_state() {
        let mut env = AdsrEnvelope::new(48000.0);
        assert_eq!(env.state(), EnvelopeState::Idle);
        assert_eq!(env.level(), 0.0);

        // Advancing in idle should stay at 0
        for _ in 0..100 {
            assert_eq!(env.advance(), 0.0);
        }
    }

    #[test]
    fn test_envelope_attack_phase() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(10.0);

        env.gate_on();
        assert_eq!(env.state(), EnvelopeState::Attack);

        // After attack time, should reach peak
        let attack_samples = (10.0 * 48.0) as usize; // ~10ms at 48kHz
        for _ in 0..attack_samples * 2 {
            env.advance();
        }

        // Should have transitioned to decay and reached near 1.0
        assert!(
            env.state() == EnvelopeState::Decay || env.state() == EnvelopeState::Sustain,
            "Expected Decay or Sustain, got {:?}",
            env.state()
        );
    }

    #[test]
    fn test_envelope_decay_to_sustain() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_decay_ms(10.0);
        env.set_sustain(0.5);

        env.gate_on();

        // Run through attack and decay
        for _ in 0..5000 {
            env.advance();
        }

        assert_eq!(env.state(), EnvelopeState::Sustain);
        assert!(
            (env.level() - 0.5).abs() < 0.01,
            "Expected sustain level 0.5, got {}",
            env.level()
        );
    }

    #[test]
    fn test_envelope_release() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_decay_ms(1.0);
        env.set_sustain(0.7);
        env.set_release_ms(50.0);

        // Go to sustain
        env.gate_on();
        for _ in 0..2000 {
            env.advance();
        }

        // Release
        env.gate_off();
        assert_eq!(env.state(), EnvelopeState::Release);

        // After release time, should be idle
        // 50ms at 48kHz = 2400 samples, need ~10x time constants = 24000 samples
        for _ in 0..30000 {
            env.advance();
        }

        assert_eq!(env.state(), EnvelopeState::Idle);
        assert!(env.level() < 0.001);
    }

    #[test]
    fn test_envelope_retrigger() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(5.0);

        // First trigger
        env.gate_on();
        for _ in 0..200 {
            env.advance();
        }
        let level_before = env.level();

        // Retrigger while still in attack
        env.gate_on();

        // Level should be preserved (smooth retrigger)
        assert!(
            (env.level() - level_before).abs() < 0.001,
            "Retrigger should preserve level"
        );
    }

    #[test]
    fn test_envelope_output_range() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(5.0);
        env.set_decay_ms(20.0);
        env.set_sustain(0.6);
        env.set_release_ms(50.0);

        env.gate_on();

        // Full cycle
        for _ in 0..2000 {
            let level = env.advance();
            assert!(
                (0.0..=1.01).contains(&level), // Small overshoot allowed
                "Level out of range: {}",
                level
            );
        }

        env.gate_off();

        for _ in 0..5000 {
            let level = env.advance();
            assert!(
                (0.0..=1.0).contains(&level),
                "Level out of range during release: {}",
                level
            );
        }
    }

    #[test]
    fn test_envelope_is_active() {
        let mut env = AdsrEnvelope::new(48000.0);

        assert!(!env.is_active());

        env.gate_on();
        assert!(env.is_active());

        env.reset();
        assert!(!env.is_active());
    }

    #[test]
    fn test_envelope_state_transitions() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_decay_ms(5.0);
        env.set_sustain(0.5);
        env.set_release_ms(10.0);

        // Start idle
        assert_eq!(env.state(), EnvelopeState::Idle);

        // Gate on -> Attack
        env.gate_on();
        assert_eq!(env.state(), EnvelopeState::Attack);

        // Run through attack
        for _ in 0..1000 {
            env.advance();
            if env.state() == EnvelopeState::Decay {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Decay);

        // Run through decay (5ms at 48kHz = 240 samples, need ~10x = 2400)
        for _ in 0..5000 {
            env.advance();
            if env.state() == EnvelopeState::Sustain {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Sustain);

        // Gate off -> Release
        env.gate_off();
        assert_eq!(env.state(), EnvelopeState::Release);

        // Run through release (10ms at 48kHz = 480 samples, need ~10x = 4800)
        for _ in 0..20000 {
            env.advance();
            if env.state() == EnvelopeState::Idle {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Idle);
    }
}
