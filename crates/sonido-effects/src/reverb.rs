//! Algorithmic reverb effect using a Hadamard Feedback Delay Network.
//!
//! Combines early reflections (tapped delay), an 8×8 Hadamard FDN with
//! modulated delay lines and one-pole damping, and allpass diffusion.
//! Based on Jot & Chaigne's FDN topology with Freeverb-derived delay tunings.
//!
//! # Architecture
//!
//! ```text
//! input → predelay → [early reflections (tapped delay)]  → er_mix
//!                  → [8 FDN delays ↔ Hadamard feedback] → [allpass diffusion] → late_mix
//!                                                                      ↓
//!                                              stereo width → wet/dry → output
//! ```
//!
//! The FDN uses an 8×8 Hadamard matrix (implemented via fast Walsh–Hadamard
//! butterfly) to mix energy between delay lines in the feedback path. Each delay
//! line has sinusoidal LFO modulation (breaking metallic resonances) and a
//! one-pole lowpass for high-frequency damping. This produces a dense, smooth
//! late reverb tail with natural high-frequency rolloff.
//!
//! # References
//!
//! - Jot & Chaigne, "Digital Delay Networks for Designing Artificial
//!   Reverberators", AES Convention Paper 3030, 1991.
//! - Jon Dattorro, "Effect Design, Part 1: Reverberator and Other Filters",
//!   J. Audio Eng. Soc., Vol. 45, No. 9, 1997.
//! - Jezar, Freeverb — delay tunings and comb filter structure.

use libm::{ceilf, powf, roundf, sinf, sqrtf};
use sonido_core::{
    Effect, InterpolatedDelay, Interpolation, ModulatedAllpass, OnePole, ParamDescriptor, ParamId,
    ParamUnit, SmoothedParam, flush_denormal, wet_dry_mix, wet_dry_mix_stereo,
};

/// FDN delay tunings at 44.1 kHz reference (from Freeverb, mutually prime).
const FDN_TUNINGS_44K: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];

/// LFO modulation rates per FDN line (Hz). Different rates prevent correlation.
const FDN_MOD_RATES: [f32; 8] = [0.3, 0.4, 0.5, 0.7, 0.8, 1.0, 1.1, 1.3];

/// Modulation depth for FDN delay lines (ms).
const FDN_MOD_DEPTH_MS: f32 = 0.3;

/// Allpass diffusion delay times at 44.1 kHz reference.
const ALLPASS_TUNINGS_44K: [usize; 4] = [556, 441, 341, 225];

/// Right-channel allpass delay times (offset for stereo decorrelation).
const ALLPASS_TUNINGS_44K_R: [usize; 4] = [579, 464, 364, 248];

/// Early reflection tap positions at 44.1 kHz reference (primes, ~25 ms window).
/// Room-size scaling extends these up to ~80 ms for large rooms.
const ER_TAP_POSITIONS_44K: [usize; 14] = [
    131, 197, 263, 337, 401, 463, 541, 613, 677, 751, 811, 877, 941, 1009,
];

/// Number of early reflection taps.
const ER_TAP_COUNT: usize = 14;

/// Reference sample rate for tuning constants.
const REFERENCE_RATE: f32 = 44100.0;

/// Maximum pre-delay in milliseconds.
const MAX_PREDELAY_MS: f32 = 100.0;

/// Maximum early reflection delay in milliseconds.
const MAX_ER_MS: f32 = 80.0;

/// Hadamard scale factor: 1/sqrt(8) for energy preservation.
///
/// The unnormalized 8×8 Hadamard satisfies H·Hᵀ = 8I, so dividing by
/// sqrt(8) makes it orthogonal (energy-preserving).
const HADAMARD_SCALE: f32 = 0.353_553_39;

/// ER output normalization. Keeps per-channel ER level near unity for
/// sustained signals (7 taps/channel with 1/√k gains sum ≈ 3.3).
const ER_GAIN_SCALE: f32 = 0.3;

/// Allpass diffusion feedback coefficient.
const ALLPASS_FEEDBACK: f32 = 0.6;

/// Allpass modulation depth (ms). Subtle — enough to decorrelate.
const ALLPASS_MOD_DEPTH_MS: f32 = 0.15;

/// Scale delay samples from 44.1 kHz reference to target rate.
fn scale_to_rate(samples: usize, target_rate: f32) -> usize {
    (roundf(samples as f32 * target_rate / REFERENCE_RATE) as usize).max(1)
}

