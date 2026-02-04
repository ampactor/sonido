//! Noise gate effect for silencing signals below a threshold.
//!
//! A gate attenuates signals that fall below a threshold level,
//! useful for removing noise, bleed, or unwanted quiet sounds.

use sonido_core::{Effect, SmoothedParam, EnvelopeFollower, ParameterInfo, ParamDescriptor, ParamUnit};
use libm::powf;

/// Noise gate states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GateState {
    /// Gate is closed (attenuating)
    Closed,
    /// Gate is opening (attack phase)
    Opening,
    /// Gate is fully open
    Open,
    /// Gate is in hold phase
    Holding,
    /// Gate is closing (release phase)
    Closing,
}

/// Noise gate effect.
///
/// Attenuates signals below a threshold with smooth attack/release transitions.
/// Includes a hold time to prevent rapid gate chatter.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Threshold | -80.0–0.0 dB | -40.0 |
/// | 1 | Attack | 0.1–50.0 ms | 1.0 |
/// | 2 | Release | 10.0–1000.0 ms | 100.0 |
/// | 3 | Hold | 0.0–500.0 ms | 50.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Gate;
/// use sonido_core::Effect;
///
/// let mut gate = Gate::new(44100.0);
/// gate.set_threshold_db(-40.0);
/// gate.set_attack_ms(1.0);
/// gate.set_release_ms(100.0);
/// gate.set_hold_ms(50.0);
///
/// let input = 0.1;
/// let output = gate.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Gate {
    envelope_follower: EnvelopeFollower,
    threshold: SmoothedParam,
    attack_ms: SmoothedParam,
    release_ms: SmoothedParam,
    hold_ms: SmoothedParam,

    /// Current gate state
    state: GateState,
    /// Current gain (0 = closed, 1 = open)
    gain: f32,
    /// Hold counter in samples
    hold_counter: u32,
    /// Attack increment per sample
    attack_inc: f32,
    /// Release decrement per sample
    release_dec: f32,

    sample_rate: f32,
}

impl Gate {
    /// Create a new noise gate.
    pub fn new(sample_rate: f32) -> Self {
        let mut envelope_follower = EnvelopeFollower::new(sample_rate);
        envelope_follower.set_attack_ms(0.1); // Fast envelope detection
        envelope_follower.set_release_ms(20.0);

        let mut gate = Self {
            envelope_follower,
            threshold: SmoothedParam::with_config(-40.0, sample_rate, 10.0),
            attack_ms: SmoothedParam::with_config(1.0, sample_rate, 10.0),
            release_ms: SmoothedParam::with_config(100.0, sample_rate, 10.0),
            hold_ms: SmoothedParam::with_config(50.0, sample_rate, 10.0),
            state: GateState::Closed,
            gain: 0.0,
            hold_counter: 0,
            attack_inc: 0.0,
            release_dec: 0.0,
            sample_rate,
        };
        gate.recalculate_rates();
        gate
    }

    /// Set threshold in dB (-80 to 0 dB).
    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.threshold.set_target(threshold_db.clamp(-80.0, 0.0));
    }

    /// Get current threshold in dB.
    pub fn threshold_db(&self) -> f32 {
        self.threshold.target()
    }

    /// Set attack time in ms (0.1-50 ms).
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.attack_ms.set_target(attack_ms.clamp(0.1, 50.0));
        self.recalculate_rates();
    }

    /// Get current attack time in ms.
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms.target()
    }

    /// Set release time in ms (10-1000 ms).
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.release_ms.set_target(release_ms.clamp(10.0, 1000.0));
        self.recalculate_rates();
    }

    /// Get current release time in ms.
    pub fn release_ms(&self) -> f32 {
        self.release_ms.target()
    }

    /// Set hold time in ms (0-500 ms).
    pub fn set_hold_ms(&mut self, hold_ms: f32) {
        self.hold_ms.set_target(hold_ms.clamp(0.0, 500.0));
    }

    /// Get current hold time in ms.
    pub fn hold_ms(&self) -> f32 {
        self.hold_ms.target()
    }

    /// Convert dB to linear amplitude.
    #[inline]
    fn db_to_linear(db: f32) -> f32 {
        powf(10.0, db / 20.0)
    }

    /// Recalculate attack and release rates based on current settings.
    fn recalculate_rates(&mut self) {
        let attack_samples = (self.attack_ms.target() / 1000.0) * self.sample_rate;
        let release_samples = (self.release_ms.target() / 1000.0) * self.sample_rate;

        self.attack_inc = if attack_samples > 0.0 { 1.0 / attack_samples } else { 1.0 };
        self.release_dec = if release_samples > 0.0 { 1.0 / release_samples } else { 1.0 };
    }
}

