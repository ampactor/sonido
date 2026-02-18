//! ADSR envelope generator for synthesis.
//!
//! Provides attack-hold-decay-sustain-release envelopes with configurable
//! per-segment curve shapes for natural-sounding amplitude and filter modulation.
//!
//! # Curve Shapes
//!
//! Each segment (attack, decay, release) has a curve parameter from -1.0 to 1.0:
//! - **0.0**: Default exponential behavior (unchanged from standard ADSR)
//! - **Negative**: Less overshoot (attack) or slower coefficient (decay/release),
//!   producing a more linear, gradual contour
//! - **Positive**: More overshoot past the segment target, producing a snappier,
//!   more aggressive contour
//!
//! # Hold Phase
//!
//! An optional hold phase sits between attack and decay, holding the envelope
//! at peak level (1.0) for a configurable duration before decay begins.
//! Set `hold_time_ms` to 0.0 (default) to skip the hold phase entirely.

use libm::expf;

/// Release convergence threshold (approximately -100 dB).
const RELEASE_THRESHOLD: f32 = 0.00001;

/// Decay convergence threshold.
const DECAY_THRESHOLD: f32 = 0.0001;

/// ADSR envelope states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvelopeState {
    /// Envelope is inactive — output is zero.
    #[default]
    Idle,
    /// Attack phase — output ramps up toward peak level.
    Attack,
    /// Hold phase — output holds at peak level before decay begins.
    Hold,
    /// Decay phase — output falls from peak toward sustain level.
    Decay,
    /// Sustain phase — output holds at sustain level while gate is held.
    Sustain,
    /// Release phase — output decays to zero after gate release.
    Release,
}

/// ADSR envelope generator with per-segment curve shaping and optional hold.
///
/// Generates attack-hold-decay-sustain-release envelopes for controlling
/// amplitude, filter cutoff, or other parameters.
///
/// ## Parameters
///
/// - `attack_ms`: Attack time in milliseconds (0.1+, default 10.0)
/// - `hold_time_ms`: Hold time in milliseconds (0.0 to 500.0, default 0.0)
/// - `decay_ms`: Decay time in milliseconds (0.1+, default 100.0)
/// - `sustain`: Sustain level (0.0 to 1.0, default 0.7)
/// - `release_ms`: Release time in milliseconds (0.1+, default 200.0)
/// - `attack_curve`: Attack curve shape (-1.0 to 1.0, default 0.0)
/// - `decay_curve`: Decay curve shape (-1.0 to 1.0, default 0.0)
/// - `release_curve`: Release curve shape (-1.0 to 1.0, default 0.0)
///
/// ## Features
///
/// - Per-segment exponential curve shaping via overshoot targets
/// - Optional hold phase between attack and decay
/// - Retriggering support (smooth level preservation)
/// - Tighter release convergence at -100 dB
///
/// ## Example
///
/// ```rust
/// use sonido_synth::{AdsrEnvelope, EnvelopeState};
///
/// let mut env = AdsrEnvelope::new(48000.0);
/// env.set_attack_ms(10.0);
/// env.set_hold_time_ms(5.0);
/// env.set_decay_ms(100.0);
/// env.set_sustain(0.7);
/// env.set_release_ms(200.0);
/// env.set_attack_curve(0.5);  // snappier attack
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
    /// Current state.
    state: EnvelopeState,
    /// Current output level.
    level: f32,
    /// Sample rate in Hz.
    sample_rate: f32,

    // Time parameters (in milliseconds)
    /// Attack time in milliseconds.
    attack_ms: f32,
    /// Hold time in milliseconds (0.0 = no hold phase).
    hold_time_ms: f32,
    /// Decay time in milliseconds.
    decay_ms: f32,
    /// Release time in milliseconds.
    release_ms: f32,
    /// Sustain level (0.0 to 1.0).
    sustain: f32,

    // Curve shapes (-1.0 to 1.0, default 0.0)
    /// Attack curve: negative = more linear, positive = snappier.
    attack_curve: f32,
    /// Decay curve: negative = more linear, positive = snappier.
    decay_curve: f32,
    /// Release curve: negative = more linear, positive = snappier.
    release_curve: f32,

    // Coefficients (pre-calculated)
    /// Attack exponential coefficient.
    attack_coeff: f32,
    /// Decay exponential coefficient.
    decay_coeff: f32,
    /// Release exponential coefficient.
    release_coeff: f32,

    // Target levels for exponential curves (overshoot past segment destination)
    /// Attack overshoot target (always > 1.0, clipped at 1.0).
    attack_target: f32,
    /// Decay overshoot target (at or below sustain level).
    decay_target: f32,
    /// Release overshoot target (at or below zero).
    release_target: f32,

    // Hold state
    /// Number of hold samples (computed from hold_time_ms).
    hold_samples: u32,
    /// Countdown during Hold state.
    hold_counter: u32,
}