/// Compute ER tap gain for the given 0-based tap index.
///
/// Gains decrease as 1/√(i+1), giving a natural amplitude decay with
/// distance. Scaled by [`ER_GAIN_SCALE`] so 7 taps per channel sum to ≈ 1.
#[inline]
fn er_tap_gain(index: usize) -> f32 {
    ER_GAIN_SCALE / sqrtf((index + 1) as f32)
}

/// Convert damping parameter (0 = bright, 1 = dark) to lowpass cutoff Hz.
///
/// Uses a logarithmic mapping: 200 Hz at damping = 1.0, 20 kHz at damping = 0.0.
/// This gives perceptually uniform brightness control.
#[inline]
fn damping_to_hz(damping: f32) -> f32 {
    200.0 * powf(100.0, 1.0 - damping)
}

// ── Hadamard fast Walsh–Hadamard transform ──────────────────────────────

/// In-place butterfly on array elements at indices `i` and `j`.
///
/// (buf[i], buf[j]) → (buf[i]+buf[j], buf[i]−buf[j]).
#[inline]
fn butterfly_at(buf: &mut [f32; 8], i: usize, j: usize) {
    let sum = buf[i] + buf[j];
    let diff = buf[i] - buf[j];
    buf[i] = sum;
    buf[j] = diff;
}

/// Apply the 8×8 Hadamard transform in place via 3-stage butterfly.
///
/// The result is scaled by 1/√8 for energy preservation. This is equivalent
/// to multiplying by the normalized Hadamard matrix H₈/√8, which is
/// orthogonal (unitary): H₈ H₈ᵀ = 8I.
///
/// Reference: Wikipedia, "Fast Walsh–Hadamard Transform".
#[inline]
fn hadamard8(buf: &mut [f32; 8]) {
    // Stage 1: pairs (0,1), (2,3), (4,5), (6,7)
    butterfly_at(buf, 0, 1);
    butterfly_at(buf, 2, 3);
    butterfly_at(buf, 4, 5);
    butterfly_at(buf, 6, 7);

    // Stage 2: pairs (0,2), (1,3), (4,6), (5,7)
    butterfly_at(buf, 0, 2);
    butterfly_at(buf, 1, 3);
    butterfly_at(buf, 4, 6);
    butterfly_at(buf, 5, 7);

    // Stage 3: pairs (0,4), (1,5), (2,6), (3,7)
    butterfly_at(buf, 0, 4);
    butterfly_at(buf, 1, 5);
    butterfly_at(buf, 2, 6);
    butterfly_at(buf, 3, 7);

    // Energy-preserving scale
    for x in buf.iter_mut() {
        *x *= HADAMARD_SCALE;
    }
}

// ── Reverb type presets (kept for programmatic API, not in ParameterInfo) ──

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
    /// Returns (room_size, decay, damping, predelay_ms).
    pub fn defaults(&self) -> (f32, f32, f32, f32) {
        match self {
            ReverbType::Room => (0.5, 0.5, 0.5, 10.0),
            ReverbType::Hall => (0.8, 0.8, 0.3, 25.0),
        }
    }
}

// ── Reverb struct ───────────────────────────────────────────────────────