impl Effect for Gate {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let threshold_db = self.threshold.advance();
        let _attack = self.attack_ms.advance();
        let _release = self.release_ms.advance();
        let hold_ms = self.hold_ms.advance();

        // Get envelope level
        let envelope = self.envelope_follower.process(input);

        // Convert threshold to linear
        let threshold_linear = Self::db_to_linear(threshold_db);

        // Calculate hold samples
        let hold_samples = ((hold_ms / 1000.0) * self.sample_rate) as u32;

        // Determine if signal is above threshold
        let above_threshold = envelope > threshold_linear;

        // State machine for gate
        match self.state {
            GateState::Closed => {
                if above_threshold {
                    self.state = GateState::Opening;
                }
            }
            GateState::Opening => {
                self.gain += self.attack_inc;
                if self.gain >= 1.0 {
                    self.gain = 1.0;
                    self.state = GateState::Open;
                }
                if !above_threshold {
                    self.state = GateState::Closing;
                }
            }
            GateState::Open => {
                if !above_threshold {
                    self.hold_counter = hold_samples;
                    self.state = GateState::Holding;
                }
            }
            GateState::Holding => {
                if above_threshold {
                    self.state = GateState::Open;
                } else if self.hold_counter > 0 {
                    self.hold_counter -= 1;
                } else {
                    self.state = GateState::Closing;
                }
            }
            GateState::Closing => {
                self.gain -= self.release_dec;
                if self.gain <= 0.0 {
                    self.gain = 0.0;
                    self.state = GateState::Closed;
                }
                if above_threshold {
                    self.state = GateState::Opening;
                }
            }
        }

        input * self.gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // Linked stereo: detect envelope from both channels (max), apply same gate
        // Using max prevents the gate from opening/closing based on only one channel
        let threshold_db = self.threshold.advance();
        let _attack = self.attack_ms.advance();
        let _release = self.release_ms.advance();
        let hold_ms = self.hold_ms.advance();

        // Get envelope from the louder channel
        let sum = (left.abs() + right.abs()) * 0.5;
        let envelope = self.envelope_follower.process(sum);

        let threshold_linear = Self::db_to_linear(threshold_db);
        let hold_samples = ((hold_ms / 1000.0) * self.sample_rate) as u32;
        let above_threshold = envelope > threshold_linear;

        // State machine (same as mono)
        match self.state {
            GateState::Closed => {
                if above_threshold {
                    self.state = GateState::Opening;
                }
            }
            GateState::Opening => {
                self.gain += self.attack_inc;
                if self.gain >= 1.0 {
                    self.gain = 1.0;
                    self.state = GateState::Open;
                }
                if !above_threshold {
                    self.state = GateState::Closing;
                }
            }
            GateState::Open => {
                if !above_threshold {
                    self.hold_counter = hold_samples;
                    self.state = GateState::Holding;
                }
            }
            GateState::Holding => {
                if above_threshold {
                    self.state = GateState::Open;
                } else if self.hold_counter > 0 {
                    self.hold_counter -= 1;
                } else {
                    self.state = GateState::Closing;
                }
            }
            GateState::Closing => {
                self.gain -= self.release_dec;
                if self.gain <= 0.0 {
                    self.gain = 0.0;
                    self.state = GateState::Closed;
                }
                if above_threshold {
                    self.state = GateState::Opening;
                }
            }
        }

        (left * self.gain, right * self.gain)
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.envelope_follower.set_sample_rate(sample_rate);
        self.threshold.set_sample_rate(sample_rate);
        self.attack_ms.set_sample_rate(sample_rate);
        self.release_ms.set_sample_rate(sample_rate);
        self.hold_ms.set_sample_rate(sample_rate);
        self.recalculate_rates();
    }

    fn reset(&mut self) {
        self.envelope_follower.reset();
        self.threshold.snap_to_target();
        self.attack_ms.snap_to_target();
        self.release_ms.snap_to_target();
        self.hold_ms.snap_to_target();
        self.state = GateState::Closed;
        self.gain = 0.0;
        self.hold_counter = 0;
    }
}

