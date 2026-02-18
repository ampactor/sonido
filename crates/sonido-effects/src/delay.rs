//! Classic delay effect with feedback filtering, diffusion, and tempo sync.
//!
//! Feedback path includes a one-pole lowpass and biquad highpass filter for
//! natural repeat darkening/thinning, plus cascaded allpass diffusion for
//! smeared, tape-like echoes. Tempo sync overrides the manual delay time
//! with a musical note division at the current BPM.
//!
//! ## Signal Flow
//!
//! ```text
//! input ──►(+)──► [delay line] ──► delayed ──► wet/dry mix ──► output
//!           ▲                         │
//!           │    ◄── feedback gain ◄──┤
//!           │    ◄── LP filter ◄──────┤
//!           │    ◄── HP filter ◄──────┤
//!           └──── allpass diffusion ◄─┘
//! ```
//!
//! ## Reference
//!
//! - Feedback filtering: standard analog delay modeling technique
//! - Diffusion: Schroeder allpass cascade (prime delay lengths 13 ms, 7 ms)
//! - Tempo sync: musical note divisions via [`NoteDivision`]

use libm::ceilf;
use sonido_core::{
    AllpassFilter, Biquad, Effect, InterpolatedDelay, NoteDivision, OnePole, ParamDescriptor,
    ParamFlags, ParamId, ParamScale, ParamUnit, SmoothedParam, TempoManager, flush_denormal,
    highpass_coefficients, wet_dry_mix, wet_dry_mix_stereo,
};

/// Note division labels for the stepped division parameter.
const DIVISION_LABELS: &[&str] = &[
    "Whole",
    "Half",
    "Quarter",
    "Eighth",
    "Sixteenth",
    "32nd",
    "Dot Half",
    "Dot Qtr",
    "Dot 8th",
    "Trip Qtr",
    "Trip 8th",
    "Trip 16th",
];

/// Map integer index (0–11) to [`NoteDivision`] variant.
fn index_to_division(index: u8) -> NoteDivision {
    match index {
        0 => NoteDivision::Whole,
        1 => NoteDivision::Half,
        2 => NoteDivision::Quarter,
        3 => NoteDivision::Eighth,
        4 => NoteDivision::Sixteenth,
        5 => NoteDivision::ThirtySecond,
        6 => NoteDivision::DottedHalf,
        7 => NoteDivision::DottedQuarter,
        8 => NoteDivision::DottedEighth,
        9 => NoteDivision::TripletQuarter,
        10 => NoteDivision::TripletEighth,
        11 => NoteDivision::TripletSixteenth,
        _ => NoteDivision::Quarter,
    }
}

/// Map [`NoteDivision`] variant to integer index (0–11).
fn division_to_index(div: NoteDivision) -> u8 {
    match div {
        NoteDivision::Whole => 0,
        NoteDivision::Half => 1,
        NoteDivision::Quarter => 2,
        NoteDivision::Eighth => 3,
        NoteDivision::Sixteenth => 4,
        NoteDivision::ThirtySecond => 5,
        NoteDivision::DottedHalf => 6,
        NoteDivision::DottedQuarter => 7,
        NoteDivision::DottedEighth => 8,
        NoteDivision::TripletQuarter => 9,
        NoteDivision::TripletEighth => 10,
        NoteDivision::TripletSixteenth => 11,
    }
}