/// Algorithmic reverb using an 8×8 Hadamard FDN with early reflections.
///
/// The reverb core is an 8-line Feedback Delay Network where energy is
/// redistributed via a Hadamard matrix at each sample. Each delay line has
/// sinusoidal modulation to break metallic resonances and a one-pole lowpass
/// for natural high-frequency absorption. A tapped delay provides early
/// reflections with room-size-dependent timing.
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
/// | 6 | ER Level | 0–100% | 50.0 |
/// | 7 | Output | −20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Reverb;
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
    // ── FDN: 8 delay lines with LFO modulation + feedback damping ──
    fdn_delays: [InterpolatedDelay; 8],
    fdn_damping: [OnePole; 8],
    fdn_base_delays: [f32; 8],
    fdn_mod_depth: f32,
    fdn_phases: [f32; 8],
    fdn_phase_incs: [f32; 8],

    /// Hadamard-mixed feedback from previous sample, one value per FDN line.
    fdn_fb: [f32; 8],

    // ── Early reflections ──
    er_delay: InterpolatedDelay,
    /// Base tap positions in samples (scaled to current sample rate).
    er_base_taps: [f32; ER_TAP_COUNT],

    // ── Allpass diffusion (stereo, modulated) ──
    allpasses_l: [ModulatedAllpass; 4],
    allpasses_r: [ModulatedAllpass; 4],

    // ── Predelay (stereo) ──
    predelay_line: InterpolatedDelay,
    predelay_line_r: InterpolatedDelay,
    predelay_samples: SmoothedParam,

    // ── Smoothed parameters ──
    room_size: SmoothedParam,
    decay: SmoothedParam,
    damping: SmoothedParam,
    mix: SmoothedParam,
    er_level: SmoothedParam,
    output_level: SmoothedParam,

    /// Stereo width (0 = mono, 1 = full stereo).
    stereo_width: f32,

    sample_rate: f32,

    // ── Cached derived values (updated on parameter change) ──
    cached_room: f32,
    cached_decay: f32,
    cached_damp: f32,
    /// FDN feedback coefficient derived from room_size + decay.
    feedback: f32,
    /// Wet-signal compensation: sqrt(1 − feedback).
    fdn_compensation: f32,
    /// ER tap scaling: 0.5 + room_size * 1.5 (range 0.5 → 2.0).
    er_room_scale: f32,
}