impl ParameterInfo for Gate {
    fn param_count(&self) -> usize {
        4
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Threshold",
                short_name: "Thresh",
                unit: ParamUnit::Decibels,
                min: -80.0,
                max: 0.0,
                default: -40.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Attack",
                short_name: "Atk",
                unit: ParamUnit::Milliseconds,
                min: 0.1,
                max: 50.0,
                default: 1.0,
                step: 0.1,
            }),
            2 => Some(ParamDescriptor {
                name: "Release",
                short_name: "Rel",
                unit: ParamUnit::Milliseconds,
                min: 10.0,
                max: 1000.0,
                default: 100.0,
                step: 1.0,
            }),
            3 => Some(ParamDescriptor {
                name: "Hold",
                short_name: "Hold",
                unit: ParamUnit::Milliseconds,
                min: 0.0,
                max: 500.0,
                default: 50.0,
                step: 1.0,
            }),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.threshold.target(),
            1 => self.attack_ms.target(),
            2 => self.release_ms.target(),
            3 => self.hold_ms.target(),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_threshold_db(value),
            1 => self.set_attack_ms(value),
            2 => self.set_release_ms(value),
            3 => self.set_hold_ms(value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "std")]
    use std::vec::Vec;
    #[cfg(not(feature = "std"))]
    use alloc::vec::Vec;

    #[test]
    fn test_gate_basic() {
        let mut gate = Gate::new(44100.0);

        for _ in 0..1000 {
            let output = gate.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_gate_silence_below_threshold() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0); // -20 dB threshold
        gate.set_attack_ms(0.1);
        gate.set_release_ms(10.0);
        gate.set_hold_ms(0.0);

        // Process very quiet signal (below -20 dB, which is 0.1 linear)
        // -40 dB = 0.01 linear
        for _ in 0..1000 {
            gate.process(0.01);
        }

        // Gate should be closed
        let output = gate.process(0.01);
        assert!(output.abs() < 0.001, "Gate should attenuate signal below threshold, got {}", output);
    }

    #[test]
    fn test_gate_passes_above_threshold() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0);
        gate.set_attack_ms(0.1);

        // Process loud signal (above threshold)
        // -10 dB = 0.316 linear
        for _ in 0..1000 {
            gate.process(0.5);
        }

        // Gate should be open
        let output = gate.process(0.5);
        assert!(output.abs() > 0.4, "Gate should pass signal above threshold, got {}", output);
    }

    #[test]
    fn test_gate_hold() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0);
        gate.set_attack_ms(0.1);
        gate.set_release_ms(10.0);
        gate.set_hold_ms(100.0); // 100ms hold

        // Open the gate with loud signal
        for _ in 0..1000 {
            gate.process(0.5);
        }

        // Now feed quiet signal but within hold time
        // 100ms at 44100 Hz = 4410 samples
        let _output_during_hold = gate.process(0.01);

        // Should still be passing signal during hold
        // (gain should still be 1.0)
        assert!(gate.gain > 0.9, "Gate should hold open, gain = {}", gate.gain);
    }

    #[test]
    fn test_gate_parameters() {
        let mut gate = Gate::new(44100.0);

        assert_eq!(gate.param_count(), 4);

        // Test threshold parameter
        gate.set_param(0, -30.0);
        assert!((gate.get_param(0) - (-30.0)).abs() < 0.01);

        // Test attack parameter
        gate.set_param(1, 5.0);
        assert!((gate.get_param(1) - 5.0).abs() < 0.01);

        // Test release parameter
        gate.set_param(2, 200.0);
        assert!((gate.get_param(2) - 200.0).abs() < 0.01);

        // Test hold parameter
        gate.set_param(3, 100.0);
        assert!((gate.get_param(3) - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_gate_reset() {
        let mut gate = Gate::new(44100.0);

        // Open the gate
        for _ in 0..1000 {
            gate.process(0.5);
        }

        gate.reset();

        // After reset, gate should be closed
        assert_eq!(gate.state, GateState::Closed);
        assert_eq!(gate.gain, 0.0);
    }

    #[test]
    fn test_gate_attack_release_smoothing() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-40.0);
        gate.set_attack_ms(10.0);
        gate.set_release_ms(50.0);
        gate.set_hold_ms(0.0);

        // Start with quiet signal
        for _ in 0..100 {
            gate.process(0.001);
        }

        // Suddenly loud signal - capture attack
        let mut gains_attack = Vec::new();
        for _ in 0..500 {
            gate.process(0.5);
            gains_attack.push(gate.gain);
        }

        // Verify smooth attack (values should increase)
        for i in 1..gains_attack.len() {
            if gains_attack[i - 1] < 1.0 {
                assert!(gains_attack[i] >= gains_attack[i - 1],
                    "Attack should be monotonically increasing");
            }
        }

        // Back to quiet signal - capture release
        let mut gains_release = Vec::new();
        for _ in 0..3000 {
            gate.process(0.001);
            gains_release.push(gate.gain);
        }

        // Verify smooth release (values should decrease or stay same)
        for i in 1..gains_release.len() {
            if gains_release[i - 1] > 0.0 {
                assert!(gains_release[i] <= gains_release[i - 1] + 0.001,
                    "Release should be monotonically decreasing");
            }
        }
    }
}