/// Classic delay effect with feedback filtering, diffusion, and tempo sync.
///
/// In mono mode, operates as a standard feedback delay.
/// In stereo mode with ping_pong enabled, creates alternating L/R repeats.
/// Feedback path includes lowpass and highpass filters for tonal shaping of
/// repeats, plus cascaded allpass filters for diffusion.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Delay Time | 1.0–2000.0 ms | 300.0 |
/// | 1 | Feedback | 0–95% | 40.0 |
/// | 2 | Mix | 0–100% | 50.0 |
/// | 3 | Ping Pong | 0–1 | 0 |
/// | 4 | Feedback LP | 200–20000 Hz | 20000.0 |
/// | 5 | Feedback HP | 20–2000 Hz | 20.0 |
/// | 6 | Diffusion | 0–100% | 0.0 |
/// | 7 | Sync | 0–1 | 0 |
/// | 8 | Division | 0–11 (note division) | 2 (Quarter) |
/// | 9 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Delay;
/// use sonido_core::Effect;
///
/// let mut delay = Delay::new(44100.0);
/// delay.set_delay_time_ms(375.0);
/// delay.set_feedback(0.5);
/// delay.set_mix(0.3);
/// delay.set_ping_pong(true); // Enable stereo ping-pong
///
/// let input = 0.5;
/// let output = delay.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Delay {
    delay_line: InterpolatedDelay,
    delay_line_r: InterpolatedDelay,
    max_delay_samples: f32,
    delay_time: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    output_level: SmoothedParam,
    sample_rate: f32,
    /// Ping-pong mode: feedback crosses between L/R channels.
    ping_pong: bool,
    // -- Feedback filtering --
    /// One-pole lowpass in feedback path (L channel / mono).
    feedback_lp_l: OnePole,
    /// One-pole lowpass in feedback path (R channel).
    feedback_lp_r: OnePole,
    /// Biquad highpass in feedback path (L channel / mono).
    feedback_hp_l: Biquad,
    /// Biquad highpass in feedback path (R channel).
    feedback_hp_r: Biquad,
    /// Feedback lowpass cutoff frequency in Hz.
    feedback_lp_freq: f32,
    /// Feedback highpass cutoff frequency in Hz.
    feedback_hp_freq: f32,
    // -- Diffusion --
    /// First allpass diffuser (L / mono), 13 ms prime delay.
    diffusion_ap1_l: AllpassFilter,
    /// Second allpass diffuser (L / mono), 7 ms prime delay.
    diffusion_ap2_l: AllpassFilter,
    /// First allpass diffuser (R), 13 ms.
    diffusion_ap1_r: AllpassFilter,
    /// Second allpass diffuser (R), 7 ms.
    diffusion_ap2_r: AllpassFilter,
    /// Diffusion amount 0.0–1.0 (maps to allpass feedback 0.0–0.6).
    diffusion: f32,
    // -- Tempo sync --
    /// Tempo manager for synced delay times.
    tempo: TempoManager,
    /// Whether tempo sync is active.
    sync: bool,
    /// Selected note division for tempo sync.
    division: NoteDivision,
}

impl Delay {
    /// Create a new delay with 2-second maximum delay.
    pub fn new(sample_rate: f32) -> Self {
        Self::with_max_delay_ms(sample_rate, 2000.0)
    }

    /// Create a new delay with custom maximum delay time.
    pub fn with_max_delay_ms(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_delay_samples = ceilf((max_delay_ms / 1000.0) * sample_rate) as usize;
        let max_delay_samples_f32 = max_delay_samples as f32;
        let default_delay_samples = ((300.0 / 1000.0) * sample_rate).min(max_delay_samples_f32);

        // Diffusion allpass delay lengths: 13ms and 7ms (prime numbers)
        let ap1_samples = (0.013 * sample_rate) as usize;
        let ap2_samples = (0.007 * sample_rate) as usize;

        // Initialize HP biquad at 20 Hz (effectively off)
        let mut feedback_hp_l = Biquad::new();
        let mut feedback_hp_r = Biquad::new();
        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(20.0, 0.707, sample_rate);
        feedback_hp_l.set_coefficients(b0, b1, b2, a0, a1, a2);
        feedback_hp_r.set_coefficients(b0, b1, b2, a0, a1, a2);

        Self {
            delay_line: InterpolatedDelay::new(max_delay_samples),
            delay_line_r: InterpolatedDelay::new(max_delay_samples),
            max_delay_samples: max_delay_samples_f32,
            delay_time: SmoothedParam::interpolated(default_delay_samples, sample_rate),
            feedback: SmoothedParam::standard(0.4, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            sample_rate,
            ping_pong: false,
            feedback_lp_l: OnePole::new(sample_rate, 20000.0),
            feedback_lp_r: OnePole::new(sample_rate, 20000.0),
            feedback_hp_l,
            feedback_hp_r,
            feedback_lp_freq: 20000.0,
            feedback_hp_freq: 20.0,
            diffusion_ap1_l: AllpassFilter::new(ap1_samples.max(1)),
            diffusion_ap2_l: AllpassFilter::new(ap2_samples.max(1)),
            diffusion_ap1_r: AllpassFilter::new(ap1_samples.max(1)),
            diffusion_ap2_r: AllpassFilter::new(ap2_samples.max(1)),
            diffusion: 0.0,
            tempo: TempoManager::new(sample_rate, 120.0),
            sync: false,
            division: NoteDivision::Quarter,
        }
    }

    /// Set delay time in milliseconds.
    ///
    /// Range: 1.0 to max_delay_ms. Ignored when tempo sync is active.
    pub fn set_delay_time_ms(&mut self, delay_ms: f32) {
        let delay_samples = (delay_ms / 1000.0) * self.sample_rate;
        let clamped = delay_samples.clamp(1.0, self.max_delay_samples - 1.0);
        self.delay_time.set_target(clamped);
    }

    /// Set feedback amount (0–0.95).
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback.set_target(feedback.clamp(0.0, 0.95));
    }

