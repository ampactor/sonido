//! Noise gate effect for silencing signals below a threshold.
//!
//! A gate attenuates signals that fall below a threshold level,
//! useful for removing noise, bleed, or unwanted quiet sounds.
//!
//! # Features
//!
//! - **Exponential attack/release curves**: One-pole smoothing for natural-sounding
//!   transitions. Coefficient: `exp(-1 / (time_ms * sample_rate / 1000))`.
//! - **Sidechain highpass filter**: Biquad HPF on the detection path removes low-frequency
//!   content (rumble, proximity effect) that would otherwise keep the gate open.
//! - **Range/floor parameter**: Sets the minimum gain when the gate is closed, allowing
//!   attenuation instead of full silence (useful for natural drum gating).
//! - **Hysteresis**: Opens at threshold, closes at `threshold - hysteresis_db` to prevent
//!   rapid cycling (chatter) near the threshold boundary.
//!
//! # References
//!
//! - Giannoulis et al., "Digital Dynamic Range Compressor Design" (2012) — exponential
//!   ballistics and hysteresis design.
//! - Zolzer, "DAFX" (2011), Ch. 4 — gate design with hold time.

use sonido_core::{
    Biquad, Effect, EnvelopeFollower, ParamDescriptor, ParamId, ParamScale, ParamUnit,
    SmoothedParam, fast_db_to_linear, highpass_coefficients, impl_params, math::db_to_linear,
};

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
/// Attenuates signals below a threshold with exponential attack/release transitions.
/// Includes hold time, hysteresis, sidechain HPF, and adjustable range (floor).
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Threshold | -80.0–0.0 dB | -40.0 |
/// | 1 | Attack | 0.1–50.0 ms | 1.0 |
/// | 2 | Release | 10.0–1000.0 ms | 100.0 |
/// | 3 | Hold | 0.0–500.0 ms | 50.0 |
/// | 4 | Range | -80.0–0.0 dB | -80.0 |
/// | 5 | Hysteresis | 0.0–12.0 dB | 3.0 |
/// | 6 | SC HPF Freq | 20.0–500.0 Hz | 80.0 |
/// | 7 | Output | -20.0–20.0 dB | 0.0 |
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
/// gate.set_range_db(-60.0);       // attenuate to -60 dB instead of silence
/// gate.set_hysteresis_db(3.0);    // 3 dB hysteresis band
/// gate.set_sidechain_freq(80.0);  // HPF at 80 Hz on sidechain
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
    /// Current gain (floor..1.0)
    gain: f32,
    /// Hold counter in samples
    hold_counter: u32,
    /// Attack coefficient for exponential curve (one-pole)
    attack_coeff: f32,
    /// Release coefficient for exponential curve (one-pole)
    release_coeff: f32,
    /// Output level with smoothing
    output_level: SmoothedParam,
    /// Cached linear threshold (recomputed only while threshold param is smoothing)
    cached_threshold_linear: f32,

    /// Range (floor) in dB: minimum gain when gate is closed
    range_db: SmoothedParam,
    /// Cached linear floor gain
    cached_floor_linear: f32,
    /// Hysteresis in dB: gate opens at threshold, closes at threshold - hysteresis
    hysteresis_db: SmoothedParam,
    /// Sidechain highpass filter (removes low-frequency content from detection path)
    sidechain_hpf: Biquad,
    /// Sidechain HPF cutoff frequency in Hz
    sidechain_freq_hz: f32,

    sample_rate: f32,
}

impl Gate {
    /// Create a new noise gate.
    ///
    /// Initializes with default parameters: threshold -40 dB, attack 1 ms,
    /// release 100 ms, hold 50 ms, range -80 dB, hysteresis 3 dB,
    /// sidechain HPF at 80 Hz.
    pub fn new(sample_rate: f32) -> Self {
        let mut envelope_follower = EnvelopeFollower::new(sample_rate);
        envelope_follower.set_attack_ms(0.1); // Fast envelope detection
        envelope_follower.set_release_ms(20.0);

        let mut sidechain_hpf = Biquad::new();
        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(80.0, 0.707, sample_rate);
        sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);

