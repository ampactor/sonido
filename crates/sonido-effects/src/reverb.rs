//! Algorithmic reverb effect.
//!
//! A Freeverb-style reverb using parallel comb filters and series allpass
//! filters. Suitable for room and hall simulations.

use libm::{ceilf, roundf};
use sonido_core::{
    AllpassFilter, CombFilter, Effect, InterpolatedDelay, ParamDescriptor, ParamUnit,
    ParameterInfo, SmoothedParam, wet_dry_mix, wet_dry_mix_stereo,
};

/// Freeverb comb filter delay times (at 44.1kHz reference).
/// These are mutually prime to avoid resonances.
const COMB_TUNINGS_44K: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];

/// Right channel comb filter delay times (slightly offset for stereo decorrelation).
const COMB_TUNINGS_44K_R: [usize; 8] = [1139, 1211, 1300, 1379, 1445, 1514, 1580, 1640];

/// Freeverb allpass filter delay times (at 44.1kHz reference).
const ALLPASS_TUNINGS_44K: [usize; 4] = [556, 441, 341, 225];

/// Right channel allpass filter delay times (slightly offset for stereo).
const ALLPASS_TUNINGS_44K_R: [usize; 4] = [579, 464, 364, 248];

/// Reference sample rate for tuning constants.
const REFERENCE_RATE: f32 = 44100.0;

/// Maximum pre-delay in milliseconds.
const MAX_PREDELAY_MS: f32 = 100.0;

/// Scale delay times from reference rate to target rate.
fn scale_to_rate(samples: usize, target_rate: f32) -> usize {
    (roundf(samples as f32 * target_rate / REFERENCE_RATE) as usize).max(1)
}

/// Reverb type presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReverbType {
    /// Small room with short decay.
    #[default]
    Room,
    /// Large hall with long decay.
    Hall,
}

impl ReverbType {
    /// Get default parameters for this reverb type.
    ///
    /// Returns (room_size, decay, damping, predelay_ms)
    pub fn defaults(&self) -> (f32, f32, f32, f32) {
        match self {
            ReverbType::Room => (0.5, 0.5, 0.5, 10.0),
            ReverbType::Hall => (0.8, 0.8, 0.3, 25.0),
        }
    }
}

/// Algorithmic reverb effect.
///
/// Based on the Freeverb algorithm with 8 parallel comb filters and
/// 4 series allpass filters. Includes pre-delay and adjustable damping.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Room Size | 0–100% | 50.0 |
/// | 1 | Decay | 0–100% | 50.0 |
/// | 2 | Damping | 0–100% | 50.0 |
/// | 3 | Pre-Delay | 0.0–100.0 ms | 10.0 |
/// | 4 | Mix | 0–100% | 50.0 |
/// | 5 | Stereo Width | 0–100% | 100.0 |
/// | 6 | Reverb Type | 0–1 (Room, Hall) | 0 |
/// | 7 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::{Reverb, ReverbType};
/// use sonido_core::Effect;
///
/// let mut reverb = Reverb::new(48000.0);
/// reverb.set_room_size(0.7);
/// reverb.set_decay(0.8);
/// reverb.set_damping(0.3);
/// reverb.set_mix(0.5);
///
/// let output = reverb.process(0.5);
/// ```
pub struct Reverb {
    // Freeverb structure (left channel)
    combs: [CombFilter; 8],
    allpasses: [AllpassFilter; 4],

    // Right channel (separate tanks for true stereo)
    combs_r: [CombFilter; 8],
    allpasses_r: [AllpassFilter; 4],

    // Pre-delay (stereo)
    predelay_line: InterpolatedDelay,
    predelay_line_r: InterpolatedDelay,
    predelay_samples: SmoothedParam,

    // Smoothed parameters
    room_size: SmoothedParam,
    decay: SmoothedParam,
    damping: SmoothedParam,
    mix: SmoothedParam,

    /// Output level (linear gain)
    output_level: SmoothedParam,

    /// Stereo width (0 = mono, 1 = full stereo)
    stereo_width: f32,

    // Configuration
    sample_rate: f32,
    reverb_type: ReverbType,

    // Cached values for comb filter updates
    cached_room: f32,
    cached_decay: f32,
    cached_damp: f32,

    /// Feedback-adaptive comb output compensation.
    /// Reduces wet gain at high feedback to prevent peak ceiling violation.
    /// Computed in `update_comb_params()`.
    comb_compensation: f32,
}