    /// Set wet/dry mix (0–1).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Enable or disable ping-pong stereo mode.
    ///
    /// In ping-pong mode, feedback alternates between left and right channels,
    /// creating a bouncing stereo delay effect.
    pub fn set_ping_pong(&mut self, enabled: bool) {
        self.ping_pong = enabled;
    }

    /// Get current ping-pong mode state.
    pub fn ping_pong(&self) -> bool {
        self.ping_pong
    }

    /// Set the feedback lowpass filter cutoff frequency.
    ///
    /// Range: 200.0 to 20000.0 Hz. At 20 kHz the filter is effectively bypassed.
    /// Each delay repeat loses high-frequency content above this cutoff,
    /// simulating analog tape or bucket-brigade darkening.
    pub fn set_feedback_lp(&mut self, freq_hz: f32) {
        let freq = freq_hz.clamp(200.0, 20000.0);
        self.feedback_lp_freq = freq;
        self.feedback_lp_l.set_frequency(freq);
        self.feedback_lp_r.set_frequency(freq);
    }

    /// Set the feedback highpass filter cutoff frequency.
    ///
    /// Range: 20.0 to 2000.0 Hz. At 20 Hz the filter is effectively bypassed.
    /// Each delay repeat loses low-frequency content below this cutoff,
    /// preventing muddy bass buildup in the feedback path.
    pub fn set_feedback_hp(&mut self, freq_hz: f32) {
        let freq = freq_hz.clamp(20.0, 2000.0);
        self.feedback_hp_freq = freq;
        let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(freq, 0.707, self.sample_rate);
        self.feedback_hp_l.set_coefficients(b0, b1, b2, a0, a1, a2);
        self.feedback_hp_r.set_coefficients(b0, b1, b2, a0, a1, a2);
    }

    /// Set the diffusion amount.
    ///
    /// Range: 0.0 to 1.0. At 0.0 the allpass filters are bypassed (feedback = 0).
    /// At 1.0, allpass feedback is 0.6 for maximum smearing of repeats.
    pub fn set_diffusion(&mut self, amount: f32) {
        let amount = amount.clamp(0.0, 1.0);
        self.diffusion = amount;
        let ap_feedback = amount * 0.6;
        self.diffusion_ap1_l.set_feedback(ap_feedback);
        self.diffusion_ap2_l.set_feedback(ap_feedback);
        self.diffusion_ap1_r.set_feedback(ap_feedback);
        self.diffusion_ap2_r.set_feedback(ap_feedback);
    }

    /// Enable or disable tempo sync.
    ///
    /// When enabled, delay time is derived from the current BPM and note
    /// division, overriding the manual delay time parameter.
    pub fn set_sync(&mut self, enabled: bool) {
        self.sync = enabled;
        if enabled {
            self.apply_synced_time();
        }
    }

    /// Set the note division for tempo sync.
    ///
    /// Only takes effect when sync is enabled.
    pub fn set_division(&mut self, division: NoteDivision) {
        self.division = division;
        if self.sync {
            self.apply_synced_time();
        }
    }

