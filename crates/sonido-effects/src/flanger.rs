//! Classic flanger effect with modulated short delay.
//!
//! A flanger creates a characteristic "whooshing" or "jet plane" sound
//! by mixing the input signal with a short, modulated delay. The delay
//! time sweeps between approximately 1-10ms, creating comb filtering
//! effects that sweep through the frequency spectrum.
//!
//! ## Through-Zero Flanging (TZF)
//!
//! When TZF mode is enabled, the dry signal is routed through a fixed delay
//! equal to the sweep midpoint (`base_delay_samples`). The wet delay sweeps
//! from 0 to `2 * base_delay`, passing *through* the dry delay point. At the
//! zero crossing the wet and dry paths are time-aligned, producing the
//! characteristic "jet whoosh" null that classic tape-flanger circuits exhibit.
//! TZF mode reports latency equal to the fixed dry delay.
//!
//! ## Bipolar Feedback
//!
//! Feedback ranges from -0.95 to +0.95. Positive feedback reinforces odd
//! harmonics of the comb filter; negative feedback shifts the comb peaks to
//! even harmonics, producing a distinctly different tonal character.

use libm::ceilf;
use sonido_core::{
    DIVISION_LABELS, Effect, InterpolatedDelay, Lfo, NoteDivision, ParamDescriptor, ParamFlags,
    ParamId, ParamUnit, SmoothedParam, TempoManager, division_to_index, flush_denormal,
    impl_params, index_to_division, wet_dry_mix, wet_dry_mix_stereo,
};

/// Flanger effect with LFO-modulated delay and optional through-zero flanging.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Rate | 0.05–5.0 Hz | 0.5 |
/// | 1 | Depth | 0–100% | 35.0 |
/// | 2 | Feedback | -95–95% | 50.0 |
/// | 3 | Mix | 0–100% | 50.0 |
/// | 4 | TZF | Off/On | Off |
/// | 5 | Sync | Off/On | Off |
/// | 6 | Division | 0–11 (note divisions) | 3 (Eighth) |
/// | 7 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Flanger;
/// use sonido_core::Effect;
///
/// let mut flanger = Flanger::new(44100.0);
/// flanger.set_rate(0.5);
/// flanger.set_depth(0.8);
/// flanger.set_feedback(0.7);
/// flanger.set_mix(0.5);
///
/// let input = 0.5;
/// let output = flanger.process(input);
/// ```
///
/// ## Through-Zero Flanging
///
/// ```rust
/// use sonido_effects::Flanger;
/// use sonido_core::Effect;
///
/// let mut flanger = Flanger::new(44100.0);
/// flanger.set_tzf(true);
/// // Reports latency for host compensation
/// assert!(flanger.latency_samples() > 0);
/// ```
#[derive(Debug, Clone)]
pub struct Flanger {
    delay: InterpolatedDelay,
    delay_r: InterpolatedDelay,
    /// TZF dry-path delay (left channel).
    dry_delay_l: InterpolatedDelay,
    /// TZF dry-path delay (right channel).
    dry_delay_r: InterpolatedDelay,
    lfo: Lfo,
    lfo_r: Lfo,
    rate: SmoothedParam,
    depth: SmoothedParam,
    feedback: SmoothedParam,
    mix: SmoothedParam,
    output_level: SmoothedParam,
    sample_rate: f32,
    /// Base delay in samples (5ms).
    base_delay_samples: f32,
    /// Maximum modulation depth in samples (5ms).
    max_mod_samples: f32,
    /// Feedback sample for regeneration (left).
    feedback_sample: f32,
    /// Feedback sample for regeneration (right).
    feedback_sample_r: f32,
    /// Stereo spread (LFO phase offset 0-0.5).
    stereo_spread: f32,
    /// Through-zero flanging mode.
    tzf: bool,
    // -- Tempo sync --
    /// Tempo manager for synced LFO rates.
    tempo: TempoManager,
    /// Whether tempo sync is active (rate derived from BPM + division).
    sync: bool,
    /// Selected note division for tempo sync.
    division: NoteDivision,
}

