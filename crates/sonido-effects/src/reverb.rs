//! Algorithmic reverb effect.
//!
//! A Freeverb-style reverb using parallel comb filters and series allpass
//! filters. Suitable for room and hall simulations.

use sonido_core::{Effect, SmoothedParam, InterpolatedDelay, CombFilter, AllpassFilter};

/// Freeverb comb filter delay times (at 44.1kHz reference).
/// These are mutually prime to avoid resonances.
const COMB_TUNINGS_44K: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];

/// Freeverb allpass filter delay times (at 44.1kHz reference).
const ALLPASS_TUNINGS_44K: [usize; 4] = [556, 441, 341, 225];

/// Reference sample rate for tuning constants.
const REFERENCE_RATE: f32 = 44100.0;

/// Maximum pre-delay in milliseconds.
const MAX_PREDELAY_MS: f32 = 100.0;

/// Scale delay times from reference rate to target rate.
fn scale_to_rate(samples: usize, target_rate: f32) -> usize {
    ((samples as f32 * target_rate / REFERENCE_RATE).round() as usize).max(1)
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
            ReverbType::Room => (0.4, 0.5, 0.5, 10.0),
            ReverbType::Hall => (0.8, 0.8, 0.3, 25.0),
        }
    }
}

/// Algorithmic reverb effect.
///
/// Based on the Freeverb algorithm with 8 parallel comb filters and
/// 4 series allpass filters. Includes pre-delay and adjustable damping.
///
/// # Parameters
///
/// - `room_size`: 0.0-1.0, affects early reflection density
/// - `decay`: 0.0-1.0, controls reverb tail length
/// - `damping`: 0.0-1.0, high-frequency absorption (0=bright, 1=dark)
/// - `predelay`: 0-100ms, gap before reverb starts
/// - `mix`: 0.0-1.0, wet/dry balance
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
    // Freeverb structure
    combs: [CombFilter; 8],
    allpasses: [AllpassFilter; 4],

    // Pre-delay
    predelay_line: InterpolatedDelay,
    predelay_samples: SmoothedParam,

    // Smoothed parameters
    room_size: SmoothedParam,
    decay: SmoothedParam,
    damping: SmoothedParam,
    mix: SmoothedParam,

    // Configuration
    sample_rate: f32,
    reverb_type: ReverbType,

    // Cached values for comb filter updates
    cached_room: f32,
    cached_decay: f32,
    cached_damp: f32,
}

impl Reverb {
    /// Create a new reverb effect at the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        // Create comb filters with scaled delay times
        let combs = core::array::from_fn(|i| {
            let delay = scale_to_rate(COMB_TUNINGS_44K[i], sample_rate);
            CombFilter::new(delay)
        });

        // Create allpass filters with scaled delay times
        let allpasses = core::array::from_fn(|i| {
            let delay = scale_to_rate(ALLPASS_TUNINGS_44K[i], sample_rate);
            let mut ap = AllpassFilter::new(delay);
            ap.set_feedback(0.5);
            ap
        });

        // Pre-delay: up to 100ms
        let max_predelay = (MAX_PREDELAY_MS / 1000.0 * sample_rate).ceil() as usize;
        let predelay_line = InterpolatedDelay::new(max_predelay.max(1));

        // Default to Room preset
        let (room, decay, damp, predelay_ms) = ReverbType::Room.defaults();
        let predelay_samps = predelay_ms / 1000.0 * sample_rate;

        let mut reverb = Self {
            combs,
            allpasses,
            predelay_line,
            predelay_samples: SmoothedParam::with_config(predelay_samps, sample_rate, 50.0),
            room_size: SmoothedParam::with_config(room, sample_rate, 20.0),
            decay: SmoothedParam::with_config(decay, sample_rate, 20.0),
            damping: SmoothedParam::with_config(damp, sample_rate, 20.0),
            mix: SmoothedParam::with_config(0.5, sample_rate, 10.0),
            sample_rate,
            reverb_type: ReverbType::Room,
            cached_room: -1.0,
            cached_decay: -1.0,
            cached_damp: -1.0,
        };

        reverb.update_comb_params();
        reverb
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

        for comb in &mut self.combs {
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
        comb_sum *= 0.125; // Scale by 1/8

        // Process through series allpass filters
        let mut diffused = comb_sum;
        for allpass in &mut self.allpasses {
            diffused = allpass.process(diffused);
        }

        // Wet/dry mix
        let wet = diffused;
        input * (1.0 - mix) + wet * mix
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (inp, out) in input.iter().zip(output.iter_mut()) {
            *out = self.process(*inp);
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        // Recreate delay structures for new sample rate
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

        let max_predelay = (MAX_PREDELAY_MS / 1000.0 * sample_rate).ceil() as usize;
        self.predelay_line = InterpolatedDelay::new(max_predelay.max(1));

        // Update parameter sample rates
        self.room_size.set_sample_rate(sample_rate);
        self.decay.set_sample_rate(sample_rate);
        self.damping.set_sample_rate(sample_rate);
        self.predelay_samples.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);

        // Force parameter update
        self.cached_room = -1.0;
        self.update_comb_params();
    }

    fn reset(&mut self) {
        for comb in &mut self.combs {
            comb.clear();
        }
        for allpass in &mut self.allpasses {
            allpass.clear();
        }
        self.predelay_line.clear();

        self.room_size.snap_to_target();
        self.decay.snap_to_target();
        self.damping.snap_to_target();
        self.predelay_samples.snap_to_target();
        self.mix.snap_to_target();

        self.cached_room = -1.0;
        self.update_comb_params();
    }

    fn latency_samples(&self) -> usize {
        // Report pre-delay as latency
        self.predelay_samples.get() as usize
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
        assert!(late.abs() > 1e-6, "Reverb tail should persist, got {}", late);
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
        assert!(output.abs() < 1e-10, "Reset should clear state, got {}", output);
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
        assert!((dry_out - 0.5).abs() < 0.01, "Dry output should match input");

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
}