impl Reverb {
    /// Create a new reverb effect at the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        // Create comb filters with scaled delay times (left channel)
        let combs = core::array::from_fn(|i| {
            let delay = scale_to_rate(COMB_TUNINGS_44K[i], sample_rate);
            CombFilter::new(delay)
        });

        // Create comb filters for right channel (slightly different tunings)
        let combs_r = core::array::from_fn(|i| {
            let delay = scale_to_rate(COMB_TUNINGS_44K_R[i], sample_rate);
            CombFilter::new(delay)
        });

        // Create allpass filters with scaled delay times (left channel)
        let allpasses = core::array::from_fn(|i| {
            let delay = scale_to_rate(ALLPASS_TUNINGS_44K[i], sample_rate);
            let mut ap = AllpassFilter::new(delay);
            ap.set_feedback(0.5);
            ap
        });

        // Create allpass filters for right channel
        let allpasses_r = core::array::from_fn(|i| {
            let delay = scale_to_rate(ALLPASS_TUNINGS_44K_R[i], sample_rate);
            let mut ap = AllpassFilter::new(delay);
            ap.set_feedback(0.5);
            ap
        });

        // Pre-delay: up to 100ms (stereo)
        let max_predelay = ceilf(MAX_PREDELAY_MS / 1000.0 * sample_rate) as usize;
        let predelay_line = InterpolatedDelay::new(max_predelay.max(1));
        let predelay_line_r = InterpolatedDelay::new(max_predelay.max(1));

        // Default to Room preset
        let (room, decay, damp, predelay_ms) = ReverbType::Room.defaults();
        let predelay_samps = predelay_ms / 1000.0 * sample_rate;

        let mut reverb = Self {
            combs,
            allpasses,
            combs_r,
            allpasses_r,
            predelay_line,
            predelay_line_r,
            predelay_samples: SmoothedParam::interpolated(predelay_samps, sample_rate),
            room_size: SmoothedParam::slow(room, sample_rate),
            decay: SmoothedParam::slow(decay, sample_rate),
            damping: SmoothedParam::slow(damp, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            stereo_width: 1.0,
            sample_rate,
            reverb_type: ReverbType::Room,
            cached_room: -1.0,
            cached_decay: -1.0,
            cached_damp: -1.0,
            comb_compensation: 1.0,
        };

        reverb.update_comb_params();
        reverb
    }

    /// Set stereo width (0 = mono, 1 = full stereo).
    pub fn set_stereo_width(&mut self, width: f32) {
        self.stereo_width = width.clamp(0.0, 1.0);
    }

    /// Get current stereo width.
    pub fn stereo_width(&self) -> f32 {
        self.stereo_width
    }

    /// Set the room size (0.0 to 1.0).
    ///
    /// Higher values create denser early reflections.
    pub fn set_room_size(&mut self, size: f32) {
        self.room_size.set_target(size.clamp(0.0, 1.0));
    }

    /// Get the current room size.
    pub fn room_size(&self) -> f32 {
        self.room_size.target()
    }

    /// Set the decay time (0.0 to 1.0).
    ///
    /// Higher values create longer reverb tails.
    pub fn set_decay(&mut self, decay: f32) {
        self.decay.set_target(decay.clamp(0.0, 1.0));
    }

    /// Get the current decay value.
    pub fn decay(&self) -> f32 {
        self.decay.target()
    }

    /// Set the damping amount (0.0 to 1.0).
    ///
    /// - 0.0 = bright (no HF absorption)
    /// - 1.0 = dark (high HF absorption)
    pub fn set_damping(&mut self, damping: f32) {
        self.damping.set_target(damping.clamp(0.0, 1.0));
    }

    /// Get the current damping value.
    pub fn damping(&self) -> f32 {
        self.damping.target()
    }

    /// Set the pre-delay time in milliseconds (0 to 100ms).
    pub fn set_predelay_ms(&mut self, ms: f32) {
        let ms_clamped = ms.clamp(0.0, MAX_PREDELAY_MS);
        let samples = ms_clamped / 1000.0 * self.sample_rate;
        self.predelay_samples.set_target(samples);
    }

    /// Get the current pre-delay in milliseconds.
    pub fn predelay_ms(&self) -> f32 {
        self.predelay_samples.target() / self.sample_rate * 1000.0
    }