    /// Set the tempo in BPM for synced delay times.
    ///
    /// Range: 1.0 and up. Updates the internal `TempoManager` and
    /// recalculates synced delay time if sync is active.
    pub fn set_tempo(&mut self, bpm: f32) {
        self.tempo.set_bpm(bpm);
        if self.sync {
            self.apply_synced_time();
        }
    }

    /// Get the current tempo in BPM.
    pub fn tempo_bpm(&self) -> f32 {
        self.tempo.bpm()
    }

    /// Recalculate delay time from tempo and division, clamped to max delay.
    fn apply_synced_time(&mut self) {
        let ms = self.tempo.division_to_ms(self.division);
        let delay_samples = (ms / 1000.0) * self.sample_rate;
        let clamped = delay_samples.clamp(1.0, self.max_delay_samples - 1.0);
        self.delay_time.set_target(clamped);
    }

    /// Process the feedback signal through LP filter, HP filter, and diffusion.
    #[inline]
    fn filter_feedback_l(&mut self, signal: f32) -> f32 {
        let lp = self.feedback_lp_l.process(signal);
        let hp = self.feedback_hp_l.process(lp);
        if self.diffusion > 0.0 {
            let d1 = self.diffusion_ap1_l.process(hp);
            self.diffusion_ap2_l.process(d1)
        } else {
            hp
        }
    }

    /// Process the feedback signal through LP filter, HP filter, and diffusion (R channel).
    #[inline]
    fn filter_feedback_r(&mut self, signal: f32) -> f32 {
        let lp = self.feedback_lp_r.process(signal);
        let hp = self.feedback_hp_r.process(lp);
        if self.diffusion > 0.0 {
            let d1 = self.diffusion_ap1_r.process(hp);
            self.diffusion_ap2_r.process(d1)
        } else {
            hp
        }
    }
}

impl Effect for Delay {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let delay_samples = self.delay_time.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        let delayed = self.delay_line.read(delay_samples);
        let filtered = self.filter_feedback_l(delayed * feedback);
        let feedback_signal = flush_denormal(input + filtered);
        self.delay_line.write(feedback_signal);

        let comp = sonido_core::gain::feedback_wet_compensation(feedback);
        wet_dry_mix(input, delayed * comp, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let delay_samples = self.delay_time.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        let delayed_l = self.delay_line.read(delay_samples);
        let delayed_r = self.delay_line_r.read(delay_samples);

        if self.ping_pong {
            // Ping-pong: feedback crosses channels
            let filtered_l = self.filter_feedback_l(delayed_r * feedback);
            let filtered_r = self.filter_feedback_r(delayed_l * feedback);
            self.delay_line.write(flush_denormal(left + filtered_l));
            self.delay_line_r.write(flush_denormal(right + filtered_r));
        } else {
            let filtered_l = self.filter_feedback_l(delayed_l * feedback);
            let filtered_r = self.filter_feedback_r(delayed_r * feedback);
            self.delay_line.write(flush_denormal(left + filtered_l));
            self.delay_line_r.write(flush_denormal(right + filtered_r));
        }

        let comp = sonido_core::gain::feedback_wet_compensation(feedback);
        let (out_l, out_r) =
            wet_dry_mix_stereo(left, right, delayed_l * comp, delayed_r * comp, mix);

        (out_l * output_gain, out_r * output_gain)
    }

    /// Optimized block processing for stereo delay.
    ///
    /// Processes all samples in a tight loop, advancing `SmoothedParam`s
    /// per sample and handling ping-pong cross-channel feedback when enabled.
    /// Produces bit-identical output to calling `process_stereo()` per sample.
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

    fn is_true_stereo(&self) -> bool {
        self.ping_pong
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.delay_time.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.feedback_lp_l.set_sample_rate(sample_rate);
        self.feedback_lp_r.set_sample_rate(sample_rate);
        // Recalculate HP coefficients for new sample rate
        self.set_feedback_hp(self.feedback_hp_freq);
        self.tempo.set_sample_rate(sample_rate);
        if self.sync {
            self.apply_synced_time();
        }
    }

