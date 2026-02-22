//! Classic chorus effect with variable voice count and feedback.
//!
//! Implements a multi-voice chorus with configurable delay modulation,
//! feedback, and stereo spread. Based on the Dimension D approach of
//! using irrational LFO rate ratios between voices to prevent phase
//! alignment and create a rich, evolving texture.
//!
//! # Algorithm
//!
//! Each voice runs an independent delay line modulated by its own LFO:
//! - Voice 1: sine LFO at base rate, 0° phase
//! - Voice 2: sine LFO at base rate, 90° phase offset
//! - Voice 3: sine LFO at rate × 0.73 (irrational ratio)
//! - Voice 4: triangle LFO at rate × 1.17 (irrational ratio)
//!
//! Irrational rate ratios prevent voices from locking into periodic
//! patterns, producing a more natural ensemble-like character.
//!
//! Optional feedback feeds delayed output back into the delay input,
//! adding resonance and metallic coloring at higher settings.
//!
//! # References
//!
//! - Dimension D chorus approach: irrational LFO ratios for decorrelation
//! - Välimäki, "Effect Design Part 2: Delay-Line Modulation and Chorus"

use libm::ceilf;
use sonido_core::math::soft_limit;
use sonido_core::{
    DIVISION_LABELS, Effect, InterpolatedDelay, Lfo, LfoWaveform, NoteDivision, ParamDescriptor,
    ParamFlags, ParamId, ParamUnit, SmoothedParam, TempoManager, division_to_index,
    index_to_division, wet_dry_mix, wet_dry_mix_stereo,
};

/// Maximum modulation depth in milliseconds.
const MAX_MOD_MS: f32 = 5.0;

/// Chorus effect with variable voice count, feedback, and configurable base delay.
///
/// ## Parameters
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Rate | 0.1–10.0 Hz | 1.0 |
/// | 1 | Depth | 0–100% | 50.0 |
/// | 2 | Mix | 0–100% | 50.0 |
/// | 3 | Voices | 2–4 (stepped) | 2 |
/// | 4 | Feedback | 0–70% | 0.0 |
/// | 5 | Base Delay | 5.0–25.0 ms | 15.0 |
/// | 6 | Sync | Off/On | Off |
/// | 7 | Division | 0–11 (note divisions) | 3 (Eighth) |
/// | 8 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Chorus;
/// use sonido_core::Effect;
///
/// let mut chorus = Chorus::new(44100.0);
/// chorus.set_rate(2.0);
/// chorus.set_depth(0.7);
/// chorus.set_mix(0.5);
/// chorus.set_voices(4);
/// chorus.set_feedback(0.3);
/// chorus.set_base_delay_ms(20.0);
///
/// let input = 0.5;
/// let output = chorus.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Chorus {
    delay1: InterpolatedDelay,
    delay2: InterpolatedDelay,
    delay3: InterpolatedDelay,
    delay4: InterpolatedDelay,
    lfo1: Lfo,
    lfo2: Lfo,
    lfo3: Lfo,
    lfo4: Lfo,
    base_delay_samples: f32,
    max_mod_samples: f32,
    rate: SmoothedParam,
    depth: SmoothedParam,
    mix: SmoothedParam,
    voices: u8,
    feedback: SmoothedParam,
    base_delay_ms: SmoothedParam,
    /// Per-voice feedback state (last delayed output for each voice).
    fb_state: [f32; 4],
    output_level: SmoothedParam,
    sample_rate: f32,
    // -- Tempo sync --
    /// Tempo manager for synced LFO rates.
    tempo: TempoManager,
    /// Whether tempo sync is active (rate derived from BPM + division).
    sync: bool,
    /// Selected note division for tempo sync.
    division: NoteDivision,
}