impl Reverb {
    /// Create a new reverb at the given sample rate.
    pub fn new(sample_rate: f32) -> Self {
        let mod_depth = FDN_MOD_DEPTH_MS * 0.001 * sample_rate;

        let fdn_delays: [InterpolatedDelay; 8] = core::array::from_fn(|i| {
            let base = scale_to_rate(FDN_TUNINGS_44K[i], sample_rate) as f32;
            let capacity = (base + mod_depth) as usize + 4;
            let mut delay = InterpolatedDelay::new(capacity);
            delay.set_interpolation(Interpolation::Cubic);
            delay
        });

        let damping_hz = damping_to_hz(0.5); // default damping = 50%
        let fdn_damping: [OnePole; 8] =
            core::array::from_fn(|_| OnePole::new(sample_rate, damping_hz));

        let fdn_base_delays: [f32; 8] =
            core::array::from_fn(|i| scale_to_rate(FDN_TUNINGS_44K[i], sample_rate) as f32);

        let fdn_phases = [0.0f32; 8];
        let fdn_phase_incs: [f32; 8] =
            core::array::from_fn(|i| core::f32::consts::TAU * FDN_MOD_RATES[i] / sample_rate);

        // Early reflections tapped delay (~80 ms max)
        let er_max = (ceilf(MAX_ER_MS * 0.001 * sample_rate) as usize).max(1);
        let er_delay = InterpolatedDelay::new(er_max);
        let er_base_taps: [f32; ER_TAP_COUNT] =
            core::array::from_fn(|i| scale_to_rate(ER_TAP_POSITIONS_44K[i], sample_rate) as f32);

        // Allpass diffusion (modulated, stereo)
        let allpasses_l: [ModulatedAllpass; 4] = core::array::from_fn(|i| {
            let base = scale_to_rate(ALLPASS_TUNINGS_44K[i], sample_rate) as f32;
            ModulatedAllpass::new(
                base,
                ALLPASS_FEEDBACK,
                0.7,
                ALLPASS_MOD_DEPTH_MS,
                sample_rate,
            )
        });
        let allpasses_r: [ModulatedAllpass; 4] = core::array::from_fn(|i| {
            let base = scale_to_rate(ALLPASS_TUNINGS_44K_R[i], sample_rate) as f32;
            ModulatedAllpass::new(
                base,
                ALLPASS_FEEDBACK,
                0.8,
                ALLPASS_MOD_DEPTH_MS,
                sample_rate,
            )
        });

        // Predelay (stereo, up to 100 ms)
        let max_predelay = (ceilf(MAX_PREDELAY_MS * 0.001 * sample_rate) as usize).max(1);
        let predelay_line = InterpolatedDelay::new(max_predelay);
        let predelay_line_r = InterpolatedDelay::new(max_predelay);

        let (room, decay, damp, predelay_ms) = ReverbType::Room.defaults();
        let predelay_samps = predelay_ms * 0.001 * sample_rate;

        let mut reverb = Self {
            fdn_delays,
            fdn_damping,
            fdn_base_delays,
            fdn_mod_depth: mod_depth,
            fdn_phases,
            fdn_phase_incs,
            fdn_fb: [0.0; 8],
            er_delay,
            er_base_taps,
            allpasses_l,
            allpasses_r,
            predelay_line,
            predelay_line_r,
            predelay_samples: SmoothedParam::interpolated(predelay_samps, sample_rate),
            room_size: SmoothedParam::slow(room, sample_rate),
            decay: SmoothedParam::slow(decay, sample_rate),
            damping: SmoothedParam::slow(damp, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            er_level: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            stereo_width: 1.0,
            sample_rate,
            cached_room: -1.0,
            cached_decay: -1.0,
            cached_damp: -1.0,
            feedback: 0.0,
            fdn_compensation: 1.0,
            er_room_scale: 1.0,
        };
        reverb.update_derived_params();
        reverb
    }

    // ── Public parameter accessors ──────────────────────────────────────

    /// Set the room size (0.0 to 1.0).
    ///
    /// Controls early reflection timing and FDN feedback density.
    /// Higher values create wider, more spacious reflections.
    pub fn set_room_size(&mut self, size: f32) {
        self.room_size.set_target(size.clamp(0.0, 1.0));
    }

    /// Get the current room size.
    pub fn room_size(&self) -> f32 {
        self.room_size.target()
    }

    /// Set the decay time (0.0 to 1.0).
    ///
    /// Controls how long the reverb tail lasts. Higher values create
    /// longer, more sustained tails.
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
    /// - 1.0 = dark (heavy HF absorption)
    pub fn set_damping(&mut self, damping: f32) {
        self.damping.set_target(damping.clamp(0.0, 1.0));
    }

    /// Get the current damping value.
    pub fn damping(&self) -> f32 {
        self.damping.target()
    }

    /// Set the pre-delay time in milliseconds (0 to 100 ms).
    pub fn set_predelay_ms(&mut self, ms: f32) {
        let clamped = ms.clamp(0.0, MAX_PREDELAY_MS);
        self.predelay_samples
            .set_target(clamped * 0.001 * self.sample_rate);
    }

    /// Get the current pre-delay in milliseconds.
    pub fn predelay_ms(&self) -> f32 {
        self.predelay_samples.target() / self.sample_rate * 1000.0
    }

    /// Set the wet/dry mix (0.0 to 1.0).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Get the current mix value.
    pub fn mix(&self) -> f32 {
        self.mix.target()
    }

    /// Set the early reflections level (0.0 to 1.0).
    ///
    /// Controls how prominent the early reflections are relative to
    /// the late diffuse reverb.
    pub fn set_er_level(&mut self, level: f32) {
        self.er_level.set_target(level.clamp(0.0, 1.0));
    }

    /// Get the current ER level.
    pub fn er_level(&self) -> f32 {
        self.er_level.target()
    }

    /// Set stereo width (0 = mono, 1 = full stereo).
    pub fn set_stereo_width(&mut self, width: f32) {
        self.stereo_width = width.clamp(0.0, 1.0);
    }

    /// Get the current stereo width.
    pub fn stereo_width(&self) -> f32 {
        self.stereo_width
    }

    /// Set the reverb type preset.
    ///
    /// Convenience method that sets room_size, decay, damping, and predelay
    /// to preset values for common room types.
    pub fn set_reverb_type(&mut self, reverb_type: ReverbType) {
        let (room, decay, damp, predelay_ms) = reverb_type.defaults();
        self.set_room_size(room);
        self.set_decay(decay);
        self.set_damping(damp);
        self.set_predelay_ms(predelay_ms);
    }

    // ── Internal helpers ────────────────────────────────────────────────

    /// Advance all smoothed parameters by one sample and update derived values.
    ///
    /// Returns `(predelay_samples, mix, er_level, output_gain)`.
    #[inline]
    fn advance_params(&mut self) -> (f32, f32, f32, f32) {
        self.room_size.advance();
        self.decay.advance();
        self.damping.advance();
        let predelay = self.predelay_samples.advance();
        let mix = self.mix.advance();
        let er_lvl = self.er_level.advance();
        let output_gain = self.output_level.advance();

        self.update_derived_params();

        (predelay, mix, er_lvl, output_gain)
    }

    /// Recompute feedback, compensation, damping, and ER scale when
    /// room/decay/damping change.
    fn update_derived_params(&mut self) {
        let room = self.room_size.get();
        let decay = self.decay.get();
        let damp = self.damping.get();

        if (room - self.cached_room).abs() < 0.001
            && (decay - self.cached_decay).abs() < 0.001
            && (damp - self.cached_damp).abs() < 0.001
        {
            return;
        }
        self.cached_room = room;
        self.cached_decay = decay;
        self.cached_damp = damp;

        // Freeverb feedback formula:
        // scaled_room = 0.28 + room * 0.7  (range 0.28..0.98)
        // feedback    = scaled_room + decay * (0.98 − scaled_room)
        let scaled_room = 0.28 + room * 0.7;
        self.feedback = (scaled_room + decay * (0.98 - scaled_room)).clamp(0.0, 0.99);
        self.fdn_compensation = sqrtf((1.0 - self.feedback).max(0.01));

        // ER tap scaling: small room → tight ER, large room → spacious ER
        self.er_room_scale = 0.5 + room * 1.5;

        // Update damping cutoff for all FDN lines
        let freq = damping_to_hz(damp);
        for filter in &mut self.fdn_damping {
            filter.set_frequency(freq);
        }
    }

    /// Process pre-delay for one channel.
    #[inline]
    fn apply_predelay(line: &mut InterpolatedDelay, input: f32, predelay: f32) -> f32 {
        if predelay > 0.5 {
            line.read_write(input, predelay)
        } else {
            line.write(input);
            input
        }
    }

    /// Compute early reflections (mono) from the ER tapped delay.
    ///
    /// Even-indexed taps contribute to L, odd-indexed to R.
    /// Returns `(er_left, er_right)`.
    #[inline]
    fn compute_er_stereo(&self) -> (f32, f32) {
        let mut er_l = 0.0f32;
        let mut er_r = 0.0f32;
        for i in 0..ER_TAP_COUNT {
            let tap_pos = self.er_base_taps[i] * self.er_room_scale;
            let sample = self.er_delay.read(tap_pos);
            let gain = er_tap_gain(i);
            if i % 2 == 0 {
                er_l += sample * gain;
            } else {
                er_r += sample * gain;
            }
        }
        (er_l, er_r)
    }

    /// Compute early reflections (mono sum of all taps).
    #[inline]
    fn compute_er_mono(&self) -> f32 {
        let mut er = 0.0f32;
        for i in 0..ER_TAP_COUNT {
            let tap_pos = self.er_base_taps[i] * self.er_room_scale;
            er += self.er_delay.read(tap_pos) * er_tap_gain(i);
        }
        er
    }

    /// Process one sample through the 8×8 Hadamard FDN.
    ///
    /// 1. Read modulated outputs from all 8 delay lines.
    /// 2. Apply Hadamard butterfly to the feedback buffer.
    /// 3. Damp the mixed signals and write them back with the new input.
    /// 4. Return raw outputs split to stereo: L = even, R = odd.
    #[inline]
    fn process_fdn(&mut self, input: f32) -> (f32, f32) {
        let mut raw = [0.0f32; 8];

        // 1. Read from all FDN delay lines (modulated read position)
        for i in 0..8 {
            let modulated = self.fdn_base_delays[i] + self.fdn_mod_depth * sinf(self.fdn_phases[i]);
            raw[i] = self.fdn_delays[i].read(modulated);
        }

        // 2. Hadamard-mix the raw outputs for the feedback path
        let mut mixed = raw;
        hadamard8(&mut mixed);

        // 3. Damp mixed signals and write back: input + damped_feedback
        for i in 0..8 {
            let damped = self.fdn_damping[i].process(mixed[i]);
            self.fdn_delays[i].write(flush_denormal(input + damped * self.feedback));

            // Advance LFO phase
            self.fdn_phases[i] += self.fdn_phase_incs[i];
            if self.fdn_phases[i] >= core::f32::consts::TAU {
                self.fdn_phases[i] -= core::f32::consts::TAU;
            }
        }

        // 4. Stereo output: L = even delays, R = odd delays
        let fdn_l = (raw[0] + raw[2] + raw[4] + raw[6]) * 0.25;
        let fdn_r = (raw[1] + raw[3] + raw[5] + raw[7]) * 0.25;

        (fdn_l * self.fdn_compensation, fdn_r * self.fdn_compensation)
    }
}

impl Default for Reverb {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Reverb {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let (predelay, mix, er_lvl, output_gain) = self.advance_params();