impl Flanger {
    /// Base delay time in milliseconds.
    const BASE_DELAY_MS: f32 = 5.0;
    /// Maximum modulation depth in milliseconds.
    const MAX_MOD_MS: f32 = 5.0;
    /// Minimum delay time in milliseconds.
    const MIN_DELAY_MS: f32 = 1.0;

    /// Create a new flanger effect.
    pub fn new(sample_rate: f32) -> Self {
        // Maximum delay = base + max mod = 10ms
        let max_delay_ms = Self::BASE_DELAY_MS + Self::MAX_MOD_MS;
        let max_delay_samples = ceilf((max_delay_ms / 1000.0) * sample_rate) as usize + 1;

        let base_delay_samples = (Self::BASE_DELAY_MS / 1000.0) * sample_rate;
        let max_mod_samples = (Self::MAX_MOD_MS / 1000.0) * sample_rate;

        // TZF dry delay: fixed at base_delay_samples (the sweep midpoint)
        let dry_delay_size = ceilf(base_delay_samples) as usize + 1;

        let mut lfo_r = Lfo::new(sample_rate, 0.5);
        lfo_r.set_phase(0.25); // 90 degree offset

        Self {
            delay: InterpolatedDelay::new(max_delay_samples),
            delay_r: InterpolatedDelay::new(max_delay_samples),
            dry_delay_l: InterpolatedDelay::new(dry_delay_size),
            dry_delay_r: InterpolatedDelay::new(dry_delay_size),
            lfo: Lfo::new(sample_rate, 0.5),
            lfo_r,
            rate: SmoothedParam::standard(0.5, sample_rate),
            depth: SmoothedParam::standard(0.35, sample_rate),
            feedback: SmoothedParam::standard(0.5, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            sample_rate,
            base_delay_samples,
            max_mod_samples,
            feedback_sample: 0.0,
            feedback_sample_r: 0.0,
            stereo_spread: 0.25,
            tzf: false,
            tempo: TempoManager::new(sample_rate, 120.0),
            sync: false,
            division: NoteDivision::Quarter,
        }
    }

    /// Set LFO rate in Hz (0.05-5 Hz).
    pub fn set_rate(&mut self, rate_hz: f32) {
        self.rate.set_target(rate_hz.clamp(0.05, 5.0));
    }

    /// Get current LFO rate in Hz.
    pub fn rate(&self) -> f32 {
        self.rate.target()
    }

    /// Set modulation depth (0-1).
    pub fn set_depth(&mut self, depth: f32) {
        self.depth.set_target(depth.clamp(0.0, 1.0));
    }

    /// Get current modulation depth.
    pub fn depth(&self) -> f32 {
        self.depth.target()
    }

    /// Set feedback amount (-0.95 to 0.95).
    ///
    /// Positive values reinforce odd harmonics of the comb filter;
    /// negative values shift comb peaks to even harmonics.
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback.set_target(feedback.clamp(-0.95, 0.95));
    }

    /// Get current feedback amount.
    pub fn feedback(&self) -> f32 {
        self.feedback.target()
    }

    /// Set wet/dry mix (0-1).
    pub fn set_mix(&mut self, mix: f32) {
        self.mix.set_target(mix.clamp(0.0, 1.0));
    }

    /// Get current wet/dry mix.
    pub fn mix(&self) -> f32 {
        self.mix.target()
    }

    /// Enable or disable through-zero flanging.
    ///
    /// When enabled, the dry signal is routed through a fixed delay at the
    /// sweep midpoint. The wet delay sweeps through this point, producing
    /// the characteristic "jet whoosh" null of tape-flanger circuits.
    /// Reports latency equal to the fixed dry delay.
    pub fn set_tzf(&mut self, enabled: bool) {
        self.tzf = enabled;
    }

    /// Returns whether through-zero flanging is enabled.
    pub fn tzf(&self) -> bool {
        self.tzf
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
        self.rate.set_target(hz.clamp(0.05, 5.0));
    }
}

impl Default for Flanger {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Flanger {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Update LFO frequency
        self.lfo.set_frequency(rate);

        // LFO output is in range [-1, 1], convert to [0, 1] for delay modulation
        let lfo_value = self.lfo.advance_unipolar();