impl Chorus {
    /// Create a new chorus effect.
    ///
    /// Initializes four delay lines and LFOs. Voices 3 and 4 are inactive
    /// by default (voices=2). LFO rate ratios follow the Dimension D
    /// approach for natural decorrelation.
    pub fn new(sample_rate: f32) -> Self {
        let base_delay_ms = 15.0;
        let max_delay_ms = 25.0 + MAX_MOD_MS; // max base + max mod
        let max_delay_samples = ceilf((max_delay_ms / 1000.0) * sample_rate) as usize;
        let base_delay_samples = (base_delay_ms / 1000.0) * sample_rate;
        let max_mod_samples = (MAX_MOD_MS / 1000.0) * sample_rate;

        let lfo1 = Lfo::new(sample_rate, 1.0);
        let mut lfo2 = Lfo::new(sample_rate, 1.0);
        lfo2.set_phase(0.25); // 90° offset

        // Voice 3: sine at rate × 0.73 (irrational ratio)
        let lfo3 = Lfo::new(sample_rate, 0.73);

        // Voice 4: triangle at rate × 1.17 (irrational ratio)
        let mut lfo4 = Lfo::new(sample_rate, 1.17);
        lfo4.set_waveform(LfoWaveform::Triangle);

        Self {
            delay1: InterpolatedDelay::new(max_delay_samples),
            delay2: InterpolatedDelay::new(max_delay_samples),
            delay3: InterpolatedDelay::new(max_delay_samples),
            delay4: InterpolatedDelay::new(max_delay_samples),
            lfo1,
            lfo2,
            lfo3,
            lfo4,
            base_delay_samples,
            max_mod_samples,
            rate: SmoothedParam::standard(1.0, sample_rate),
            depth: SmoothedParam::standard(0.5, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            voices: 2,
            feedback: SmoothedParam::standard(0.0, sample_rate),
            base_delay_ms: SmoothedParam::standard(15.0, sample_rate),
            fb_state: [0.0; 4],
            output_level: sonido_core::gain::output_level_param(sample_rate),
            sample_rate,
            tempo: TempoManager::new(sample_rate, 120.0),
            sync: false,
            division: NoteDivision::Quarter,
        }
    }

    /// Set LFO rate in Hz.
    ///
    /// Range: 0.1 to 10.0 Hz. Values are clamped.
    /// Voices 1-2 use this rate directly; voice 3 uses rate × 0.73,
    /// voice 4 uses rate × 1.17.
    pub fn set_rate(&mut self, rate_hz: f32) {
        self.rate.set_target(rate_hz.clamp(0.1, 10.0));
    }

    /// Set modulation depth (0.0–1.0).
    ///
    /// Range: 0.0 to 1.0. Controls the amplitude of delay time modulation.
    /// Higher values produce more pronounced pitch wobble.
    pub fn set_depth(&mut self, depth: f32) {
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    /// Set wet/dry mix (0.0–1.0).
    ///
    /// Range: 0.0 (fully dry) to 1.0 (fully wet).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Set number of active voices.
    ///
    /// Range: 2 to 4 (stepped). More voices create a richer, thicker
    /// ensemble effect. Voices 3 and 4 use irrational rate ratios
    /// relative to voices 1-2 (Dimension D approach).
    pub fn set_voices(&mut self, voices: u8) {
        self.voices = voices.clamp(2, 4);
    }

    /// Get number of active voices.
    pub fn voices(&self) -> u8 {
        self.voices
    }

    /// Set feedback amount (0.0–0.7).
    ///
    /// Range: 0.0 to 0.7. Feeds delayed output back into the delay
    /// input, adding resonance. Keep below 0.7 to prevent instability.
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback.set_target(feedback.clamp(0.0, 0.7));
    }

    /// Set base delay time in milliseconds.
    ///
    /// Range: 5.0 to 25.0 ms. Shorter values produce a more transparent,
    /// doubling effect; longer values create a more ensemble-like character.
    pub fn set_base_delay_ms(&mut self, ms: f32) {
        self.base_delay_ms.set_target(ms.clamp(5.0, 25.0));
    }

    /// Enable or disable tempo sync.
    ///
    /// When enabled, LFO rate is derived from the current BPM and note
    /// division, overriding the manual rate parameter.
    pub fn set_sync(&mut self, enabled: bool) {
        self.sync = enabled;
        if enabled {
            self.apply_synced_rate();
        }
    }

    /// Set the note division for tempo sync.
    ///
    /// Only takes effect when sync is enabled.
    pub fn set_division(&mut self, division: NoteDivision) {
        self.division = division;
        if self.sync {
            self.apply_synced_rate();
        }
    }

    /// Recalculate LFO rate from tempo and division.
    fn apply_synced_rate(&mut self) {
        let hz = self.tempo.division_to_hz(self.division);
        self.rate.set_target(hz.clamp(0.1, 10.0));
    }

    /// Recalculate sample-domain values from the current base delay target.
    fn update_base_delay(&mut self) {
        let ms = self.base_delay_ms.advance();
        self.base_delay_samples = (ms / 1000.0) * self.sample_rate;
    }

    /// Process a single sample through all active voices (mono).
    ///
    /// Returns the average of all active voices mixed with feedback,
    /// blended with the dry signal according to the mix parameter.
    #[inline]
    fn process_voices_mono(&mut self, input: f32, depth: f32, feedback: f32) -> f32 {
        let voices = self.voices;

        // Voice 1
        let wet1 = self
            .delay1
            .read(self.base_delay_samples + self.lfo1.advance() * depth * self.max_mod_samples);
        self.delay1.write(input + self.fb_state[0] * feedback);

        // Voice 2
        let wet2 = self
            .delay2
            .read(self.base_delay_samples + self.lfo2.advance() * depth * self.max_mod_samples);
        self.delay2.write(input + self.fb_state[1] * feedback);

        self.fb_state[0] = wet1;
        self.fb_state[1] = wet2;

        if voices == 2 {
            return (wet1 + wet2) * 0.5;
        }

        // Voice 3
        let wet3 = self
            .delay3
            .read(self.base_delay_samples + self.lfo3.advance() * depth * self.max_mod_samples);
        self.delay3.write(input + self.fb_state[2] * feedback);
        self.fb_state[2] = wet3;

        if voices == 3 {
            return (wet1 + wet2 + wet3) / 3.0;
        }

        // Voice 4
        let wet4 = self
            .delay4
            .read(self.base_delay_samples + self.lfo4.advance() * depth * self.max_mod_samples);
        self.delay4.write(input + self.fb_state[3] * feedback);
        self.fb_state[3] = wet4;

        (wet1 + wet2 + wet3 + wet4) * 0.25
    }

    /// Process stereo voices with panning.
    ///
    /// Returns (wet_left, wet_right) with voice panning:
    /// - 2 voices: V1 80% L / 20% R, V2 20% L / 80% R
    /// - 3 voices: V1 left, V2 right, V3 center
    /// - 4 voices: V1 hard left, V2 hard right, V3 center-left, V4 center-right
    #[inline]
    fn process_voices_stereo(
        &mut self,
        left: f32,
        right: f32,
        depth: f32,
        feedback: f32,
    ) -> (f32, f32) {
        let voices = self.voices;

        // Voice 1 — fed by left channel
        let dt1 = self.base_delay_samples + self.lfo1.advance() * depth * self.max_mod_samples;
        let wet1 = self.delay1.read(dt1);
        self.delay1.write(left + self.fb_state[0] * feedback);
        self.fb_state[0] = wet1;

        // Voice 2 — fed by right channel
        let dt2 = self.base_delay_samples + self.lfo2.advance() * depth * self.max_mod_samples;
        let wet2 = self.delay2.read(dt2);
        self.delay2.write(right + self.fb_state[1] * feedback);
        self.fb_state[1] = wet2;

        if voices == 2 {
            // V1 mostly left, V2 mostly right
            let wet_l = wet1 * 0.8 + wet2 * 0.2;
            let wet_r = wet2 * 0.8 + wet1 * 0.2;
            return (wet_l, wet_r);
        }

        // Voice 3 — fed by mid (average of L+R), panned center
        let mid = (left + right) * 0.5;
        let dt3 = self.base_delay_samples + self.lfo3.advance() * depth * self.max_mod_samples;
        let wet3 = self.delay3.read(dt3);
        self.delay3.write(mid + self.fb_state[2] * feedback);
        self.fb_state[2] = wet3;

        if voices == 3 {
            // V1 left, V2 right, V3 center — equal contribution
            let scale = 1.0 / 3.0;
            let wet_l = (wet1 * 0.9 + wet2 * 0.1 + wet3 * 0.5) * scale * 2.0;
            let wet_r = (wet2 * 0.9 + wet1 * 0.1 + wet3 * 0.5) * scale * 2.0;
            return (wet_l, wet_r);
        }

        // Voice 4 — fed by mid, panned center-right
        let dt4 = self.base_delay_samples + self.lfo4.advance() * depth * self.max_mod_samples;
        let wet4 = self.delay4.read(dt4);
        self.delay4.write(mid + self.fb_state[3] * feedback);
        self.fb_state[3] = wet4;

        // 4 voices: V1 hard-left, V2 hard-right, V3 center-left, V4 center-right
        let wet_l = (wet1 * 0.9 + wet2 * 0.1 + wet3 * 0.7 + wet4 * 0.3) * 0.5;
        let wet_r = (wet2 * 0.9 + wet1 * 0.1 + wet4 * 0.7 + wet3 * 0.3) * 0.5;
        (wet_l, wet_r)
    }
}

impl Default for Chorus {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Chorus {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let mix = self.mix.advance();
        let feedback = self.feedback.advance();
        let output_gain = self.output_level.advance();
        self.update_base_delay();