    /// Set the wet/dry mix (0.0 to 1.0).
    ///
    /// - 0.0 = fully dry (no reverb)
    /// - 1.0 = fully wet (only reverb)
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Get the current mix value.
    pub fn mix(&self) -> f32 {
        self.mix.target()
    }

    /// Set the reverb type preset.
    ///
    /// This updates room_size, decay, damping, and predelay to preset values.
    pub fn set_reverb_type(&mut self, reverb_type: ReverbType) {
        self.reverb_type = reverb_type;
        let (room, decay, damp, predelay_ms) = reverb_type.defaults();
        self.set_room_size(room);
        self.set_decay(decay);
        self.set_damping(damp);
        self.set_predelay_ms(predelay_ms);
    }

    /// Get the current reverb type.
    pub fn reverb_type(&self) -> ReverbType {
        self.reverb_type
    }

    /// Update comb filter parameters based on room/decay/damping.
    fn update_comb_params(&mut self) {
        let room = self.room_size.get();
        let decay = self.decay.get();
        let damp = self.damping.get();

        // Only update if parameters changed significantly
        if (room - self.cached_room).abs() < 0.001
            && (decay - self.cached_decay).abs() < 0.001
            && (damp - self.cached_damp).abs() < 0.001
        {
            return;
        }

        self.cached_room = room;
        self.cached_decay = decay;
        self.cached_damp = damp;

        // Freeverb feedback calculation:
        // scaled_room = 0.28 + room * 0.7  (range: 0.28 to 0.98)
        // feedback = scaled_room + decay * (0.98 - scaled_room)
        let scaled_room = 0.28 + room * 0.7;
        let feedback = scaled_room + decay * (0.98 - scaled_room);

        // Reverb uses moderate compensation: parallel comb averaging (÷8)
        // provides ~18 dB headroom, so exact (1-fb) is too aggressive.
        self.comb_compensation = libm::sqrtf((1.0 - feedback).max(0.01));

        for comb in &mut self.combs {
            comb.set_feedback(feedback);
            comb.set_damp(damp);
        }
        // Update right channel combs with same parameters
        for comb in &mut self.combs_r {
            comb.set_feedback(feedback);
            comb.set_damp(damp);
        }
    }
}

impl Effect for Reverb {
    fn process(&mut self, input: f32) -> f32 {
        // Advance smoothed parameters
        self.room_size.advance();
        self.decay.advance();
        self.damping.advance();
        let predelay = self.predelay_samples.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Update comb filter coefficients if needed
        self.update_comb_params();

        // Apply pre-delay
        let predelayed = if predelay > 0.5 {
            self.predelay_line.read_write(input, predelay)
        } else {
            self.predelay_line.write(input);
            input
        };

        // Process through parallel comb filters
        let mut comb_sum = 0.0f32;
        for comb in &mut self.combs {
            comb_sum += comb.process(predelayed);
        }
        comb_sum *= 0.125 * self.comb_compensation;

        // Process through series allpass filters
        let mut diffused = comb_sum;
        for allpass in &mut self.allpasses {
            diffused = allpass.process(diffused);
        }

        wet_dry_mix(input, diffused, mix) * output_gain
    }

    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // True stereo: separate reverb tanks for L/R with stereo width control
        self.room_size.advance();
        self.decay.advance();
        self.damping.advance();
        let predelay = self.predelay_samples.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        self.update_comb_params();

        // Apply pre-delay (stereo)
        let predelayed_l = if predelay > 0.5 {
            self.predelay_line.read_write(left, predelay)
        } else {
            self.predelay_line.write(left);
            left
        };

        let predelayed_r = if predelay > 0.5 {
            self.predelay_line_r.read_write(right, predelay)
        } else {
            self.predelay_line_r.write(right);
            right
        };

        // Process left channel through its tank
        let mut comb_sum_l = 0.0f32;
        for comb in &mut self.combs {
            comb_sum_l += comb.process(predelayed_l);
        }
        comb_sum_l *= 0.125 * self.comb_compensation;

        let mut diffused_l = comb_sum_l;
        for allpass in &mut self.allpasses {
            diffused_l = allpass.process(diffused_l);
        }

        // Process right channel through its tank
        let mut comb_sum_r = 0.0f32;
        for comb in &mut self.combs_r {
            comb_sum_r += comb.process(predelayed_r);
        }
        comb_sum_r *= 0.125 * self.comb_compensation;