        // Calculate delay time: base delay + modulation
        let mod_amount = (lfo_value * 2.0 - 1.0) * depth * self.max_mod_samples;
        let min_delay = if self.tzf {
            0.0
        } else {
            (Self::MIN_DELAY_MS / 1000.0) * self.sample_rate
        };
        let delay_samples = (self.base_delay_samples + mod_amount).max(min_delay);

        // Read from delay line
        let delayed = self.delay.read(delay_samples);

        // Write input + feedback to delay line
        let delay_input = input + self.feedback_sample * feedback;
        self.delay.write(delay_input);

        // Store for next iteration
        self.feedback_sample = flush_denormal(delayed);

        // Dry signal: fixed delay in TZF mode, direct in normal mode
        let dry = if self.tzf {
            let d = self.dry_delay_l.read(self.base_delay_samples);
            self.dry_delay_l.write(input);
            d
        } else {
            input
        };

        // Mix dry and wet signals (use abs feedback for compensation)
        let comp = sonido_core::gain::feedback_wet_compensation(libm::fabsf(feedback));
        wet_dry_mix(dry, delayed * comp, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // True stereo: offset LFO phase between channels for stereo spread
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Update LFO frequencies
        self.lfo.set_frequency(rate);
        self.lfo_r.set_frequency(rate);

        // Get LFO values (with phase offset for right channel)
        let lfo_l = self.lfo.advance_unipolar();
        let lfo_r = self.lfo_r.advance_unipolar();

        let min_delay = if self.tzf {
            0.0
        } else {
            (Self::MIN_DELAY_MS / 1000.0) * self.sample_rate
        };

        // Calculate delay times for each channel
        let mod_l = (lfo_l * 2.0 - 1.0) * depth * self.max_mod_samples;
        let mod_r = (lfo_r * 2.0 - 1.0) * depth * self.max_mod_samples;
        let delay_l = (self.base_delay_samples + mod_l).max(min_delay);
        let delay_r = (self.base_delay_samples + mod_r).max(min_delay);

        // Read from delay lines
        let delayed_l = self.delay.read(delay_l);
        let delayed_r = self.delay_r.read(delay_r);

        // Write input + feedback to delay lines
        let input_l = left + self.feedback_sample * feedback;
        let input_r = right + self.feedback_sample_r * feedback;
        self.delay.write(input_l);
        self.delay_r.write(input_r);

        // Store for next iteration
        self.feedback_sample = flush_denormal(delayed_l);
        self.feedback_sample_r = flush_denormal(delayed_r);

        // Dry signals: fixed delay in TZF mode, direct in normal mode
        let (dry_l, dry_r) = if self.tzf {
            let dl = self.dry_delay_l.read(self.base_delay_samples);
            let dr = self.dry_delay_r.read(self.base_delay_samples);
            self.dry_delay_l.write(left);
            self.dry_delay_r.write(right);
            (dl, dr)
        } else {
            (left, right)
        };

        // Mix dry and wet signals (use abs feedback for compensation)
        let comp = sonido_core::gain::feedback_wet_compensation(libm::fabsf(feedback));
        let (out_l, out_r) =
            wet_dry_mix_stereo(dry_l, dry_r, delayed_l * comp, delayed_r * comp, mix);

        (out_l * output_gain, out_r * output_gain)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        self.base_delay_samples = (Self::BASE_DELAY_MS / 1000.0) * sample_rate;
        self.max_mod_samples = (Self::MAX_MOD_MS / 1000.0) * sample_rate;

        // Rebuild dry delay buffers for new sample rate
        let dry_delay_size = ceilf(self.base_delay_samples) as usize + 1;
        self.dry_delay_l = InterpolatedDelay::new(dry_delay_size);
        self.dry_delay_r = InterpolatedDelay::new(dry_delay_size);

        self.lfo.set_sample_rate(sample_rate);
        self.lfo_r.set_sample_rate(sample_rate);
        self.rate.set_sample_rate(sample_rate);
        self.depth.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
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
        self.delay.clear();
        self.delay_r.clear();
        self.dry_delay_l.clear();
        self.dry_delay_r.clear();
        self.lfo.reset();
        self.lfo_r.reset();
        self.lfo_r.set_phase(self.stereo_spread);
        self.feedback_sample = 0.0;
        self.feedback_sample_r = 0.0;
        self.rate.snap_to_target();
        self.depth.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();
    }