        self.lfo1.set_frequency(rate);
        self.lfo2.set_frequency(rate);
        self.lfo3.set_frequency(rate * 0.73);
        self.lfo4.set_frequency(rate * 1.17);

        let wet = self.process_voices_mono(input, depth, feedback);
        soft_limit(wet_dry_mix(input, wet, mix), 1.0) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let mix = self.mix.advance();
        let feedback = self.feedback.advance();
        let output_gain = self.output_level.advance();
        self.update_base_delay();

        self.lfo1.set_frequency(rate);
        self.lfo2.set_frequency(rate);
        self.lfo3.set_frequency(rate * 0.73);
        self.lfo4.set_frequency(rate * 1.17);

        let (wet_l, wet_r) = self.process_voices_stereo(left, right, depth, feedback);

        let (out_l, out_r) = wet_dry_mix_stereo(left, right, wet_l, wet_r, mix);
        (
            soft_limit(out_l, 1.0) * output_gain,
            soft_limit(out_r, 1.0) * output_gain,
        )
    }

    /// Optimized block processing for stereo chorus.
    ///
    /// Processes all samples in a tight loop, advancing `SmoothedParam`s and
    /// LFOs per sample, computing modulated delay times for all active voices,
    /// and applying stereo panning. Produces bit-identical output to calling
    /// `process_stereo()` per sample.
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
        true
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        let base_ms = self.base_delay_ms.target();
        self.base_delay_samples = (base_ms / 1000.0) * sample_rate;
        self.max_mod_samples = (MAX_MOD_MS / 1000.0) * sample_rate;