        let mut diffused_r = comb_sum_r;
        for allpass in &mut self.allpasses_r {
            diffused_r = allpass.process(diffused_r);
        }

        // Apply stereo width (0 = mono, 1 = full stereo)
        // At width=0, both channels get the average
        // At width=1, channels are fully independent
        let mid = (diffused_l + diffused_r) * 0.5;
        let side = (diffused_l - diffused_r) * 0.5;
        let wet_l = mid + side * self.stereo_width;
        let wet_r = mid - side * self.stereo_width;

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, wet_l, wet_r, mix);

        (out_l * output_gain, out_r * output_gain)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (inp, out) in input.iter().zip(output.iter_mut()) {
            *out = self.process(*inp);
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        // Recreate delay structures for new sample rate (left channel)
        self.combs = core::array::from_fn(|i| {
            let delay = scale_to_rate(COMB_TUNINGS_44K[i], sample_rate);
            CombFilter::new(delay)
        });

        self.allpasses = core::array::from_fn(|i| {
            let delay = scale_to_rate(ALLPASS_TUNINGS_44K[i], sample_rate);
            let mut ap = AllpassFilter::new(delay);
            ap.set_feedback(0.5);
            ap
        });

        // Recreate right channel structures
        self.combs_r = core::array::from_fn(|i| {
            let delay = scale_to_rate(COMB_TUNINGS_44K_R[i], sample_rate);
            CombFilter::new(delay)
        });

        self.allpasses_r = core::array::from_fn(|i| {
            let delay = scale_to_rate(ALLPASS_TUNINGS_44K_R[i], sample_rate);
            let mut ap = AllpassFilter::new(delay);
            ap.set_feedback(0.5);
            ap
        });

        let max_predelay = ceilf(MAX_PREDELAY_MS / 1000.0 * sample_rate) as usize;
        self.predelay_line = InterpolatedDelay::new(max_predelay.max(1));
        self.predelay_line_r = InterpolatedDelay::new(max_predelay.max(1));

        // Update parameter sample rates
        self.room_size.set_sample_rate(sample_rate);
        self.decay.set_sample_rate(sample_rate);
        self.damping.set_sample_rate(sample_rate);
        self.predelay_samples.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);

        // Force parameter update
        self.cached_room = -1.0;
        self.update_comb_params();
    }

    fn reset(&mut self) {
        for comb in &mut self.combs {
            comb.clear();
        }
        for comb in &mut self.combs_r {
            comb.clear();
        }
        for allpass in &mut self.allpasses {
            allpass.clear();
        }
        for allpass in &mut self.allpasses_r {
            allpass.clear();
        }
        self.predelay_line.clear();
        self.predelay_line_r.clear();

        self.room_size.snap_to_target();
        self.decay.snap_to_target();
        self.damping.snap_to_target();
        self.predelay_samples.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();

        self.cached_room = -1.0;
        self.update_comb_params();
    }

    fn latency_samples(&self) -> usize {
        // Report pre-delay as latency
        self.predelay_samples.get() as usize
    }
}

impl ParameterInfo for Reverb {
    fn param_count(&self) -> usize {
        8
    }

    fn param_info(&self, index: usize) -> Option<ParamDescriptor> {
        match index {
            0 => Some(ParamDescriptor {
                name: "Room Size",
                short_name: "Room",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 50.0,
                step: 1.0,
            }),
            1 => Some(ParamDescriptor {
                name: "Decay",
                short_name: "Decay",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 50.0,
                step: 1.0,
            }),
            2 => Some(ParamDescriptor {
                name: "Damping",
                short_name: "Damping",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 50.0,
                step: 1.0,
            }),
            3 => Some(ParamDescriptor {
                name: "Pre-Delay",
                short_name: "PreDly",
                unit: ParamUnit::Milliseconds,
                min: 0.0,
                max: 100.0,
                default: 10.0,
                step: 1.0,
            }),
            4 => Some(ParamDescriptor::mix()),
            5 => Some(ParamDescriptor {
                name: "Stereo Width",
                short_name: "Width",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 100.0,
                step: 1.0,
            }),
            6 => Some(ParamDescriptor {
                name: "Reverb Type",
                short_name: "Type",
                unit: ParamUnit::None,
                min: 0.0,
                max: 1.0,
                default: 0.0,
                step: 1.0,
            }),
            7 => Some(sonido_core::gain::output_param_descriptor()),
            _ => None,
        }
    }