        let mut gate = Self {
            envelope_follower,
            threshold: SmoothedParam::standard(-40.0, sample_rate),
            attack_ms: SmoothedParam::standard(1.0, sample_rate),
            release_ms: SmoothedParam::standard(100.0, sample_rate),
            hold_ms: SmoothedParam::standard(50.0, sample_rate),
            state: GateState::Closed,
            gain: 0.0,
            hold_counter: 0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            output_level: sonido_core::gain::output_level_param(sample_rate),
            cached_threshold_linear: fast_db_to_linear(-40.0),
            range_db: SmoothedParam::standard(-80.0, sample_rate),
            cached_floor_linear: db_to_linear(-80.0),
            hysteresis_db: SmoothedParam::standard(3.0, sample_rate),
            sidechain_hpf,
            sidechain_freq_hz: 80.0,
            sample_rate,
        };
        gate.recalculate_coefficients();
        gate.gain = gate.cached_floor_linear;
        gate
    }

    /// Set threshold in dB.
    ///
    /// Range: -80.0 to 0.0 dB. The gate opens when the sidechain signal
    /// exceeds this level.
    pub fn set_threshold_db(&mut self, threshold_db: f32) {
        self.threshold.set_target(threshold_db.clamp(-80.0, 0.0));
    }

    /// Get current threshold in dB.
    pub fn threshold_db(&self) -> f32 {
        self.threshold.target()
    }

    /// Set attack time in ms.
    ///
    /// Range: 0.1 to 50.0 ms. Controls how quickly the gate opens.
    /// Uses an exponential (one-pole) curve for natural-sounding attack.
    pub fn set_attack_ms(&mut self, attack_ms: f32) {
        self.attack_ms.set_target(attack_ms.clamp(0.1, 50.0));
        self.recalculate_coefficients();
    }

    /// Get current attack time in ms.
    pub fn attack_ms(&self) -> f32 {
        self.attack_ms.target()
    }

    /// Set release time in ms.
    ///
    /// Range: 10.0 to 1000.0 ms. Controls how quickly the gate closes.
    /// Uses an exponential (one-pole) curve for natural-sounding release.
    pub fn set_release_ms(&mut self, release_ms: f32) {
        self.release_ms.set_target(release_ms.clamp(10.0, 1000.0));
        self.recalculate_coefficients();
    }

    /// Get current release time in ms.
    pub fn release_ms(&self) -> f32 {
        self.release_ms.target()
    }

    /// Set hold time in ms.
    ///
    /// Range: 0.0 to 500.0 ms. The gate stays open for this duration
    /// after the signal drops below the close threshold, preventing chatter.
    pub fn set_hold_ms(&mut self, hold_ms: f32) {
        self.hold_ms.set_target(hold_ms.clamp(0.0, 500.0));
    }

    /// Get current hold time in ms.
    pub fn hold_ms(&self) -> f32 {
        self.hold_ms.target()
    }

    /// Set range (floor) in dB.
    ///
    /// Range: -80.0 to 0.0 dB (default -80.0). Sets the minimum gain when
    /// the gate is closed. At -80 dB the gate is effectively silent; at -20 dB
    /// the signal is attenuated but still audible (useful for natural drum gating).
    pub fn set_range_db(&mut self, range_db: f32) {
        self.range_db.set_target(range_db.clamp(-80.0, 0.0));
    }

    /// Get current range (floor) in dB.
    pub fn range_db(&self) -> f32 {
        self.range_db.target()
    }

    /// Set hysteresis in dB.
    ///
    /// Range: 0.0 to 12.0 dB (default 3.0). The gate opens at threshold_db
    /// and closes at threshold_db - hysteresis_db. This prevents rapid
    /// open/close cycling when the signal hovers near the threshold.
    pub fn set_hysteresis_db(&mut self, hysteresis_db: f32) {
        self.hysteresis_db
            .set_target(hysteresis_db.clamp(0.0, 12.0));
    }

    /// Get current hysteresis in dB.
    pub fn hysteresis_db(&self) -> f32 {
        self.hysteresis_db.target()
    }

    /// Set sidechain highpass filter frequency in Hz.
    ///
    /// Range: 20.0 to 500.0 Hz (default 80.0). Filters the detection path
    /// to prevent low-frequency content (rumble, proximity effect) from
    /// keeping the gate open.
    pub fn set_sidechain_freq(&mut self, freq_hz: f32) {
        let freq = freq_hz.clamp(20.0, 500.0);
        self.sidechain_freq_hz = freq;
        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(freq, 0.707, self.sample_rate);
        self.sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);
    }

    /// Get current sidechain HPF frequency in Hz.
    pub fn sidechain_freq(&self) -> f32 {
        self.sidechain_freq_hz
    }

    /// Recalculate exponential attack and release coefficients.
    ///
    /// Uses one-pole filter coefficient: `exp(-1 / (time_ms * sample_rate / 1000))`.
    /// This produces a natural exponential curve that reaches ~63% of target
    /// after one time constant.
    fn recalculate_coefficients(&mut self) {
        let attack_samples = self.attack_ms.target() / 1000.0 * self.sample_rate;
        self.attack_coeff = if attack_samples > 0.0 {
            libm::expf(-1.0 / attack_samples)
        } else {
            0.0 // instant attack
        };

        let release_samples = self.release_ms.target() / 1000.0 * self.sample_rate;
        self.release_coeff = if release_samples > 0.0 {
            libm::expf(-1.0 / release_samples)
        } else {
            0.0 // instant release
        };
    }

    /// Advance the gate state machine and update gain.
    ///
    /// Shared logic for mono and stereo processing. Takes the envelope level
    /// from the (already HPF-filtered) sidechain and updates the gate state,
    /// hold counter, and gain value.
    ///
    /// Uses exponential (one-pole) curves for attack and release:
    /// - Attack: `gain = 1.0 + coeff * (gain - 1.0)` (approaches 1.0 from below)
    /// - Release: `gain = floor + coeff * (gain - floor)` (approaches floor from above)
    fn advance_gate_state(&mut self, envelope: f32, hold_samples: u32, floor: f32) {
        let threshold_linear = self.cached_threshold_linear;
        let hysteresis = self.hysteresis_db.advance();

        // Open threshold = threshold_db, close threshold = threshold_db - hysteresis
        let close_threshold = fast_db_to_linear(self.threshold.target() - hysteresis);

        let above_open = envelope > threshold_linear;
        let above_close = envelope > close_threshold;

        match self.state {
            GateState::Closed => {
                // Track floor so range changes apply while gate is closed
                self.gain = floor;
                if above_open {
                    self.state = GateState::Opening;
                }
            }
            GateState::Opening => {
                // Exponential approach toward 1.0
                self.gain = 1.0 + self.attack_coeff * (self.gain - 1.0);
                if self.gain >= 0.999 {
                    self.gain = 1.0;
                    self.state = GateState::Open;
                }
                if !above_close {
                    self.state = GateState::Closing;
                }
            }
            GateState::Open => {
                if !above_close {
                    self.hold_counter = hold_samples;
                    self.state = GateState::Holding;
                }
            }
            GateState::Holding => {
                if above_close {
                    self.state = GateState::Open;
                } else if self.hold_counter > 0 {
                    self.hold_counter -= 1;
                } else {
                    self.state = GateState::Closing;
                }
            }
            GateState::Closing => {
                // Exponential approach toward floor
                self.gain = floor + self.release_coeff * (self.gain - floor);
                if self.gain <= floor + 0.001 {
                    self.gain = floor;
                    self.state = GateState::Closed;
                }
                if above_open {
                    self.state = GateState::Opening;
                }
            }
        }
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Gate {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let threshold_db = self.threshold.advance();
        let _attack = self.attack_ms.advance();
        let _release = self.release_ms.advance();
        let hold_ms = self.hold_ms.advance();
        let range = self.range_db.advance();

        // Recompute cached values while smoothed params are moving
        if !self.threshold.is_settled() {
            self.cached_threshold_linear = fast_db_to_linear(threshold_db);
        }
        if !self.range_db.is_settled() {
            self.cached_floor_linear = db_to_linear(range);
        }
        let floor = self.cached_floor_linear;

        // Sidechain path: HPF then envelope
        let sc_filtered = self.sidechain_hpf.process(input);
        let envelope = self.envelope_follower.process(sc_filtered);

        let hold_samples = ((hold_ms / 1000.0) * self.sample_rate) as u32;

        self.advance_gate_state(envelope, hold_samples, floor);

        let output_gain = self.output_level.advance();
        input * self.gain * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let threshold_db = self.threshold.advance();
        let _attack = self.attack_ms.advance();
        let _release = self.release_ms.advance();
        let hold_ms = self.hold_ms.advance();
        let range = self.range_db.advance();

        // Recompute cached values while smoothed params are moving
        if !self.threshold.is_settled() {
            self.cached_threshold_linear = fast_db_to_linear(threshold_db);
        }
        if !self.range_db.is_settled() {
            self.cached_floor_linear = db_to_linear(range);
        }
        let floor = self.cached_floor_linear;

        // Linked stereo: average both channels, HPF, then envelope
        let sum = (libm::fabsf(left) + libm::fabsf(right)) * 0.5;
        let sc_filtered = self.sidechain_hpf.process(sum);
        let envelope = self.envelope_follower.process(sc_filtered);

        let hold_samples = ((hold_ms / 1000.0) * self.sample_rate) as u32;

        self.advance_gate_state(envelope, hold_samples, floor);

        let output_gain = self.output_level.advance();
        (
            left * self.gain * output_gain,
            right * self.gain * output_gain,
        )
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.envelope_follower.set_sample_rate(sample_rate);
        self.threshold.set_sample_rate(sample_rate);
        self.attack_ms.set_sample_rate(sample_rate);
        self.release_ms.set_sample_rate(sample_rate);
        self.hold_ms.set_sample_rate(sample_rate);
        self.range_db.set_sample_rate(sample_rate);
        self.hysteresis_db.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.recalculate_coefficients();
        // Recompute sidechain HPF for new sample rate
        let (b0, b1, b2, a0, a1, a2) =
            highpass_coefficients(self.sidechain_freq_hz, 0.707, sample_rate);
        self.sidechain_hpf.set_coefficients(b0, b1, b2, a0, a1, a2);
    }

    fn reset(&mut self) {
        self.envelope_follower.reset();
        self.threshold.snap_to_target();
        self.attack_ms.snap_to_target();
        self.release_ms.snap_to_target();
        self.hold_ms.snap_to_target();
        self.range_db.snap_to_target();
        self.hysteresis_db.snap_to_target();
        self.output_level.snap_to_target();
        self.cached_threshold_linear = fast_db_to_linear(self.threshold.target());
        self.cached_floor_linear = db_to_linear(self.range_db.target());
        self.sidechain_hpf.clear();
        self.state = GateState::Closed;
        self.gain = self.cached_floor_linear;
        self.hold_counter = 0;
    }
}