        let predelayed = Self::apply_predelay(&mut self.predelay_line, input, predelay);

        // Early reflections
        self.er_delay.write(predelayed);
        let er = self.compute_er_mono();

        // FDN
        let (fdn_l, fdn_r) = self.process_fdn(predelayed);
        let fdn_mono = (fdn_l + fdn_r) * 0.5;

        // Allpass diffusion
        let mut diffused = fdn_mono;
        for ap in &mut self.allpasses_l {
            diffused = ap.process(diffused);
        }

        // Combine ER + late reverb
        let wet = diffused + er * er_lvl;

        wet_dry_mix(input, wet, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (predelay, mix, er_lvl, output_gain) = self.advance_params();

        // Stereo predelay
        let pre_l = Self::apply_predelay(&mut self.predelay_line, left, predelay);
        let pre_r = Self::apply_predelay(&mut self.predelay_line_r, right, predelay);
        let mono = (pre_l + pre_r) * 0.5;

        // Early reflections (stereo)
        self.er_delay.write(mono);
        let (er_l, er_r) = self.compute_er_stereo();

        // FDN (mono in → stereo out)
        let (fdn_l, fdn_r) = self.process_fdn(mono);

        // Allpass diffusion (separate L/R chains)
        let mut diff_l = fdn_l;
        for ap in &mut self.allpasses_l {
            diff_l = ap.process(diff_l);
        }
        let mut diff_r = fdn_r;
        for ap in &mut self.allpasses_r {
            diff_r = ap.process(diff_r);
        }

        // Combine ER + late reverb
        let wet_l = diff_l + er_l * er_lvl;
        let wet_r = diff_r + er_r * er_lvl;

        // Stereo width: mid/side
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5;
        let final_l = mid + side * self.stereo_width;
        let final_r = mid - side * self.stereo_width;

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, final_l, final_r, mix);
        (out_l * output_gain, out_r * output_gain)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn process_block_stereo(
        &mut self,
        left_in: &[f32],
        right_in: &[f32],
        left_out: &mut [f32],
        right_out: &mut [f32],
    ) {
        debug_assert_eq!(left_in.len(), right_in.len());
        debug_assert_eq!(left_in.len(), left_out.len());
        debug_assert_eq!(left_out.len(), right_out.len());

        for i in 0..left_in.len() {
            let (l, r) = self.process_stereo(left_in[i], right_in[i]);
            left_out[i] = l;
            right_out[i] = r;
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        let mod_depth = FDN_MOD_DEPTH_MS * 0.001 * sample_rate;
        self.fdn_mod_depth = mod_depth;

        // Recreate FDN delay lines
        self.fdn_delays = core::array::from_fn(|i| {
            let base = scale_to_rate(FDN_TUNINGS_44K[i], sample_rate) as f32;
            let capacity = (base + mod_depth) as usize + 4;
            let mut delay = InterpolatedDelay::new(capacity);
            delay.set_interpolation(Interpolation::Cubic);
            delay
        });
        self.fdn_base_delays =
            core::array::from_fn(|i| scale_to_rate(FDN_TUNINGS_44K[i], sample_rate) as f32);
        self.fdn_phase_incs =
            core::array::from_fn(|i| core::f32::consts::TAU * FDN_MOD_RATES[i] / sample_rate);
        for filter in &mut self.fdn_damping {
            filter.set_sample_rate(sample_rate);
        }
        self.fdn_phases = [0.0; 8];
        self.fdn_fb = [0.0; 8];

        // Recreate ER delay
        let er_max = (ceilf(MAX_ER_MS * 0.001 * sample_rate) as usize).max(1);
        self.er_delay = InterpolatedDelay::new(er_max);
        self.er_base_taps =
            core::array::from_fn(|i| scale_to_rate(ER_TAP_POSITIONS_44K[i], sample_rate) as f32);

        // Recreate allpass diffusion
        self.allpasses_l = core::array::from_fn(|i| {
            let base = scale_to_rate(ALLPASS_TUNINGS_44K[i], sample_rate) as f32;
            ModulatedAllpass::new(
                base,
                ALLPASS_FEEDBACK,
                0.7,
                ALLPASS_MOD_DEPTH_MS,
                sample_rate,
            )
        });
        self.allpasses_r = core::array::from_fn(|i| {
            let base = scale_to_rate(ALLPASS_TUNINGS_44K_R[i], sample_rate) as f32;
            ModulatedAllpass::new(
                base,
                ALLPASS_FEEDBACK,
                0.8,
                ALLPASS_MOD_DEPTH_MS,
                sample_rate,
            )
        });

        // Recreate predelay
        let max_predelay = (ceilf(MAX_PREDELAY_MS * 0.001 * sample_rate) as usize).max(1);
        self.predelay_line = InterpolatedDelay::new(max_predelay);
        self.predelay_line_r = InterpolatedDelay::new(max_predelay);

        // Update parameter sample rates
        self.room_size.set_sample_rate(sample_rate);
        self.decay.set_sample_rate(sample_rate);
        self.damping.set_sample_rate(sample_rate);
        self.predelay_samples.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.er_level.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);

        // Force derived param update
        self.cached_room = -1.0;
        self.update_derived_params();
    }