    fn get_param(&self, index: usize) -> f32 {
        match index {
            0 => self.room_size() * 100.0,
            1 => self.decay() * 100.0,
            2 => self.damping() * 100.0,
            3 => self.predelay_ms(),
            4 => self.mix() * 100.0,
            5 => self.stereo_width * 100.0,
            6 => match self.reverb_type {
                ReverbType::Room => 0.0,
                ReverbType::Hall => 1.0,
            },
            7 => sonido_core::gain::output_level_db(&self.output_level),
            _ => 0.0,
        }
    }

    fn set_param(&mut self, index: usize, value: f32) {
        match index {
            0 => self.set_room_size(value / 100.0),
            1 => self.set_decay(value / 100.0),
            2 => self.set_damping(value / 100.0),
            3 => self.set_predelay_ms(value),
            4 => self.set_mix(value / 100.0),
            5 => self.set_stereo_width(value / 100.0),
            6 => match value as u8 {
                0 => self.set_reverb_type(ReverbType::Room),
                _ => self.set_reverb_type(ReverbType::Hall),
            },
            7 => sonido_core::gain::set_output_level_db(&mut self.output_level, value),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_basic_processing() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.reset();

        // Process impulse
        let _first = reverb.process(1.0);

        // Should have some output
        for _ in 0..10000 {
            let out = reverb.process(0.0);
            assert!(out.is_finite(), "Output should be finite");
        }
    }

    #[test]
    fn test_reverb_decay_tail() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_decay(0.9);
        reverb.set_mix(1.0);
        reverb.set_predelay_ms(0.0);
        reverb.reset();

        // Impulse
        reverb.process(1.0);

        // After 1 second, should still have some tail
        for _ in 0..48000 {
            reverb.process(0.0);
        }
        let late = reverb.process(0.0);
        assert!(
            late.abs() > 1e-6,
            "Reverb tail should persist, got {}",
            late
        );
    }

    #[test]
    fn test_reverb_dc_blocking() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.reset();