impl_params! {
    Gate, this {
        [0] ParamDescriptor {
                name: "Threshold",
                short_name: "Thresh",
                unit: ParamUnit::Decibels,
                min: -80.0,
                max: 0.0,
                default: -40.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(400), "gate_thresh"),
            get: this.threshold.target(),
            set: |v| this.set_threshold_db(v);

        [1] ParamDescriptor {
                name: "Attack",
                short_name: "Atk",
                unit: ParamUnit::Milliseconds,
                min: 0.1,
                max: 50.0,
                default: 1.0,
                step: 0.1,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(401), "gate_attack"),
            get: this.attack_ms.target(),
            set: |v| this.set_attack_ms(v);

        [2] ParamDescriptor::time_ms("Release", "Rel", 10.0, 1000.0, 100.0)
                .with_id(ParamId(402), "gate_release"),
            get: this.release_ms.target(),
            set: |v| this.set_release_ms(v);

        [3] ParamDescriptor::time_ms("Hold", "Hold", 0.0, 500.0, 50.0)
                .with_id(ParamId(403), "gate_hold"),
            get: this.hold_ms.target(),
            set: |v| this.set_hold_ms(v);

        [4] ParamDescriptor {
                name: "Range",
                short_name: "Range",
                unit: ParamUnit::Decibels,
                min: -80.0,
                max: 0.0,
                default: -80.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(405), "gate_range"),
            get: this.range_db.target(),
            set: |v| this.set_range_db(v);

        [5] ParamDescriptor {
                name: "Hysteresis",
                short_name: "Hyst",
                unit: ParamUnit::Decibels,
                min: 0.0,
                max: 12.0,
                default: 3.0,
                step: 0.1,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(406), "gate_hysteresis"),
            get: this.hysteresis_db.target(),
            set: |v| this.set_hysteresis_db(v);

        [6] ParamDescriptor {
                name: "SC HPF Freq",
                short_name: "SC HPF",
                unit: ParamUnit::Hertz,
                min: 20.0,
                max: 500.0,
                default: 80.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(407), "gate_sc_hpf")
            .with_scale(ParamScale::Logarithmic),
            get: this.sidechain_freq_hz,
            set: |v| this.set_sidechain_freq(v);

        [7] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(404), "gate_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use super::*;
    use alloc::vec::Vec;
    use sonido_core::ParameterInfo;

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
        gate.set_threshold_db(-20.0);
        gate.set_attack_ms(0.1);
        gate.set_release_ms(10.0);
        gate.set_hold_ms(0.0);
        gate.set_hysteresis_db(0.0); // no hysteresis for simple test

        // Process very quiet signal (below -20 dB, which is 0.1 linear)
        // -40 dB = 0.01 linear
        for _ in 0..1000 {
            gate.process(0.01);
        }

        // Gate should be closed — output attenuated to floor
        let output = gate.process(0.01);
        assert!(
            libm::fabsf(output) < 0.001,
            "Gate should attenuate signal below threshold, got {}",
            output
        );
    }

    #[test]
    fn test_gate_passes_above_threshold() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0);
        gate.set_attack_ms(0.1);

        // Process loud signal (above threshold)
        for _ in 0..1000 {
            gate.process(0.5);
        }

        // Gate should be open
        let output = gate.process(0.5);
        assert!(
            libm::fabsf(output) > 0.4,
            "Gate should pass signal above threshold, got {}",
            output
        );
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
        let _output_during_hold = gate.process(0.01);

        // Should still be passing signal during hold
        assert!(
            gate.gain > 0.9,
            "Gate should hold open, gain = {}",
            gate.gain
        );
    }

    #[test]
    fn test_gate_parameters() {
        let mut gate = Gate::new(44100.0);

        assert_eq!(gate.param_count(), 8);

        // Test threshold parameter
        gate.set_param(0, -30.0);
        assert!(libm::fabsf(gate.get_param(0) - (-30.0)) < 0.01);

        // Test attack parameter
        gate.set_param(1, 5.0);
        assert!(libm::fabsf(gate.get_param(1) - 5.0) < 0.01);

        // Test release parameter
        gate.set_param(2, 200.0);
        assert!(libm::fabsf(gate.get_param(2) - 200.0) < 0.01);

        // Test hold parameter
        gate.set_param(3, 100.0);
        assert!(libm::fabsf(gate.get_param(3) - 100.0) < 0.01);

        // Test range parameter
        gate.set_param(4, -40.0);
        assert!(libm::fabsf(gate.get_param(4) - (-40.0)) < 0.01);

        // Test hysteresis parameter
        gate.set_param(5, 6.0);
        assert!(libm::fabsf(gate.get_param(5) - 6.0) < 0.01);

        // Test sidechain HPF freq parameter
        gate.set_param(6, 120.0);
        assert!(libm::fabsf(gate.get_param(6) - 120.0) < 0.01);
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
        // Gain should be at floor level
        assert!(
            libm::fabsf(gate.gain - gate.cached_floor_linear) < 0.001,
            "gain after reset should be floor, got {}",
            gate.gain
        );
    }

    #[test]
    fn test_gate_attack_release_smoothing() {
        let sr = 44100.0;
        let mut gate = Gate::new(sr);
        gate.set_threshold_db(-40.0);
        gate.set_attack_ms(10.0);
        gate.set_release_ms(50.0);
        gate.set_hold_ms(0.0);
        gate.set_hysteresis_db(0.0);
        gate.set_sidechain_freq(20.0); // minimize HPF

        // Use 1 kHz sine so HPF passes it cleanly (no DC filtering issues)
        let freq = 1000.0;

        // Start with quiet sine (below threshold)
        for i in 0..500 {
            let t = i as f32 / sr;
            let sample = 0.001 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);
            gate.process(sample);
        }

        // Loud sine — capture attack (exponential needs ~5 time constants
        // to reach 0.99; at 10ms attack, that's ~2205 samples at 44100 Hz)
        let mut gains_attack = Vec::new();
        for i in 0..3000 {
            let t = i as f32 / sr;
            let sample = 0.5 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);
            gate.process(sample);
            gains_attack.push(gate.gain);
        }

        // Verify smooth attack: gain should be monotonically non-decreasing
        for i in 1..gains_attack.len() {
            if gains_attack[i - 1] < 1.0 {
                assert!(
                    gains_attack[i] >= gains_attack[i - 1] - 1e-6,
                    "Attack should be monotonically increasing"
                );
            }
        }
        // Should have reached fully open
        assert!(
            *gains_attack.last().unwrap() > 0.99,
            "Gate should be fully open after attack"
        );

        // Back to silence — capture release
        let mut gains_release = Vec::new();
        for _ in 0..20000 {
            gate.process(0.0);
            gains_release.push(gate.gain);
        }

        // Verify overall trend: gain should end at floor
        let final_gain = *gains_release.last().unwrap();
        assert!(
            libm::fabsf(final_gain - gate.cached_floor_linear) < 0.01,
            "Release should reach floor, got {}",
            final_gain
        );

        // Verify no gain exceeds 1.0 during release
        for &g in &gains_release {
            assert!(
                g <= 1.001,
                "Gain should never exceed 1.0 during release, got {}",
                g
            );
        }
    }

    #[test]
    fn test_gate_exponential_curves() {
        // Verify attack/release are exponential, not linear.
        // Exponential curves change faster at the start and slow down.
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-40.0);
        gate.set_attack_ms(10.0);
        gate.set_hold_ms(0.0);
        gate.set_hysteresis_db(0.0);
        gate.set_range_db(-80.0);

        // Ensure gate is closed
        for _ in 0..2000 {
            gate.process(0.001);
        }

        // Open gate with loud signal, capture first 100 samples of attack
        let mut gains = Vec::new();
        for _ in 0..100 {
            gate.process(0.5);
            gains.push(gate.gain);
        }

        // Exponential: first half of gains should show larger increments than second half
        if gains.len() >= 4 {
            let early_delta = gains[1] - gains[0];
            // Find a later point where gain is still rising
            let mid = gains.len() / 2;
            let late_delta = gains[mid + 1] - gains[mid];
            // Early deltas should be larger (exponential approach slows down)
            assert!(
                early_delta > late_delta || gains[mid] > 0.99,
                "Curve should be exponential: early_delta={}, late_delta={}",
                early_delta,
                late_delta,
            );
        }
    }

    #[test]
    fn test_gate_range_floor() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0);
        gate.set_attack_ms(0.1);
        gate.set_release_ms(10.0);
        gate.set_hold_ms(0.0);
        gate.set_hysteresis_db(0.0);
        gate.set_range_db(-20.0); // -20 dB floor = 0.1 linear

        // Process quiet signal to close gate
        for _ in 0..5000 {
            gate.process(0.001);
        }

        // Gate closed — gain should be at floor, not zero
        let floor_linear = db_to_linear(-20.0);
        assert!(
            libm::fabsf(gate.gain - floor_linear) < 0.01,
            "Closed gate gain should be at floor ({}), got {}",
            floor_linear,
            gate.gain,
        );
    }

    #[test]
    fn test_gate_hysteresis() {
        let mut gate = Gate::new(44100.0);
        gate.set_threshold_db(-20.0); // opens at -20 dB = 0.1 linear
        gate.set_hysteresis_db(6.0); // closes at -26 dB ≈ 0.05 linear
        gate.set_attack_ms(0.1);
        gate.set_release_ms(10.0);
        gate.set_hold_ms(0.0);

        // Open the gate with loud signal
        for _ in 0..1000 {
            gate.process(0.5);
        }
        assert_eq!(gate.state, GateState::Open);

        // Feed signal between close threshold (-26 dB) and open threshold (-20 dB)
        // -23 dB ≈ 0.071 linear — above close threshold, should stay open
        for _ in 0..1000 {
            gate.process(0.071);
        }
        assert!(
            gate.state == GateState::Open || gate.state == GateState::Holding,
            "Gate should stay open within hysteresis band, state = {:?}",
            gate.state,
        );
    }

    #[test]
    fn test_gate_sidechain_hpf() {
        // The sidechain HPF should reject low frequencies from the detector.
        // A low-frequency signal should not open the gate as easily.
        let mut gate_no_hpf = Gate::new(44100.0);
        gate_no_hpf.set_threshold_db(-30.0);
        gate_no_hpf.set_sidechain_freq(20.0); // effectively bypass HPF
        gate_no_hpf.set_hysteresis_db(0.0);

        let mut gate_with_hpf = Gate::new(44100.0);
        gate_with_hpf.set_threshold_db(-30.0);
        gate_with_hpf.set_sidechain_freq(500.0); // aggressive HPF
        gate_with_hpf.set_hysteresis_db(0.0);

        // Feed low-frequency signal (50 Hz sine at moderate level)
        let freq = 50.0;
        let sr = 44100.0;
        for i in 0..4410 {
            let t = i as f32 / sr;
            let sample = 0.2 * libm::sinf(2.0 * core::f32::consts::PI * freq * t);
            gate_no_hpf.process(sample);
            gate_with_hpf.process(sample);
        }

        // The HPF gate should be harder to open with low-frequency content
        assert!(
            gate_with_hpf.gain < gate_no_hpf.gain,
            "HPF should reduce gate sensitivity to low frequencies: no_hpf={}, with_hpf={}",
            gate_no_hpf.gain,
            gate_with_hpf.gain,
        );
    }
}