    fn latency_samples(&self) -> usize {
        if self.tzf {
            self.base_delay_samples as usize
        } else {
            0
        }
    }
}

impl_params! {
    Flanger, this {
        [0] ParamDescriptor::rate_hz(0.05, 5.0, 0.5)
                .with_id(ParamId(800), "flgr_rate"),
            get: this.rate.target(),
            set: |v| this.set_rate(v);

        [1] ParamDescriptor { default: 35.0, ..ParamDescriptor::depth() }
                .with_id(ParamId(801), "flgr_depth"),
            get: this.depth.target() * 100.0,
            set: |v| this.set_depth(v / 100.0);

        [2] ParamDescriptor::custom("Feedback", "Fdbk", -95.0, 95.0, 50.0)
                .with_unit(ParamUnit::Percent)
                .with_step(1.0)
                .with_id(ParamId(802), "flgr_fdbk"),
            get: this.feedback.target() * 100.0,
            set: |v| this.set_feedback(v / 100.0);

        [3] ParamDescriptor::mix()
                .with_id(ParamId(803), "flgr_mix"),
            get: this.mix.target() * 100.0,
            set: |v| this.set_mix(v / 100.0);

        [4] ParamDescriptor::custom("TZF", "TZF", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(805), "flgr_tzf")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.tzf { 1.0 } else { 0.0 },
            set: |v| this.set_tzf(v >= 0.5);

        [5] ParamDescriptor::custom("Sync", "Sync", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(806), "flgr_sync")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.sync { 1.0 } else { 0.0 },
            set: |v| this.set_sync(v > 0.5);

        [6] ParamDescriptor::custom("Division", "Div", 0.0, 11.0, 3.0)
                .with_step(1.0)
                .with_id(ParamId(807), "flgr_division")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(DIVISION_LABELS),
            get: division_to_index(this.division) as f32,
            set: |v| this.set_division(index_to_division(v as u8));

        [7] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(804), "flgr_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_flanger_basic() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_mix(1.0);

        for _ in 0..1000 {
            let output = flanger.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_flanger_bypass() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_mix(0.0);

        // Let smoothing settle
        for _ in 0..1000 {
            flanger.process(1.0);
        }

        let output = flanger.process(0.5);
        assert!((output - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_flanger_feedback_stability() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_feedback(0.95);
        flanger.set_mix(1.0);

        // Process many samples to check for instability
        for _ in 0..10000 {
            let output = flanger.process(0.1);
            assert!(output.is_finite());
            assert!(output.abs() < 10.0, "Output exceeded bounds: {output}");
        }
    }

    #[test]
    fn test_flanger_negative_feedback_stability() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_feedback(-0.95);
        flanger.set_mix(1.0);

        for _ in 0..10000 {
            let output = flanger.process(0.1);
            assert!(output.is_finite());
            assert!(output.abs() < 10.0, "Output exceeded bounds: {output}");
        }
    }

    #[test]
    fn test_flanger_bipolar_feedback_range() {
        let mut flanger = Flanger::new(44100.0);

        flanger.set_feedback(-0.8);
        assert!((flanger.feedback() - -0.8).abs() < 0.001);

        flanger.set_feedback(0.8);
        assert!((flanger.feedback() - 0.8).abs() < 0.001);

        // Clamping at extremes
        flanger.set_feedback(-1.5);
        assert!((flanger.feedback() - -0.95).abs() < 0.001);

        flanger.set_feedback(1.5);
        assert!((flanger.feedback() - 0.95).abs() < 0.001);
    }

    #[test]
    fn test_flanger_reset() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_feedback(0.8);
        flanger.set_mix(1.0);

        // Fill with signal
        for _ in 0..500 {
            flanger.process(1.0);
        }

        flanger.reset();

        // After reset, processing zeros should decay quickly
        let output = flanger.process(0.0);
        assert!(
            output.abs() < 0.01,
            "Should be silent after reset, got {output}",
        );
    }

    #[test]
    fn test_flanger_parameter_info() {
        let flanger = Flanger::new(44100.0);

        assert_eq!(flanger.param_count(), 8);

        let rate_info = flanger.param_info(0).unwrap();
        assert_eq!(rate_info.name, "Rate");
        assert_eq!(rate_info.min, 0.05);
        assert_eq!(rate_info.max, 5.0);

        let depth_info = flanger.param_info(1).unwrap();
        assert_eq!(depth_info.name, "Depth");

        let feedback_info = flanger.param_info(2).unwrap();
        assert_eq!(feedback_info.name, "Feedback");
        assert_eq!(feedback_info.min, -95.0);
        assert_eq!(feedback_info.max, 95.0);

        let tzf_info = flanger.param_info(4).unwrap();
        assert_eq!(tzf_info.name, "TZF");
        assert!(tzf_info.flags.contains(ParamFlags::STEPPED));
    }

    #[test]
    fn test_flanger_parameter_get_set() {
        let mut flanger = Flanger::new(44100.0);

        flanger.set_param(0, 2.0);
        assert!((flanger.get_param(0) - 2.0).abs() < 0.01);

        flanger.set_param(1, 75.0);
        assert!((flanger.get_param(1) - 75.0).abs() < 0.01);

        flanger.set_param(2, 80.0);
        assert!((flanger.get_param(2) - 80.0).abs() < 0.01);

        // Negative feedback via param
        flanger.set_param(2, -60.0);
        assert!((flanger.get_param(2) - -60.0).abs() < 0.01);

        flanger.set_param(3, 60.0);
        assert!((flanger.get_param(3) - 60.0).abs() < 0.01);

        // TZF toggle
        flanger.set_param(4, 1.0);
        assert!(flanger.tzf());
        flanger.set_param(4, 0.0);
        assert!(!flanger.tzf());
    }

    #[test]
    fn test_flanger_rate_range() {
        let mut flanger = Flanger::new(44100.0);

        // Test clamping
        flanger.set_rate(0.01);
        assert!((flanger.rate() - 0.05).abs() < 0.001);

        flanger.set_rate(10.0);
        assert!((flanger.rate() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_flanger_tzf_mode() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_tzf(true);
        flanger.set_mix(1.0);
        flanger.set_depth(1.0);

        assert!(flanger.latency_samples() > 0);

        for _ in 0..2000 {
            let output = flanger.process(0.5);
            assert!(output.is_finite());
            assert!(output.abs() < 10.0, "TZF output exceeded bounds: {output}");
        }
    }

    #[test]
    fn test_flanger_tzf_latency() {
        let flanger_off = Flanger::new(44100.0);
        assert_eq!(flanger_off.latency_samples(), 0);

        let mut flanger_on = Flanger::new(44100.0);
        flanger_on.set_tzf(true);
        // base_delay = 5ms at 44100 = 220 samples
        assert_eq!(flanger_on.latency_samples(), 220);
    }

    #[test]
    fn test_flanger_tzf_stereo() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_tzf(true);
        flanger.set_mix(1.0);
        flanger.set_depth(1.0);

        for _ in 0..2000 {
            let (l, r) = flanger.process_stereo(0.5, 0.5);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_flanger_tzf_reset() {
        let mut flanger = Flanger::new(44100.0);
        flanger.set_tzf(true);
        flanger.set_mix(1.0);

        // Fill buffers
        for _ in 0..500 {
            flanger.process(1.0);
        }

        flanger.reset();

        let output = flanger.process(0.0);
        assert!(
            output.abs() < 0.01,
            "Should be silent after TZF reset, got {output}",
        );
    }

    #[test]
    fn test_flanger_tempo_sync() {
        let mut flanger = Flanger::new(44100.0);
        // At 120 BPM, Quarter note = 2 Hz
        flanger.set_sync(true);
        flanger.set_division(sonido_core::NoteDivision::Quarter);
        assert!((flanger.rate.target() - 2.0).abs() < 0.01);

        // At 60 BPM, Quarter note = 1 Hz
        flanger.set_tempo_context(&sonido_core::TempoContext {
            bpm: 60.0,
            ..Default::default()
        });
        assert!((flanger.rate.target() - 1.0).abs() < 0.01);
    }
}