        // DC input should not accumulate indefinitely
        let mut output = 0.0;
        for _ in 0..100000 {
            output = reverb.process(1.0);
        }
        assert!(output.abs() < 10.0, "DC should not blow up: {}", output);
    }

    #[test]
    fn test_reverb_reset() {
        let mut reverb = Reverb::new(48000.0);

        for _ in 0..1000 {
            reverb.process(1.0);
        }

        reverb.reset();

        let output = reverb.process(0.0);
        assert!(
            output.abs() < 1e-10,
            "Reset should clear state, got {}",
            output
        );
    }

    #[test]
    fn test_reverb_parameter_ranges() {
        let mut reverb = Reverb::new(48000.0);

        // All parameters should clamp properly
        reverb.set_room_size(2.0);
        reverb.set_decay(-1.0);
        reverb.set_damping(1.5);
        reverb.set_mix(1.1);
        reverb.set_predelay_ms(200.0);

        assert!(reverb.room_size() <= 1.0);
        assert!(reverb.decay() >= 0.0);
        assert!(reverb.damping() <= 1.0);
        assert!(reverb.mix() <= 1.0);
        assert!(reverb.predelay_ms() <= MAX_PREDELAY_MS);
    }

    #[test]
    fn test_reverb_type_presets() {
        let mut reverb = Reverb::new(48000.0);

        reverb.set_reverb_type(ReverbType::Hall);
        assert_eq!(reverb.reverb_type(), ReverbType::Hall);
        assert!(reverb.decay() > 0.7); // Hall should have long decay

        reverb.set_reverb_type(ReverbType::Room);
        assert_eq!(reverb.reverb_type(), ReverbType::Room);
        assert!(reverb.decay() < 0.6); // Room should have shorter decay
    }

    #[test]
    fn test_reverb_mix() {
        let mut dry_reverb = Reverb::new(48000.0);
        dry_reverb.set_mix(0.0);
        dry_reverb.reset();

        let mut wet_reverb = Reverb::new(48000.0);
        wet_reverb.set_mix(1.0);
        wet_reverb.reset();

        // With mix=0, output should equal input
        let dry_out = dry_reverb.process(0.5);
        assert!(
            (dry_out - 0.5).abs() < 0.01,
            "Dry output should match input"
        );

        // With mix=1, output should be different (reverb signal)
        let wet_out = wet_reverb.process(0.5);
        // First sample won't have reverb yet, but should still be different
        // due to allpass processing
        assert!(wet_out.is_finite());
    }

    #[test]
    fn test_reverb_predelay() {
        // Test that predelay parameter can be set
        let mut reverb = Reverb::new(48000.0);
        reverb.set_predelay_ms(50.0);
        assert!((reverb.predelay_ms() - 50.0).abs() < 0.1);

        reverb.set_predelay_ms(0.0);
        assert!((reverb.predelay_ms() - 0.0).abs() < 0.1);

        // Verify clamping
        reverb.set_predelay_ms(200.0); // Max is 100ms
        assert!(reverb.predelay_ms() <= 100.0);
    }

    #[test]
    fn test_reverb_stereo_processing() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.reset();

        // Process stereo impulse
        let (l, r) = reverb.process_stereo(1.0, 1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());

        // Process more samples
        for _ in 0..10000 {
            let (l, r) = reverb.process_stereo(0.0, 0.0);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_reverb_is_true_stereo() {
        let reverb = Reverb::new(48000.0);
        assert!(reverb.is_true_stereo());
    }

    #[test]
    fn test_reverb_stereo_width() {
        let mut reverb = Reverb::new(48000.0);

        reverb.set_stereo_width(0.5);
        assert!((reverb.stereo_width() - 0.5).abs() < 0.01);

        reverb.set_stereo_width(0.0);
        assert!((reverb.stereo_width() - 0.0).abs() < 0.01);

        reverb.set_stereo_width(1.0);
        assert!((reverb.stereo_width() - 1.0).abs() < 0.01);

        // Clamping
        reverb.set_stereo_width(2.0);
        assert!(reverb.stereo_width() <= 1.0);
    }

    #[test]
    fn test_reverb_stereo_decorrelation() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.set_decay(0.8);
        reverb.set_stereo_width(1.0);
        reverb.reset();

        // Process stereo impulse with DIFFERENT values to each channel
        // This will excite the different tanks with different signals
        reverb.process_stereo(1.0, 0.5);

        // Let the reverb develop for a bit
        for _ in 0..5000 {
            reverb.process_stereo(0.0, 0.0);
        }

        // Now collect samples and check decorrelation
        let mut diff_count = 0;
        for _ in 0..1000 {
            let (l, r) = reverb.process_stereo(0.0, 0.0);
            // Check if L and R are different (accounting for stereo width effect)
            if (l - r).abs() > 0.0001 {
                diff_count += 1;
            }
        }

        // With true stereo (different tanks) and different inputs, L and R should differ
        assert!(
            diff_count > 100,
            "L and R should be decorrelated, but only {} samples differed",
            diff_count
        );
    }

    #[test]
    fn test_no_denormals_after_silence() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_decay(0.9);
        reverb.set_mix(1.0);
        reverb.reset();

        // Feed signal for 1000 samples to fill the reverb tanks
        for _ in 0..1000 {
            reverb.process(0.5);
        }

        // Feed silence for 200k samples -- reverb tail should decay to exact
        // zero, not linger as denormal values (which cause severe CPU penalty)
        for i in 0..200_000 {
            let out = reverb.process(0.0);
            assert!(
                out == 0.0 || out.abs() > f32::MIN_POSITIVE,
                "Denormal detected at sample {}: {:.2e} (below f32::MIN_POSITIVE {:.2e})",
                i,
                out,
                f32::MIN_POSITIVE
            );
        }
    }

    #[test]
    fn test_no_denormals_stereo_after_silence() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_decay(0.9);
        reverb.set_mix(1.0);
        reverb.reset();

        // Feed stereo signal
        for _ in 0..1000 {
            reverb.process_stereo(0.5, 0.5);
        }

        // Feed silence and check both channels
        for i in 0..200_000 {
            let (l, r) = reverb.process_stereo(0.0, 0.0);
            assert!(
                l == 0.0 || l.abs() > f32::MIN_POSITIVE,
                "Left denormal detected at sample {}: {:.2e} (below f32::MIN_POSITIVE {:.2e})",
                i,
                l,
                f32::MIN_POSITIVE
            );
            assert!(
                r == 0.0 || r.abs() > f32::MIN_POSITIVE,
                "Right denormal detected at sample {}: {:.2e} (below f32::MIN_POSITIVE {:.2e})",
                i,
                r,
                f32::MIN_POSITIVE
            );
        }
    }
}