    fn reset(&mut self) {
        self.delay_line.clear();
        self.delay_line_r.clear();
        self.delay_time.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();
        self.feedback_lp_l.reset();
        self.feedback_lp_r.reset();
        self.feedback_hp_l.clear();
        self.feedback_hp_r.clear();
        self.diffusion_ap1_l.clear();
        self.diffusion_ap2_l.clear();
        self.diffusion_ap1_r.clear();
        self.diffusion_ap2_r.clear();
    }
}

sonido_core::impl_params! {
    Delay, this {
        [0] ParamDescriptor::time_ms("Delay Time", "Time", 1.0, 2000.0, 300.0)
                .with_id(ParamId(1100), "dly_time"),
            get: this.delay_time.target() / this.sample_rate * 1000.0,
            set: |v| this.set_delay_time_ms(v);

        [1] ParamDescriptor {
                    name: "Feedback",
                    short_name: "Feedback",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 95.0,
                    default: 40.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1101), "dly_feedback"),
            get: this.feedback.target() * 100.0,
            set: |v| this.set_feedback(v / 100.0);

        [2] ParamDescriptor::mix()
                .with_id(ParamId(1102), "dly_mix"),
            get: this.mix.target() * 100.0,
            set: |v| this.set_mix(v / 100.0);

        [3] ParamDescriptor {
                    name: "Ping Pong",
                    short_name: "PngPng",
                    unit: ParamUnit::None,
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1103), "dly_ping_pong")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.ping_pong { 1.0 } else { 0.0 },
            set: |v| this.set_ping_pong(v > 0.5);

        [4] ParamDescriptor::custom("Feedback LP", "Fb LP", 200.0, 20000.0, 20000.0)
                .with_id(ParamId(1105), "dly_fb_lp")
                .with_unit(ParamUnit::Hertz)
                .with_scale(ParamScale::Logarithmic),
            get: this.feedback_lp_freq,
            set: |v| this.set_feedback_lp(v);

        [5] ParamDescriptor::custom("Feedback HP", "Fb HP", 20.0, 2000.0, 20.0)
                .with_id(ParamId(1106), "dly_fb_hp")
                .with_unit(ParamUnit::Hertz)
                .with_scale(ParamScale::Logarithmic),
            get: this.feedback_hp_freq,
            set: |v| this.set_feedback_hp(v);

        [6] ParamDescriptor {
                    name: "Diffusion",
                    short_name: "Diff",
                    unit: ParamUnit::Percent,
                    min: 0.0,
                    max: 100.0,
                    default: 0.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1107), "dly_diffusion"),
            get: this.diffusion * 100.0,
            set: |v| this.set_diffusion(v / 100.0);

        [7] ParamDescriptor {
                    name: "Sync",
                    short_name: "Sync",
                    unit: ParamUnit::None,
                    min: 0.0,
                    max: 1.0,
                    default: 0.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1108), "dly_sync")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.sync { 1.0 } else { 0.0 },
            set: |v| this.set_sync(v > 0.5);

        [8] ParamDescriptor {
                    name: "Division",
                    short_name: "Div",
                    unit: ParamUnit::None,
                    min: 0.0,
                    max: 11.0,
                    default: 2.0,
                    step: 1.0,
                    ..ParamDescriptor::mix()
                }
                .with_id(ParamId(1109), "dly_division")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(DIVISION_LABELS),
            get: division_to_index(this.division) as f32,
            set: |v| this.set_division(index_to_division(v as u8));

        [9] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1104), "dly_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_delay_basic() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_mix(1.0);
        delay.reset();

        // Process impulse
        delay.process(1.0);

        // Look for delayed impulse (threshold accounts for feedback_wet_compensation)
        let mut found = false;
        for _ in 0..5000 {
            if delay.process(0.0) > 0.5 {
                found = true;
                break;
            }
        }
        assert!(found, "Should find delayed impulse");
    }

    #[test]
    fn test_delay_bypass() {
        let mut delay = Delay::new(44100.0);
        delay.set_mix(0.0);
        delay.reset();

        let output = delay.process(0.5);
        assert!((output - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_delay_stereo_processing() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_mix(1.0);
        delay.reset();

        // Process stereo impulse
        delay.process_stereo(1.0, 0.5);

        // Look for delayed impulse on both channels
        let mut found_l = false;
        let mut found_r = false;
        for _ in 0..5000 {
            let (l, r) = delay.process_stereo(0.0, 0.0);
            if l > 0.5 {
                found_l = true;
            }
            if r > 0.2 {
                found_r = true;
            }
            if found_l && found_r {
                break;
            }
        }
        assert!(found_l, "Should find delayed impulse on left");
        assert!(found_r, "Should find delayed impulse on right");
    }

    #[test]
    fn test_delay_ping_pong() {
        let mut delay = Delay::new(44100.0);
        delay.set_delay_time_ms(100.0);
        delay.set_feedback(0.8);
        delay.set_mix(1.0);
        delay.set_ping_pong(true);
        delay.reset();

        assert!(delay.ping_pong());
        assert!(delay.is_true_stereo());

        // Process impulse on left only
        delay.process_stereo(1.0, 0.0);

        // With ping-pong, the feedback should cross channels
        let mut first_l_echo = false;
        let mut first_r_echo = false;
        for _i in 0..15000 {
            let (l, r) = delay.process_stereo(0.0, 0.0);
            if !first_l_echo && l.abs() > 0.1 {
                first_l_echo = true;
            }
            if first_l_echo && r.abs() > 0.05 {
                first_r_echo = true;
                break;
            }
            if first_l_echo && first_r_echo {
                break;
            }
        }
        assert!(first_l_echo, "Should find first echo on left");
        assert!(
            first_r_echo,
            "Ping-pong should produce echo on right channel from left input"
        );
    }

    #[test]
    fn test_delay_param_count() {
        let delay = Delay::new(44100.0);
        assert_eq!(delay.param_count(), 10);
    }

    #[test]
    fn test_feedback_lp_darkens_repeats() {
        let mut delay = Delay::new(48000.0);
        delay.set_delay_time_ms(50.0);
        delay.set_feedback(0.9);
        delay.set_mix(1.0);
        delay.set_feedback_lp(500.0); // aggressive LP
        delay.reset();

        // Feed a high-frequency impulse (alternating samples)
        for _ in 0..10 {
            delay.process(1.0);
            delay.process(-1.0);
        }

        // Run for a while to let repeats decay through the LP filter
        let mut energy = 0.0f32;
        for _ in 0..10000 {
            let out = delay.process(0.0);
            energy += out * out;
        }

        // Compare with unfiltered delay
        let mut delay2 = Delay::new(48000.0);
        delay2.set_delay_time_ms(50.0);
        delay2.set_feedback(0.9);
        delay2.set_mix(1.0);
        delay2.reset();

        for _ in 0..10 {
            delay2.process(1.0);
            delay2.process(-1.0);
        }

        let mut energy2 = 0.0f32;
        for _ in 0..10000 {
            let out = delay2.process(0.0);
            energy2 += out * out;
        }

        // Filtered version should have less energy (HF content removed)
        assert!(
            energy < energy2,
            "LP-filtered feedback should have less energy: {energy} vs {energy2}"
        );
    }

    #[test]
    fn test_feedback_hp_thins_repeats() {
        let mut delay = Delay::new(48000.0);
        delay.set_delay_time_ms(50.0);
        delay.set_feedback(0.9);
        delay.set_mix(1.0);
        delay.set_feedback_hp(1000.0); // aggressive HP
        delay.reset();

        // Feed DC-like low-frequency content
        for _ in 0..100 {
            delay.process(1.0);
        }

        // Run and collect energy
        let mut energy = 0.0f32;
        for _ in 0..10000 {
            let out = delay.process(0.0);
            energy += out * out;
        }

        // Compare with unfiltered
        let mut delay2 = Delay::new(48000.0);
        delay2.set_delay_time_ms(50.0);
        delay2.set_feedback(0.9);
        delay2.set_mix(1.0);
        delay2.reset();

        for _ in 0..100 {
            delay2.process(1.0);
        }

        let mut energy2 = 0.0f32;
        for _ in 0..10000 {
            let out = delay2.process(0.0);
            energy2 += out * out;
        }

        assert!(
            energy < energy2,
            "HP-filtered feedback should have less LF energy: {energy} vs {energy2}"
        );
    }

    #[test]
    fn test_diffusion_alters_feedback() {
        // Allpass diffusion preserves energy but spreads it temporally.
        // Verify by collecting the full output and checking that diffused
        // output differs from clean (non-zero difference in later samples).
        let sr = 48000.0;
        let n = 10000;

        let mut diffused = Delay::new(sr);
        diffused.set_delay_time_ms(50.0);
        diffused.set_feedback(0.7);
        diffused.set_mix(1.0);
        diffused.set_diffusion(1.0);
        diffused.reset();

        let mut clean = Delay::new(sr);
        clean.set_delay_time_ms(50.0);
        clean.set_feedback(0.7);
        clean.set_mix(1.0);
        clean.reset();

        diffused.process(1.0);
        clean.process(1.0);

        let mut diff_sum = 0.0f32;
        for _ in 0..n {
            let d = diffused.process(0.0);
            let c = clean.process(0.0);
            diff_sum += (d - c).abs();
        }

        assert!(
            diff_sum > 0.1,
            "Diffusion should produce different output from clean delay, diff_sum={diff_sum}"
        );
    }

    #[test]
    fn test_tempo_sync() {
        let mut delay = Delay::new(48000.0);
        delay.set_sync(true);
        delay.set_division(NoteDivision::Quarter);
        // At 120 BPM, quarter note = 500 ms
        delay.set_tempo(120.0);

        let expected_ms = 500.0;
        let actual_ms = delay.delay_time.target() / delay.sample_rate * 1000.0;
        assert!(
            (actual_ms - expected_ms).abs() < 1.0,
            "Quarter note at 120 BPM should be ~500ms, got {actual_ms}"
        );
    }

    #[test]
    fn test_tempo_sync_dotted_eighth() {
        let mut delay = Delay::new(48000.0);
        delay.set_sync(true);
        delay.set_division(NoteDivision::DottedEighth);
        delay.set_tempo(120.0);

        // Dotted eighth at 120 BPM = 375 ms
        let expected_ms = 375.0;
        let actual_ms = delay.delay_time.target() / delay.sample_rate * 1000.0;
        assert!(
            (actual_ms - expected_ms).abs() < 1.0,
            "Dotted eighth at 120 BPM should be ~375ms, got {actual_ms}"
        );
    }

    #[test]
    fn test_tempo_change_updates_delay() {
        let mut delay = Delay::new(48000.0);
        delay.set_sync(true);
        delay.set_division(NoteDivision::Quarter);
        delay.set_tempo(120.0);

        let ms_120 = delay.delay_time.target() / delay.sample_rate * 1000.0;

        delay.set_tempo(60.0);
        let ms_60 = delay.delay_time.target() / delay.sample_rate * 1000.0;

        // Half the BPM = double the delay time
        assert!(
            (ms_60 - ms_120 * 2.0).abs() < 1.0,
            "Halving BPM should double delay: {ms_60} vs {}",
            ms_120 * 2.0
        );
    }

    #[test]
    fn test_sync_off_ignores_tempo() {
        let mut delay = Delay::new(48000.0);
        delay.set_delay_time_ms(300.0);
        delay.set_sync(false);

        let before = delay.delay_time.target();
        delay.set_tempo(60.0); // should not affect delay time
        let after = delay.delay_time.target();

        assert!(
            (before - after).abs() < 0.01,
            "Sync off: tempo change should not affect delay time"
        );
    }

    #[test]
    fn test_division_labels() {
        let delay = Delay::new(48000.0);
        let info = delay.param_info(8).unwrap();
        assert_eq!(info.step_labels, Some(DIVISION_LABELS));
        assert_eq!(info.format_value(0.0), "Whole");
        assert_eq!(info.format_value(2.0), "Quarter");
        assert_eq!(info.format_value(8.0), "Dot 8th");
    }

    #[test]
    fn test_division_roundtrip() {
        for i in 0..12u8 {
            let div = index_to_division(i);
            let back = division_to_index(div);
            assert_eq!(i, back, "Roundtrip failed for index {i}");
        }
    }
}