    fn reset(&mut self) {
        for delay in &mut self.fdn_delays {
            delay.clear();
        }
        for filter in &mut self.fdn_damping {
            filter.reset();
        }
        self.fdn_phases = [0.0; 8];
        self.fdn_fb = [0.0; 8];

        self.er_delay.clear();

        for ap in &mut self.allpasses_l {
            ap.reset();
        }
        for ap in &mut self.allpasses_r {
            ap.reset();
        }

        self.predelay_line.clear();
        self.predelay_line_r.clear();

        self.room_size.snap_to_target();
        self.decay.snap_to_target();
        self.damping.snap_to_target();
        self.predelay_samples.snap_to_target();
        self.mix.snap_to_target();
        self.er_level.snap_to_target();
        self.output_level.snap_to_target();

        self.cached_room = -1.0;
        self.update_derived_params();
    }

    fn latency_samples(&self) -> usize {
        0 // Predelay is musical, not processing latency
    }
}

// ── ParameterInfo ───────────────────────────────────────────────────────

sonido_core::impl_params! {
    Reverb, this {
        [0] ParamDescriptor {
                    name: "Room Size",
                    short_name: "Room",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1500), "rev_room_size"),
            get: this.room_size() * 100.0,
            set: |v| this.set_room_size(v / 100.0);

        [1] ParamDescriptor {
                    name: "Decay",
                    short_name: "Decay",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1501), "rev_decay"),
            get: this.decay() * 100.0,
            set: |v| this.set_decay(v / 100.0);