impl Default for AdsrEnvelope {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl AdsrEnvelope {
    /// Creates a new ADSR envelope with default settings.
    ///
    /// Default values:
    /// - Attack: 10ms
    /// - Hold: 0ms (disabled)
    /// - Decay: 100ms
    /// - Sustain: 0.7
    /// - Release: 200ms
    /// - All curves: 0.0 (standard exponential)
    pub fn new(sample_rate: f32) -> Self {
        let mut env = Self {
            state: EnvelopeState::Idle,
            level: 0.0,
            sample_rate,
            attack_ms: 10.0,
            hold_time_ms: 0.0,
            decay_ms: 100.0,
            release_ms: 200.0,
            sustain: 0.7,
            attack_curve: 0.0,
            decay_curve: 0.0,
            release_curve: 0.0,
            attack_coeff: 0.0,
            decay_coeff: 0.0,
            release_coeff: 0.0,
            attack_target: 1.2,
            decay_target: 0.7,
            release_target: 0.0,
            hold_samples: 0,
            hold_counter: 0,
        };
        env.recalculate_coefficients();
        env
    }

    /// Sets the attack time in milliseconds.
    ///
    /// Range: 0.1 ms minimum. Values below 0.1 are clamped.
    pub fn set_attack_ms(&mut self, ms: f32) {
        self.attack_ms = ms.max(0.1);
        self.recalculate_attack_coeff();
    }

    /// Returns the attack time in milliseconds.
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms
    }

    /// Sets the hold time in milliseconds.
    ///
    /// Range: 0.0 to 500.0 ms. Set to 0.0 to disable the hold phase.
    /// The hold phase sits between attack and decay, holding at peak level (1.0).
    pub fn set_hold_time_ms(&mut self, ms: f32) {
        self.hold_time_ms = ms.clamp(0.0, 500.0);
        self.recalculate_hold_samples();
    }

    /// Returns the hold time in milliseconds.
    pub fn hold_time_ms(&self) -> f32 {
        self.hold_time_ms
    }

    /// Sets the decay time in milliseconds.
    ///
    /// Range: 0.1 ms minimum. Values below 0.1 are clamped.
    pub fn set_decay_ms(&mut self, ms: f32) {
        self.decay_ms = ms.max(0.1);
        self.recalculate_decay_coeff();
    }

    /// Returns the decay time in milliseconds.
    pub fn decay_ms(&self) -> f32 {
        self.decay_ms
    }

    /// Sets the sustain level.
    ///
    /// Range: 0.0 to 1.0. Values are clamped to this range.
    pub fn set_sustain(&mut self, level: f32) {
        self.sustain = level.clamp(0.0, 1.0);
        // Decay target depends on sustain
        self.recalculate_decay_target();
    }

    /// Returns the sustain level.
    pub fn sustain(&self) -> f32 {
        self.sustain
    }

    /// Sets the release time in milliseconds.
    ///
    /// Range: 0.1 ms minimum. Values below 0.1 are clamped.
    pub fn set_release_ms(&mut self, ms: f32) {
        self.release_ms = ms.max(0.1);
        self.recalculate_release_coeff();
    }

    /// Returns the release time in milliseconds.
    pub fn release_ms(&self) -> f32 {
        self.release_ms
    }

    /// Sets the attack curve shape.
    ///
    /// Range: -1.0 to 1.0, default 0.0.
    /// - Negative: reduces overshoot target (1.2 → 1.05), producing a slower, more linear rise
    /// - Zero: default exponential with overshoot target at 1.2
    /// - Positive: increases overshoot target (1.2 → 1.5), producing a snappier attack
    pub fn set_attack_curve(&mut self, curve: f32) {
        self.attack_curve = curve.clamp(-1.0, 1.0);
        self.recalculate_attack_coeff();
    }