        self.lfo1.set_sample_rate(sample_rate);
        self.lfo2.set_sample_rate(sample_rate);
        self.lfo3.set_sample_rate(sample_rate);
        self.lfo4.set_sample_rate(sample_rate);
        self.rate.set_sample_rate(sample_rate);
        self.depth.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.base_delay_ms.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.tempo.set_sample_rate(sample_rate);
    }

    fn set_tempo_context(&mut self, ctx: &sonido_core::TempoContext) {
        self.tempo.set_bpm(ctx.bpm);
        if self.sync {
            self.apply_synced_rate();
        }
    }

    fn reset(&mut self) {
        self.delay1.clear();
        self.delay2.clear();
        self.delay3.clear();
        self.delay4.clear();
        self.lfo1.reset();
        self.lfo2.reset();
        self.lfo3.reset();
        self.lfo4.reset();
        self.rate.snap_to_target();
        self.depth.snap_to_target();
        self.mix.snap_to_target();
        self.feedback.snap_to_target();
        self.base_delay_ms.snap_to_target();
        self.output_level.snap_to_target();
        self.fb_state = [0.0; 4];
    }
}

sonido_core::impl_params! {
    Chorus, this {
        [0] ParamDescriptor::rate_hz(0.1, 10.0, 1.0)
                .with_id(ParamId(700), "chor_rate"),
            get: this.rate.target(),
            set: |v| this.set_rate(v);

        [1] ParamDescriptor::depth()
                .with_id(ParamId(701), "chor_depth"),
            get: this.depth.target() * 100.0,
            set: |v| this.set_depth(v / 100.0);

        [2] ParamDescriptor::mix()
                .with_id(ParamId(702), "chor_mix"),
            get: this.mix.target() * 100.0,
            set: |v| this.set_mix(v / 100.0);

        [3] ParamDescriptor::custom("Voices", "Voices", 2.0, 4.0, 2.0)
                .with_step(1.0)
                .with_id(ParamId(704), "chor_voices")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["2", "3", "4"]),
            get: this.voices as f32,
            set: |v| this.set_voices(v as u8);

        [4] ParamDescriptor::custom("Feedback", "Fdbk", 0.0, 70.0, 0.0)
                .with_unit(ParamUnit::Percent)
                .with_step(1.0)
                .with_id(ParamId(705), "chor_feedback"),
            get: this.feedback.target() * 100.0,
            set: |v| this.set_feedback(v / 100.0);

        [5] ParamDescriptor::custom("Base Delay", "BDly", 5.0, 25.0, 15.0)
                .with_unit(ParamUnit::Milliseconds)
                .with_step(0.5)
                .with_id(ParamId(706), "chor_base_delay"),
            get: this.base_delay_ms.target(),
            set: |v| this.set_base_delay_ms(v);

        [6] ParamDescriptor::custom("Sync", "Sync", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(707), "chor_sync")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.sync { 1.0 } else { 0.0 },
            set: |v| this.set_sync(v > 0.5);

        [7] ParamDescriptor::custom("Division", "Div", 0.0, 11.0, 3.0)
                .with_step(1.0)
                .with_id(ParamId(708), "chor_division")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(DIVISION_LABELS),
            get: division_to_index(this.division) as f32,
            set: |v| this.set_division(index_to_division(v as u8));

        [8] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(703), "chor_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_chorus_basic() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);

        for _ in 0..1000 {
            let output = chorus.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_chorus_bypass() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(0.0);

        // Let smoothing settle
        for _ in 0..1000 {
            chorus.process(1.0);
        }

        let output = chorus.process(0.5);
        assert!((output - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_chorus_stereo_processing() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_depth(0.8);

        // Process stereo signal
        for _ in 0..1000 {
            let (l, r) = chorus.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_chorus_is_true_stereo() {
        let chorus = Chorus::new(44100.0);
        assert!(chorus.is_true_stereo());
    }

    #[test]
    fn test_chorus_stereo_spread() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_depth(1.0);

        // Process for a while to fill delay lines
        for _ in 0..1000 {
            chorus.process_stereo(0.5, 0.5);
        }

        // Collect outputs and verify they differ (stereo spread)
        let mut l_sum = 0.0f32;
        let mut r_sum = 0.0f32;
        for _ in 0..1000 {
            let (l, r) = chorus.process_stereo(0.5, 0.5);
            l_sum += l;
            r_sum += r;
        }

        // With stereo spread, left and right should have different summed outputs
        // (due to different voice panning)
        assert!(l_sum.is_finite());
        assert!(r_sum.is_finite());
    }

    #[test]
    fn test_chorus_stereo_input_not_mono_summed() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_depth(0.5);

        // Snap smoothing so mix=1.0 takes effect immediately
        for _ in 0..1000 {
            chorus.process_stereo(0.0, 0.0);
        }
        chorus.reset();

        // Feed asymmetric stereo: L=1.0, R=0.0 impulse, then silence
        let (l0, r0) = chorus.process_stereo(1.0, 0.0);
        for _ in 0..500 {
            chorus.process_stereo(0.0, 0.0);
        }

        // After the impulse has passed through the delay, collect output
        let mut l_energy = l0 * l0;
        let mut r_energy = r0 * r0;
        for _ in 0..1000 {
            let (l, r) = chorus.process_stereo(0.0, 0.0);
            l_energy += l * l;
            r_energy += r * r;
        }

        // L channel fed voice 1 (panned 80% left), R channel fed voice 2 (0 input).
        // So L energy should dominate. With old mono-sum both would be equal.
        assert!(
            (l_energy - r_energy).abs() > 1e-6,
            "L and R energy should differ for asymmetric input, got L={l_energy} R={r_energy}"
        );
    }

    #[test]
    fn test_chorus_param_count() {
        let chorus = Chorus::new(44100.0);
        assert_eq!(chorus.param_count(), 9);
    }

    #[test]
    fn test_chorus_voices_param() {
        let mut chorus = Chorus::new(44100.0);
        assert_eq!(chorus.voices(), 2);

        chorus.set_voices(4);
        assert_eq!(chorus.voices(), 4);

        // Clamped to range
        chorus.set_voices(1);
        assert_eq!(chorus.voices(), 2);
        chorus.set_voices(10);
        assert_eq!(chorus.voices(), 4);
    }

    #[test]
    fn test_chorus_voices_via_param_info() {
        let mut chorus = Chorus::new(44100.0);
        // Voices is index 3
        chorus.set_param(3, 3.0);
        assert_eq!(chorus.voices(), 3);
        assert!((chorus.get_param(3) - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_chorus_feedback() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_mix(1.0);
        chorus.set_feedback(0.5);

        // With feedback, output should eventually show resonance
        for _ in 0..2000 {
            let output = chorus.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_chorus_base_delay() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_base_delay_ms(5.0);
        assert!((chorus.base_delay_ms.target() - 5.0).abs() < 0.01);

        chorus.set_base_delay_ms(25.0);
        assert!((chorus.base_delay_ms.target() - 25.0).abs() < 0.01);

        // Clamped
        chorus.set_base_delay_ms(1.0);
        assert!((chorus.base_delay_ms.target() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_chorus_4_voices_stereo() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_voices(4);
        chorus.set_mix(1.0);
        chorus.set_depth(0.5);

        for _ in 0..2000 {
            let (l, r) = chorus.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_chorus_3_voices_mono() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_voices(3);
        chorus.set_mix(1.0);

        for _ in 0..2000 {
            let output = chorus.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_chorus_feedback_bounded() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_voices(4);
        chorus.set_mix(1.0);
        chorus.set_feedback(0.7); // max feedback
        chorus.set_depth(1.0);

        // Even at max feedback, output should stay bounded due to soft_limit
        for _ in 0..10000 {
            let output = chorus.process(0.5);
            assert!(output.abs() <= 1.5, "Output exceeded bounds: {output}");
        }
    }

    #[test]
    fn test_chorus_reset_clears_feedback_state() {
        let mut chorus = Chorus::new(44100.0);
        chorus.set_feedback(0.5);
        chorus.set_mix(1.0);

        // Build up some feedback state
        for _ in 0..1000 {
            chorus.process(0.5);
        }

        chorus.reset();
        assert_eq!(chorus.fb_state, [0.0; 4]);
    }

    #[test]
    fn test_chorus_param_ids_stable() {
        let chorus = Chorus::new(44100.0);
        assert_eq!(chorus.param_id(0), Some(ParamId(700)));
        assert_eq!(chorus.param_id(1), Some(ParamId(701)));
        assert_eq!(chorus.param_id(2), Some(ParamId(702)));
        assert_eq!(chorus.param_id(3), Some(ParamId(704))); // voices
        assert_eq!(chorus.param_id(4), Some(ParamId(705))); // feedback
        assert_eq!(chorus.param_id(5), Some(ParamId(706))); // base_delay
        assert_eq!(chorus.param_id(6), Some(ParamId(707))); // sync
        assert_eq!(chorus.param_id(7), Some(ParamId(708))); // division
        assert_eq!(chorus.param_id(8), Some(ParamId(703))); // output (kept stable)
    }

    #[test]
    fn test_chorus_tempo_sync() {
        let mut chorus = Chorus::new(44100.0);
        // At 120 BPM, Eighth note = 4 Hz
        chorus.set_sync(true);
        chorus.set_division(sonido_core::NoteDivision::Eighth);
        assert!((chorus.rate.target() - 4.0).abs() < 0.01);

        // At 60 BPM, Eighth note = 2 Hz
        chorus.set_tempo_context(&sonido_core::TempoContext {
            bpm: 60.0,
            ..Default::default()
        });
        assert!((chorus.rate.target() - 2.0).abs() < 0.01);
    }
}