        [2] ParamDescriptor {
                    name: "Damping",
                    short_name: "Damping",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1502), "rev_damping"),
            get: this.damping() * 100.0,
            set: |v| this.set_damping(v / 100.0);

        [3] ParamDescriptor::custom("Pre-Delay", "PreDly", 0.0, 100.0, 10.0)
                .with_unit(ParamUnit::Milliseconds)
                .with_step(1.0)
                .with_id(ParamId(1503), "rev_predelay"),
            get: this.predelay_ms(),
            set: |v| this.set_predelay_ms(v);

        [4] ParamDescriptor::mix()
                .with_id(ParamId(1504), "rev_mix"),
            get: this.mix() * 100.0,
            set: |v| this.set_mix(v / 100.0);

        [5] ParamDescriptor {
                    name: "Stereo Width",
                    short_name: "Width",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 100.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1505), "rev_width"),
            get: this.stereo_width * 100.0,
            set: |v| this.set_stereo_width(v / 100.0);

        [6] ParamDescriptor {
                    name: "ER Level",
                    short_name: "ER Lvl",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 50.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1508), "rev_er_level"),
            get: this.er_level() * 100.0,
            set: |v| this.set_er_level(v / 100.0);

        [7] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1507), "rev_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_basic_processing() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.reset();

        let _first = reverb.process(1.0);

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

        reverb.process(1.0);

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

        reverb.set_room_size(2.0);
        reverb.set_decay(-1.0);
        reverb.set_damping(1.5);
        reverb.set_mix(1.1);
        reverb.set_predelay_ms(200.0);
        reverb.set_er_level(2.0);

        assert!(reverb.room_size() <= 1.0);
        assert!(reverb.decay() >= 0.0);
        assert!(reverb.damping() <= 1.0);
        assert!(reverb.mix() <= 1.0);
        assert!(reverb.predelay_ms() <= MAX_PREDELAY_MS);
        assert!(reverb.er_level() <= 1.0);
    }

    #[test]
    fn test_reverb_type_presets() {
        let mut reverb = Reverb::new(48000.0);

        reverb.set_reverb_type(ReverbType::Hall);
        assert!(reverb.decay() > 0.7);

        reverb.set_reverb_type(ReverbType::Room);
        assert!(reverb.decay() < 0.6);
    }

    #[test]
    fn test_reverb_mix() {
        let mut dry_reverb = Reverb::new(48000.0);
        dry_reverb.set_mix(0.0);
        dry_reverb.reset();

        let mut wet_reverb = Reverb::new(48000.0);
        wet_reverb.set_mix(1.0);
        wet_reverb.reset();

        let dry_out = dry_reverb.process(0.5);
        assert!(
            (dry_out - 0.5).abs() < 0.01,
            "Dry output should match input"
        );

        let wet_out = wet_reverb.process(0.5);
        assert!(wet_out.is_finite());
    }

    #[test]
    fn test_reverb_predelay() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_predelay_ms(50.0);
        assert!((reverb.predelay_ms() - 50.0).abs() < 0.1);

        reverb.set_predelay_ms(0.0);
        assert!((reverb.predelay_ms() - 0.0).abs() < 0.1);

        reverb.set_predelay_ms(200.0);
        assert!(reverb.predelay_ms() <= 100.0);
    }

    #[test]
    fn test_reverb_stereo_processing() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_mix(1.0);
        reverb.reset();

        let (l, r) = reverb.process_stereo(1.0, 1.0);
        assert!(l.is_finite());
        assert!(r.is_finite());

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

        reverb.process_stereo(1.0, 0.5);

        for _ in 0..5000 {
            reverb.process_stereo(0.0, 0.0);
        }

        let mut diff_count = 0;
        for _ in 0..1000 {
            let (l, r) = reverb.process_stereo(0.0, 0.0);
            if (l - r).abs() > 0.0001 {
                diff_count += 1;
            }
        }

        assert!(
            diff_count > 100,
            "L and R should be decorrelated, but only {} samples differed",
            diff_count
        );
    }

    #[test]
    fn test_reverb_latency_samples() {
        let reverb = Reverb::new(48000.0);
        assert_eq!(
            reverb.latency_samples(),
            0,
            "Predelay is musical, not processing latency"
        );
    }

    #[test]
    fn test_no_denormals_after_silence() {
        let mut reverb = Reverb::new(48000.0);
        reverb.set_decay(0.9);
        reverb.set_mix(1.0);
        reverb.reset();

        for _ in 0..1000 {
            reverb.process(0.5);
        }

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

        for _ in 0..1000 {
            reverb.process_stereo(0.5, 0.5);
        }

        for i in 0..200_000 {
            let (l, r) = reverb.process_stereo(0.0, 0.0);
            assert!(
                l == 0.0 || l.abs() > f32::MIN_POSITIVE,
                "Left denormal detected at sample {}: {:.2e}",
                i,
                l,
            );
            assert!(
                r == 0.0 || r.abs() > f32::MIN_POSITIVE,
                "Right denormal detected at sample {}: {:.2e}",
                i,
                r,
            );
        }
    }

    #[test]
    fn test_er_level_parameter() {
        let mut reverb = Reverb::new(48000.0);

        reverb.set_er_level(0.0);
        assert!((reverb.er_level() - 0.0).abs() < 0.01);

        reverb.set_er_level(1.0);
        assert!((reverb.er_level() - 1.0).abs() < 0.01);

        reverb.set_er_level(0.5);
        assert!((reverb.er_level() - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_hadamard_orthogonality() {
        // H₈ * H₈ᵀ = I (for the normalized version)
        // Verify: applying hadamard8 twice returns the original vector
        let original = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mut buf = original;
        hadamard8(&mut buf);
        hadamard8(&mut buf);

        for i in 0..8 {
            assert!(
                (buf[i] - original[i]).abs() < 1e-5,
                "Hadamard should be involutory (H² = I): buf[{}] = {}, expected {}",
                i,
                buf[i],
                original[i]
            );
        }
    }

    #[test]
    fn test_hadamard_energy_preservation() {
        let mut buf = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let energy_before: f32 = buf.iter().map(|x| x * x).sum();
        hadamard8(&mut buf);
        let energy_after: f32 = buf.iter().map(|x| x * x).sum();

        assert!(
            (energy_before - energy_after).abs() < 1e-5,
            "Hadamard should preserve energy: before={}, after={}",
            energy_before,
            energy_after
        );
    }
}