    /// Returns the attack curve shape.
    pub fn attack_curve(&self) -> f32 {
        self.attack_curve
    }

    /// Sets the decay curve shape.
    ///
    /// Range: -1.0 to 1.0, default 0.0.
    /// - Negative: slows the decay coefficient for a more linear descent
    /// - Zero: default exponential decay toward sustain
    /// - Positive: overshoots below sustain for a faster initial drop
    pub fn set_decay_curve(&mut self, curve: f32) {
        self.decay_curve = curve.clamp(-1.0, 1.0);
        self.recalculate_decay_coeff();
    }

    /// Returns the decay curve shape.
    pub fn decay_curve(&self) -> f32 {
        self.decay_curve
    }

    /// Sets the release curve shape.
    ///
    /// Range: -1.0 to 1.0, default 0.0.
    /// - Negative: slows the release coefficient for a more gradual tail
    /// - Zero: default exponential release toward zero
    /// - Positive: overshoots below zero for a faster initial release
    pub fn set_release_curve(&mut self, curve: f32) {
        self.release_curve = curve.clamp(-1.0, 1.0);
        self.recalculate_release_coeff();
    }

    /// Returns the release curve shape.
    pub fn release_curve(&self) -> f32 {
        self.release_curve
    }

    /// Sets the sample rate and recalculates all coefficients.
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.recalculate_coefficients();
    }

    /// Triggers the envelope (note on).
    ///
    /// Enters attack phase without resetting the current level,
    /// allowing smooth retriggering from any state.
    pub fn gate_on(&mut self) {
        self.state = EnvelopeState::Attack;
        // Don't reset level for smooth retriggering
    }

    /// Releases the envelope (note off).
    ///
    /// Transitions to release phase from any active state.
    /// Has no effect if the envelope is already idle.
    pub fn gate_off(&mut self) {
        if self.state != EnvelopeState::Idle {
            self.state = EnvelopeState::Release;
        }
    }

    /// Forces the envelope to idle state with zero output.
    pub fn reset(&mut self) {
        self.state = EnvelopeState::Idle;
        self.level = 0.0;
        self.hold_counter = 0;
    }

    /// Returns the current envelope state.
    pub fn state(&self) -> EnvelopeState {
        self.state
    }

    /// Returns the current output level without advancing.
    pub fn level(&self) -> f32 {
        self.level
    }

    /// Returns `true` if the envelope is active (not idle).
    pub fn is_active(&self) -> bool {
        self.state != EnvelopeState::Idle
    }

    /// Advances the envelope by one sample and returns the current level.
    ///
    /// Call this once per sample in the audio processing loop.
    /// The returned value is in the range 0.0 to 1.0.
    #[inline]
    pub fn advance(&mut self) -> f32 {
        match self.state {
            EnvelopeState::Idle => {
                self.level = 0.0;
            }

            EnvelopeState::Attack => {
                // Exponential approach to overshoot target (always > 1.0)
                self.level =
                    self.attack_target + (self.level - self.attack_target) * self.attack_coeff;

                if self.level >= 1.0 {
                    self.level = 1.0;
                    if self.hold_samples > 0 {
                        self.hold_counter = self.hold_samples;
                        self.state = EnvelopeState::Hold;
                    } else {
                        self.state = EnvelopeState::Decay;
                    }
                }
            }

            EnvelopeState::Hold => {
                self.level = 1.0;
                if self.hold_counter > 0 {
                    self.hold_counter -= 1;
                } else {
                    self.state = EnvelopeState::Decay;
                }
            }

            EnvelopeState::Decay => {
                // Exponential approach toward decay target (at or below sustain)
                self.level =
                    self.decay_target + (self.level - self.decay_target) * self.decay_coeff;

                // Snap to sustain when close enough
                if (self.level - self.sustain).abs() < DECAY_THRESHOLD {
                    self.level = self.sustain;
                    self.state = EnvelopeState::Sustain;
                }
            }

            EnvelopeState::Sustain => {
                self.level = self.sustain;
            }

            EnvelopeState::Release => {
                // Exponential approach toward release target (at or below zero)
                self.level =
                    self.release_target + (self.level - self.release_target) * self.release_coeff;

                // Clamp above zero (release target can be negative with positive curve)
                if self.level < 0.0 {
                    self.level = 0.0;
                }

                if self.level < RELEASE_THRESHOLD {
                    self.level = 0.0;
                    self.state = EnvelopeState::Idle;
                }
            }
        }

        self.level
    }

    /// Recalculates all coefficients, targets, and hold samples.
    fn recalculate_coefficients(&mut self) {
        self.recalculate_attack_coeff();
        self.recalculate_decay_coeff();
        self.recalculate_release_coeff();
        self.recalculate_hold_samples();
    }

    /// Recalculates attack coefficient and overshoot target from attack time and curve.
    ///
    /// The attack target is always above 1.0. The curve parameter controls
    /// the amount of overshoot, which shapes the approach contour:
    /// - curve = -1.0 → target = 1.05 (barely overshoots, more linear)
    /// - curve =  0.0 → target = 1.2  (default snappy attack)
    /// - curve = +1.0 → target = 1.5  (aggressive overshoot, very snappy)
    fn recalculate_attack_coeff(&mut self) {
        let samples = self.attack_ms * self.sample_rate / 1000.0;
        self.attack_coeff = expf(-1.0 / samples.max(1.0));

        // Overshoot target: asymmetric scaling around default 1.2
        let curve = self.attack_curve;
        if curve < 0.0 {
            // Toward 1.05: range 0.15 over curve [-1, 0]
            self.attack_target = 1.2 + curve * 0.15;
        } else {
            // Toward 1.5: range 0.3 over curve [0, 1]
            self.attack_target = 1.2 + curve * 0.3;
        }
    }

    /// Recalculates decay coefficient and overshoot target from decay time and curve.
    ///
    /// For positive curves, the target overshoots below sustain for a faster initial drop.
    /// For negative curves, the coefficient is slowed for a more linear descent.
    fn recalculate_decay_coeff(&mut self) {
        let curve = self.decay_curve;
        let mut samples = self.decay_ms * self.sample_rate / 1000.0;

        // Negative curve: slow down for more linear feel (up to 2x at curve=-1)
        if curve < 0.0 {
            samples *= 1.0 - curve; // curve is negative, so this adds
        }

        self.decay_coeff = expf(-1.0 / samples.max(1.0));
        self.recalculate_decay_target();
    }

    /// Recalculates the decay overshoot target from sustain level and decay curve.
    fn recalculate_decay_target(&mut self) {
        let curve = self.decay_curve;
        if curve > 0.0 {
            // Overshoot below sustain: proportional to the drop distance (1.0 - sustain)
            let range = 1.0 - self.sustain;
            self.decay_target = self.sustain - curve * range * 0.3;
        } else {
            // No overshoot for zero/negative curves
            self.decay_target = self.sustain;
        }
    }

    /// Recalculates release coefficient and overshoot target from release time and curve.
    ///
    /// For positive curves, the target overshoots below zero for a faster initial release.
    /// For negative curves, the coefficient is slowed for a more gradual tail.
    fn recalculate_release_coeff(&mut self) {
        let curve = self.release_curve;
        let mut samples = self.release_ms * self.sample_rate / 1000.0;

        // Negative curve: slow down for more linear feel (up to 2x at curve=-1)
        if curve < 0.0 {
            samples *= 1.0 - curve;
        }

        self.release_coeff = expf(-1.0 / samples.max(1.0));

        // Positive curve: overshoot below zero
        if curve > 0.0 {
            self.release_target = -curve * 0.3;
        } else {
            self.release_target = 0.0;
        }
    }

    /// Recalculates hold sample count from hold time and sample rate.
    fn recalculate_hold_samples(&mut self) {
        self.hold_samples = (self.hold_time_ms * self.sample_rate / 1000.0) as u32;
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
        // 50ms at 48kHz = 2400 samples, need ~10x time constants
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

    // --- Hold phase tests ---

    #[test]
    fn test_hold_phase() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_hold_time_ms(10.0); // 10ms hold = 480 samples at 48kHz
        env.set_decay_ms(10.0);
        env.set_sustain(0.5);

        env.gate_on();

        // Run through attack
        for _ in 0..1000 {
            env.advance();
            if env.state() == EnvelopeState::Hold {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Hold);
        assert!((env.level() - 1.0).abs() < f32::EPSILON);

        // Hold should maintain 1.0 for ~480 samples
        for _ in 0..200 {
            let level = env.advance();
            assert!(
                (level - 1.0).abs() < f32::EPSILON,
                "Hold level should be 1.0, got {level}"
            );
        }
        assert_eq!(env.state(), EnvelopeState::Hold, "Should still be in Hold");

        // Run remaining hold + into decay
        for _ in 0..1000 {
            env.advance();
            if env.state() == EnvelopeState::Decay {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Decay);
    }

    #[test]
    fn test_hold_zero_skipped() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_hold_time_ms(0.0); // no hold
        env.set_decay_ms(10.0);
        env.set_sustain(0.5);

        env.gate_on();

        // Run through attack into decay — should never see Hold
        let mut saw_hold = false;
        for _ in 0..2000 {
            env.advance();
            if env.state() == EnvelopeState::Hold {
                saw_hold = true;
            }
            if env.state() == EnvelopeState::Decay {
                break;
            }
        }
        assert!(
            !saw_hold,
            "Hold phase should be skipped when hold_time_ms=0"
        );
        assert_eq!(env.state(), EnvelopeState::Decay);
    }

    #[test]
    fn test_gate_off_during_hold() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_hold_time_ms(100.0); // long hold
        env.set_sustain(0.5);

        env.gate_on();

        // Get to Hold state
        for _ in 0..1000 {
            env.advance();
            if env.state() == EnvelopeState::Hold {
                break;
            }
        }
        assert_eq!(env.state(), EnvelopeState::Hold);

        // Gate off during hold should go to Release
        env.gate_off();
        assert_eq!(env.state(), EnvelopeState::Release);
    }

    #[test]
    fn test_hold_state_transitions() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(1.0);
        env.set_hold_time_ms(5.0); // 5ms = 240 samples
        env.set_decay_ms(5.0);
        env.set_sustain(0.5);
        env.set_release_ms(10.0);

        env.gate_on();

        // Track all visited states
        let mut visited_attack = false;
        let mut visited_hold = false;
        let mut visited_decay = false;
        let mut visited_sustain = false;

        for _ in 0..10000 {
            env.advance();
            match env.state() {
                EnvelopeState::Attack => visited_attack = true,
                EnvelopeState::Hold => visited_hold = true,
                EnvelopeState::Decay => visited_decay = true,
                EnvelopeState::Sustain => {
                    visited_sustain = true;
                    break;
                }
                _ => {}
            }
        }

        assert!(visited_attack, "Should visit Attack");
        assert!(visited_hold, "Should visit Hold");
        assert!(visited_decay, "Should visit Decay");
        assert!(visited_sustain, "Should visit Sustain");
    }

    // --- Curve shape tests ---

    #[test]
    fn test_attack_curve_positive_faster() {
        // Positive curve (more overshoot) should reach peak in fewer samples
        let mut env_default = AdsrEnvelope::new(48000.0);
        env_default.set_attack_ms(20.0);

        let mut env_snappy = AdsrEnvelope::new(48000.0);
        env_snappy.set_attack_ms(20.0);
        env_snappy.set_attack_curve(1.0);

        env_default.gate_on();
        env_snappy.gate_on();

        let mut samples_default = 0u32;
        let mut samples_snappy = 0u32;

        for i in 0..10000_u32 {
            env_default.advance();
            if env_default.state() != EnvelopeState::Attack && samples_default == 0 {
                samples_default = i;
            }
            env_snappy.advance();
            if env_snappy.state() != EnvelopeState::Attack && samples_snappy == 0 {
                samples_snappy = i;
            }
            if samples_default > 0 && samples_snappy > 0 {
                break;
            }
        }

        assert!(
            samples_snappy < samples_default,
            "Positive curve should reach peak faster: snappy={samples_snappy}, default={samples_default}"
        );
    }

    #[test]
    fn test_attack_curve_negative_slower() {
        // Negative curve (less overshoot) should reach peak in more samples
        let mut env_default = AdsrEnvelope::new(48000.0);
        env_default.set_attack_ms(20.0);

        let mut env_slow = AdsrEnvelope::new(48000.0);
        env_slow.set_attack_ms(20.0);
        env_slow.set_attack_curve(-1.0);

        env_default.gate_on();
        env_slow.gate_on();

        let mut samples_default = 0u32;
        let mut samples_slow = 0u32;

        for i in 0..20000_u32 {
            env_default.advance();
            if env_default.state() != EnvelopeState::Attack && samples_default == 0 {
                samples_default = i;
            }
            env_slow.advance();
            if env_slow.state() != EnvelopeState::Attack && samples_slow == 0 {
                samples_slow = i;
            }
            if samples_default > 0 && samples_slow > 0 {
                break;
            }
        }

        assert!(
            samples_slow > samples_default,
            "Negative curve should reach peak slower: slow={samples_slow}, default={samples_default}"
        );
    }

    #[test]
    fn test_decay_curve_positive_faster() {
        // Positive decay curve should reach sustain faster
        let mut env_default = AdsrEnvelope::new(48000.0);
        env_default.set_attack_ms(0.1);
        env_default.set_decay_ms(50.0);
        env_default.set_sustain(0.3);

        let mut env_fast = AdsrEnvelope::new(48000.0);
        env_fast.set_attack_ms(0.1);
        env_fast.set_decay_ms(50.0);
        env_fast.set_sustain(0.3);
        env_fast.set_decay_curve(1.0);

        env_default.gate_on();
        env_fast.gate_on();

        let mut samples_default = 0u32;
        let mut samples_fast = 0u32;

        for i in 0..100_000_u32 {
            env_default.advance();
            if env_default.state() == EnvelopeState::Sustain && samples_default == 0 {
                samples_default = i;
            }
            env_fast.advance();
            if env_fast.state() == EnvelopeState::Sustain && samples_fast == 0 {
                samples_fast = i;
            }
            if samples_default > 0 && samples_fast > 0 {
                break;
            }
        }

        assert!(
            samples_fast < samples_default,
            "Positive decay curve should reach sustain faster: fast={samples_fast}, default={samples_default}"
        );
    }

    #[test]
    fn test_release_curve_positive_faster() {
        // Positive release curve should reach idle faster
        let mut env_default = AdsrEnvelope::new(48000.0);
        env_default.set_attack_ms(0.1);
        env_default.set_decay_ms(0.1);
        env_default.set_sustain(0.8);
        env_default.set_release_ms(100.0);

        let mut env_fast = AdsrEnvelope::new(48000.0);
        env_fast.set_attack_ms(0.1);
        env_fast.set_decay_ms(0.1);
        env_fast.set_sustain(0.8);
        env_fast.set_release_ms(100.0);
        env_fast.set_release_curve(1.0);

        // Get both to sustain
        env_default.gate_on();
        env_fast.gate_on();
        for _ in 0..2000 {
            env_default.advance();
            env_fast.advance();
        }

        env_default.gate_off();
        env_fast.gate_off();

        let mut samples_default = 0u32;
        let mut samples_fast = 0u32;

        for i in 0..200_000_u32 {
            env_default.advance();
            if env_default.state() == EnvelopeState::Idle && samples_default == 0 {
                samples_default = i;
            }
            env_fast.advance();
            if env_fast.state() == EnvelopeState::Idle && samples_fast == 0 {
                samples_fast = i;
            }
            if samples_default > 0 && samples_fast > 0 {
                break;
            }
        }

        assert!(
            samples_fast < samples_default,
            "Positive release curve should reach idle faster: fast={samples_fast}, default={samples_default}"
        );
    }

    #[test]
    fn test_curve_defaults_preserve_behavior() {
        // With all curves at 0.0, behavior should match the original envelope
        let env = AdsrEnvelope::new(48000.0);
        assert!((env.attack_curve() - 0.0).abs() < f32::EPSILON);
        assert!((env.decay_curve() - 0.0).abs() < f32::EPSILON);
        assert!((env.release_curve() - 0.0).abs() < f32::EPSILON);

        // Default attack target should be 1.2
        assert!((env.attack_target - 1.2).abs() < f32::EPSILON);
        // Default decay target should equal sustain (0.7)
        assert!((env.decay_target - 0.7).abs() < f32::EPSILON);
        // Default release target should be 0.0
        assert!((env.release_target - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_curve_clamping() {
        let mut env = AdsrEnvelope::new(48000.0);

        env.set_attack_curve(5.0);
        assert!((env.attack_curve() - 1.0).abs() < f32::EPSILON);

        env.set_decay_curve(-10.0);
        assert!((env.decay_curve() - (-1.0)).abs() < f32::EPSILON);

        env.set_release_curve(3.0);
        assert!((env.release_curve() - 1.0).abs() < f32::EPSILON);
    }

    // --- Tighter convergence threshold tests ---

    #[test]
    fn test_release_threshold_tight() {
        // Verify the release threshold is approximately -100 dB (1e-5),
        // not the old -80 dB (1e-4) threshold.
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_attack_ms(0.1);
        env.set_decay_ms(0.1);
        env.set_sustain(1.0);
        env.set_release_ms(10.0);

        env.gate_on();
        for _ in 0..500 {
            env.advance();
        }

        env.gate_off();

        // Count samples until idle and track level just before transition.
        // The last level before idle is just above the threshold (by one step),
        // so we verify it's in the right order of magnitude.
        let mut samples_to_idle = 0_u32;
        let mut last_active_level = 0.0_f32;
        for i in 0..100_000_u32 {
            last_active_level = env.level();
            env.advance();
            if env.state() == EnvelopeState::Idle {
                samples_to_idle = i;
                break;
            }
        }

        assert_eq!(env.state(), EnvelopeState::Idle);
        // The last active level should be close to the threshold (within 2x)
        assert!(
            last_active_level < RELEASE_THRESHOLD * 2.0,
            "Last active level should be near {RELEASE_THRESHOLD}, was {last_active_level}"
        );
        // Must not have triggered at the old 1e-4 threshold — would have been fewer samples.
        // 10ms release at 48kHz = 480 sample time constant.
        // For level to drop from 1.0 to 1e-4: ~4420 samples
        // For level to drop from 1.0 to 1e-5: ~5525 samples
        assert!(
            samples_to_idle > 5000,
            "Should need >5000 samples to reach -100dB threshold, got {samples_to_idle}"
        );
    }

    #[test]
    fn test_output_range_with_curves() {
        // Even with extreme curves, output should stay in [0.0, 1.0]
        for &attack_c in &[-1.0, -0.5, 0.0, 0.5, 1.0] {
            for &decay_c in &[-1.0, 0.0, 1.0] {
                for &release_c in &[-1.0, 0.0, 1.0] {
                    let mut env = AdsrEnvelope::new(48000.0);
                    env.set_attack_ms(5.0);
                    env.set_decay_ms(20.0);
                    env.set_sustain(0.5);
                    env.set_release_ms(30.0);
                    env.set_attack_curve(attack_c);
                    env.set_decay_curve(decay_c);
                    env.set_release_curve(release_c);

                    env.gate_on();
                    for _ in 0..3000 {
                        let level = env.advance();
                        assert!(
                            (0.0..=1.01).contains(&level),
                            "Level {level} out of range with curves ({attack_c}, {decay_c}, {release_c})"
                        );
                    }

                    env.gate_off();
                    for _ in 0..10000 {
                        let level = env.advance();
                        assert!(
                            (0.0..=1.0).contains(&level),
                            "Release level {level} out of range with curves ({attack_c}, {decay_c}, {release_c})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn test_hold_getters_setters() {
        let mut env = AdsrEnvelope::new(48000.0);
        assert!((env.hold_time_ms() - 0.0).abs() < f32::EPSILON);

        env.set_hold_time_ms(50.0);
        assert!((env.hold_time_ms() - 50.0).abs() < f32::EPSILON);

        // Clamp to 500ms max
        env.set_hold_time_ms(1000.0);
        assert!((env.hold_time_ms() - 500.0).abs() < f32::EPSILON);

        // Clamp to 0ms min
        env.set_hold_time_ms(-10.0);
        assert!((env.hold_time_ms() - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_sustain_change_updates_decay_target() {
        let mut env = AdsrEnvelope::new(48000.0);
        env.set_decay_curve(0.5);

        env.set_sustain(0.5);
        let target_a = env.decay_target;

        env.set_sustain(0.8);
        let target_b = env.decay_target;

        // Different sustain should produce different decay targets
        assert!(
            (target_a - target_b).abs() > 0.01,
            "Decay target should change with sustain: a={target_a}, b={target_b}"
        );
    }
}
