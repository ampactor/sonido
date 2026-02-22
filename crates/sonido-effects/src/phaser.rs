//! Classic phaser effect with cascaded allpass filters.
//!
//! A phaser creates a characteristic "swooshing" sound by mixing the input
//! with a phase-shifted version of itself. The phase shift is created by
//! cascading multiple allpass filters, whose frequencies are modulated by an LFO.
//! This creates notches in the frequency spectrum that sweep up and down.

use core::f32::consts::PI;
use sonido_core::{
    DIVISION_LABELS, Effect, Lfo, NoteDivision, ParamDescriptor, ParamFlags, ParamId, ParamScale,
    ParamUnit, SmoothedParam, TempoManager, division_to_index, fast_exp2, fast_log2, fast_tan,
    flush_denormal, impl_params, index_to_division, wet_dry_mix,
};

/// Maximum number of allpass stages.
const MAX_STAGES: usize = 12;

/// How many samples between allpass coefficient updates.
///
/// At 48 kHz this gives ~0.67 ms between updates — fast enough that the
/// sweep sounds continuous, but saves 31/32 of the tanf work.
const COEFF_UPDATE_INTERVAL: u32 = 32;

/// Phaser effect with LFO-modulated allpass filters.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Rate | 0.05–5.0 Hz | 0.3 |
/// | 1 | Depth | 0–100% | 50.0 |
/// | 2 | Stages | 2–12 | 6 |
/// | 3 | Feedback | 0–95% | 50.0 |
/// | 4 | Mix | 0–100% | 50.0 |
/// | 5 | Min Freq | 20–2000 Hz | 200.0 |
/// | 6 | Max Freq | 200–20000 Hz | 4000.0 |
/// | 7 | Sync | Off/On | Off |
/// | 8 | Division | 0–11 (note divisions) | 3 (Eighth) |
/// | 9 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Algorithm
///
/// The phaser uses cascaded first-order allpass filters. Each allpass filter
/// contributes a 180-degree phase shift at its center frequency. When the
/// phase-shifted signal is mixed with the original, notches appear at
/// frequencies where the phase difference is 180 degrees.
///
/// # Example
///
/// ```rust
/// use sonido_effects::Phaser;
/// use sonido_core::Effect;
///
/// let mut phaser = Phaser::new(44100.0);
/// phaser.set_rate(0.3);
/// phaser.set_depth(0.8);
/// phaser.set_stages(6);
/// phaser.set_feedback(0.7);
/// phaser.set_mix(0.5);
///
/// let input = 0.5;
/// let output = phaser.process(input);
/// ```
#[derive(Debug, Clone)]
pub struct Phaser {
    /// Allpass filter stages (left channel)
    allpass: [FirstOrderAllpass; MAX_STAGES],
    /// Allpass filter stages (right channel for stereo)
    allpass_r: [FirstOrderAllpass; MAX_STAGES],
    /// LFO for modulation (left channel)
    lfo: Lfo,
    /// LFO for modulation (right channel, phase offset)
    lfo_r: Lfo,
    /// LFO rate parameter
    rate: SmoothedParam,
    /// Modulation depth parameter
    depth: SmoothedParam,
    /// Feedback amount parameter
    feedback: SmoothedParam,
    /// Wet/dry mix parameter
    mix: SmoothedParam,
    /// Output level (linear gain)
    output_level: SmoothedParam,
    /// Stereo spread (LFO phase offset 0-0.5, where 0.5 = 180 degrees)
    stereo_spread: f32,
    /// Number of active stages (2-12)
    stages: usize,
    /// Sample rate
    sample_rate: f32,
    /// Feedback sample for resonance (left)
    feedback_sample: f32,
    /// Feedback sample for resonance (right)
    feedback_sample_r: f32,
    /// Minimum center frequency (Hz)
    min_freq: f32,
    /// Maximum center frequency (Hz)
    max_freq: f32,
    /// Down-counter for block-rate coefficient decimation.
    /// Starts at 0 so the first sample triggers an immediate update.
    coeff_update_counter: u32,
    // -- Tempo sync --
    /// Tempo manager for synced LFO rates.
    tempo: TempoManager,
    /// Whether tempo sync is active (rate derived from BPM + division).
    sync: bool,
    /// Selected note division for tempo sync.
    division: NoteDivision,
}

/// Simple first-order allpass filter for phaser.
///
/// Uses the structure:
/// y[n] = a * x[n] + x[n-1] - a * y[n-1]
///
/// where `a = (tan(pi*fc/fs) - 1) / (tan(pi*fc/fs) + 1)`
#[derive(Debug, Clone, Copy, Default)]
struct FirstOrderAllpass {
    /// Allpass coefficient
    a: f32,
    /// Previous input sample
    x1: f32,
    /// Previous output sample
    y1: f32,
}

impl FirstOrderAllpass {
    /// Create a new first-order allpass filter.
    fn new() -> Self {
        Self {
            a: 0.0,
            x1: 0.0,
            y1: 0.0,
        }
    }

    /// Set the center frequency.
    #[inline]
    fn set_frequency(&mut self, freq: f32, sample_rate: f32) {
        // Clamp frequency to valid range (10 Hz to Nyquist/2)
        let freq = freq.clamp(10.0, sample_rate * 0.4);
        let tan_val = fast_tan(PI * freq / sample_rate);
        self.a = (tan_val - 1.0) / (tan_val + 1.0);
    }

    /// Process a single sample.
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.a * input + self.x1 - self.a * self.y1;
        self.x1 = input;
        self.y1 = output;
        output
    }

    /// Clear filter state.
    fn clear(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

impl Phaser {
    /// Default minimum center frequency for sweep (Hz).
    const DEFAULT_MIN_FREQ: f32 = 200.0;
    /// Default maximum center frequency for sweep (Hz).
    const DEFAULT_MAX_FREQ: f32 = 4000.0;

    /// Create a new phaser effect.
    pub fn new(sample_rate: f32) -> Self {
        let mut lfo_r = Lfo::new(sample_rate, 0.3);
        lfo_r.set_phase(0.25); // 90 degree offset for stereo spread

        Self {
            allpass: [FirstOrderAllpass::new(); MAX_STAGES],
            allpass_r: [FirstOrderAllpass::new(); MAX_STAGES],
            lfo: Lfo::new(sample_rate, 0.3),
            lfo_r,
            rate: SmoothedParam::standard(0.3, sample_rate),
            depth: SmoothedParam::standard(0.5, sample_rate),
            feedback: SmoothedParam::standard(0.5, sample_rate),
            mix: SmoothedParam::standard(0.5, sample_rate),
            output_level: sonido_core::gain::output_level_param(sample_rate),
            stereo_spread: 0.25, // 90 degree default spread
            stages: 6,
            sample_rate,
            feedback_sample: 0.0,
            feedback_sample_r: 0.0,
            min_freq: Self::DEFAULT_MIN_FREQ,
            max_freq: Self::DEFAULT_MAX_FREQ,
            coeff_update_counter: 1,
            tempo: TempoManager::new(sample_rate, 120.0),
            sync: false,
            division: NoteDivision::Quarter,
        }
    }

    /// Set stereo spread (0-0.5, where 0.5 = 180 degree phase offset).
    pub fn set_stereo_spread(&mut self, spread: f32) {
        self.stereo_spread = spread.clamp(0.0, 0.5);
    }

    /// Get current stereo spread.
    pub fn stereo_spread(&self) -> f32 {
        self.stereo_spread
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

    /// Set number of allpass stages (2-12).
    pub fn set_stages(&mut self, stages: usize) {
        self.stages = stages.clamp(2, MAX_STAGES);
    }

    /// Get current number of stages.
    pub fn stages(&self) -> usize {
        self.stages
    }

    /// Set feedback amount (0-0.95).
    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback.set_target(feedback.clamp(0.0, 0.95));
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

    /// Set minimum sweep frequency (20–2000 Hz).
    ///
    /// This is the lowest frequency the allpass sweep reaches when the LFO
    /// is at its minimum. Lower values produce a deeper, wider sweep.
    pub fn set_min_freq(&mut self, freq: f32) {
        self.min_freq = freq.clamp(20.0, 2000.0);
    }

    /// Get current minimum sweep frequency in Hz.
    pub fn min_freq(&self) -> f32 {
        self.min_freq
    }

    /// Set maximum sweep frequency (200–20000 Hz, clamped to 0.45 × sample rate).
    ///
    /// This is the highest frequency the allpass sweep reaches when the LFO
    /// is at its maximum. Higher values produce a brighter, more pronounced sweep.
    pub fn set_max_freq(&mut self, freq: f32) {
        let nyquist_limit = self.sample_rate * 0.45;
        self.max_freq = freq.clamp(200.0, 20000.0f32.min(nyquist_limit));
    }

    /// Get current maximum sweep frequency in Hz.
    pub fn max_freq(&self) -> f32 {
        self.max_freq
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

impl Default for Phaser {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl Effect for Phaser {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Update LFO frequency — must happen every sample
        self.lfo.set_frequency(rate);

        // LFO must advance every sample to keep phase correct
        let lfo_value = self.lfo.advance_unipolar();

        // Decimate allpass coefficient updates to every COEFF_UPDATE_INTERVAL samples
        self.coeff_update_counter = self.coeff_update_counter.wrapping_sub(1);
        if self.coeff_update_counter == 0 {
            self.coeff_update_counter = COEFF_UPDATE_INTERVAL;

            // Calculate center frequency using exponential mapping for natural sweep
            // freq = min_freq * (max_freq/min_freq)^(lfo * depth)
            // Uses fast_exp2(fast_log2()) in place of powf for ~3× speedup
            let freq_ratio = self.max_freq / self.min_freq;
            let center_freq = self.min_freq * fast_exp2(fast_log2(freq_ratio) * lfo_value * depth);

            // Update allpass frequencies
            for i in 0..self.stages {
                // Slightly offset each stage for richer sound
                let stage_offset = 1.0 + (i as f32 * 0.1);
                let stage_freq = center_freq * stage_offset;
                self.allpass[i].set_frequency(stage_freq, self.sample_rate);
            }
        }

        // Add feedback to input
        let input_with_feedback = input + self.feedback_sample * feedback;

        // Process through allpass cascade
        let mut wet = input_with_feedback;
        for i in 0..self.stages {
            wet = self.allpass[i].process(wet);
        }

        // Store for next iteration
        self.feedback_sample = flush_denormal(wet);

        let comp = sonido_core::gain::feedback_wet_compensation(feedback);
        wet_dry_mix(input, wet * comp, mix) * output_gain
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        // True stereo: offset LFO phase between channels for stereo spread
        let rate = self.rate.advance();
        let depth = self.depth.advance();
        let feedback = self.feedback.advance();
        let mix = self.mix.advance();
        let output_gain = self.output_level.advance();

        // Update LFO frequencies — must happen every sample
        self.lfo.set_frequency(rate);
        self.lfo_r.set_frequency(rate);

        // Both LFOs must advance every sample to keep phase correct
        let lfo_l = self.lfo.advance_unipolar();
        let lfo_r = self.lfo_r.advance_unipolar();

        // Decimate allpass coefficient updates to every COEFF_UPDATE_INTERVAL samples
        self.coeff_update_counter = self.coeff_update_counter.wrapping_sub(1);
        if self.coeff_update_counter == 0 {
            self.coeff_update_counter = COEFF_UPDATE_INTERVAL;

            let freq_ratio = self.max_freq / self.min_freq;
            let log_ratio = fast_log2(freq_ratio);

            // Calculate center frequencies for each channel
            let center_freq_l = self.min_freq * fast_exp2(log_ratio * lfo_l * depth);
            let center_freq_r = self.min_freq * fast_exp2(log_ratio * lfo_r * depth);

            // Update allpass frequencies for both channels
            for i in 0..self.stages {
                let stage_offset = 1.0 + (i as f32 * 0.1);
                self.allpass[i].set_frequency(center_freq_l * stage_offset, self.sample_rate);
                self.allpass_r[i].set_frequency(center_freq_r * stage_offset, self.sample_rate);
            }
        }

        // Process left channel
        let input_l = left + self.feedback_sample * feedback;
        let mut wet_l = input_l;
        for i in 0..self.stages {
            wet_l = self.allpass[i].process(wet_l);
        }
        self.feedback_sample = flush_denormal(wet_l);

        // Process right channel
        let input_r = right + self.feedback_sample_r * feedback;
        let mut wet_r = input_r;
        for i in 0..self.stages {
            wet_r = self.allpass_r[i].process(wet_r);
        }
        self.feedback_sample_r = flush_denormal(wet_r);

        let comp = sonido_core::gain::feedback_wet_compensation(feedback);
        let out_l = wet_dry_mix(left, wet_l * comp, mix) * output_gain;
        let out_r = wet_dry_mix(right, wet_r * comp, mix) * output_gain;

        (out_l, out_r)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;

        self.lfo.set_sample_rate(sample_rate);
        self.lfo_r.set_sample_rate(sample_rate);
        self.rate.set_sample_rate(sample_rate);
        self.depth.set_sample_rate(sample_rate);
        self.feedback.set_sample_rate(sample_rate);
        self.mix.set_sample_rate(sample_rate);
        self.output_level.set_sample_rate(sample_rate);
        self.tempo.set_sample_rate(sample_rate);

        // Re-clamp max_freq to new Nyquist limit
        let nyquist_limit = sample_rate * 0.45;
        self.max_freq = self.max_freq.min(nyquist_limit);
    }

    fn set_tempo_context(&mut self, ctx: &sonido_core::TempoContext) {
        self.tempo.set_bpm(ctx.bpm);
        if self.sync {
            self.apply_synced_rate();
        }
    }

    fn reset(&mut self) {
        for i in 0..MAX_STAGES {
            self.allpass[i].clear();
            self.allpass_r[i].clear();
        }
        self.lfo.reset();
        self.lfo_r.reset();
        // Restore stereo spread phase offset
        self.lfo_r.set_phase(self.stereo_spread);
        self.feedback_sample = 0.0;
        self.feedback_sample_r = 0.0;
        self.coeff_update_counter = 1; // wrapping_sub(1) → 0 on next sample, triggers immediate update
        self.rate.snap_to_target();
        self.depth.snap_to_target();
        self.feedback.snap_to_target();
        self.mix.snap_to_target();
        self.output_level.snap_to_target();
    }
}

impl_params! {
    Phaser, this {
        [0] ParamDescriptor::rate_hz(0.05, 5.0, 0.3)
                .with_id(ParamId(900), "phsr_rate"),
            get: this.rate.target(),
            set: |v| this.set_rate(v);

        [1] ParamDescriptor::depth()
                .with_id(ParamId(901), "phsr_depth"),
            get: this.depth.target() * 100.0,
            set: |v| this.set_depth(v / 100.0);

        [2] ParamDescriptor {
                name: "Stages",
                short_name: "Stg",
                unit: ParamUnit::None,
                min: 2.0,
                max: 12.0,
                default: 6.0,
                step: 2.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(902), "phsr_stages")
            .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED)),
            get: this.stages as f32,
            set: |v| this.set_stages(v as usize);

        [3] ParamDescriptor::feedback()
                .with_id(ParamId(903), "phsr_feedback"),
            get: this.feedback.target() * 100.0,
            set: |v| this.set_feedback(v / 100.0);

        [4] ParamDescriptor::mix()
                .with_id(ParamId(904), "phsr_mix"),
            get: this.mix.target() * 100.0,
            set: |v| this.set_mix(v / 100.0);

        [5] ParamDescriptor::custom("Min Freq", "MinF", 20.0, 2000.0, 200.0)
                .with_id(ParamId(906), "phsr_min_freq")
                .with_unit(ParamUnit::Hertz)
                .with_scale(ParamScale::Logarithmic),
            get: this.min_freq,
            set: |v| this.set_min_freq(v);

        [6] ParamDescriptor::custom("Max Freq", "MaxF", 200.0, 20000.0, 4000.0)
                .with_id(ParamId(907), "phsr_max_freq")
                .with_unit(ParamUnit::Hertz)
                .with_scale(ParamScale::Logarithmic),
            get: this.max_freq,
            set: |v| this.set_max_freq(v);

        [7] ParamDescriptor::custom("Sync", "Sync", 0.0, 1.0, 0.0)
                .with_step(1.0)
                .with_id(ParamId(908), "phsr_sync")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(&["Off", "On"]),
            get: if this.sync { 1.0 } else { 0.0 },
            set: |v| this.set_sync(v > 0.5);

        [8] ParamDescriptor::custom("Division", "Div", 0.0, 11.0, 3.0)
                .with_step(1.0)
                .with_id(ParamId(909), "phsr_division")
                .with_flags(ParamFlags::AUTOMATABLE.union(ParamFlags::STEPPED))
                .with_step_labels(DIVISION_LABELS),
            get: division_to_index(this.division) as f32,
            set: |v| this.set_division(index_to_division(v as u8));

        [9] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(905), "phsr_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_phaser_basic() {
        let mut phaser = Phaser::new(44100.0);
        phaser.set_mix(1.0);

        for _ in 0..1000 {
            let output = phaser.process(0.5);
            assert!(output.is_finite());
        }
    }

    #[test]
    fn test_phaser_bypass() {
        let mut phaser = Phaser::new(44100.0);
        phaser.set_mix(0.0);

        // Let smoothing settle
        for _ in 0..1000 {
            phaser.process(1.0);
        }

        let output = phaser.process(0.5);
        assert!((output - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_phaser_feedback_stability() {
        let mut phaser = Phaser::new(44100.0);
        phaser.set_feedback(0.95);
        phaser.set_mix(1.0);
        phaser.set_stages(12);

        // Process many samples to check for instability
        for _ in 0..10000 {
            let output = phaser.process(0.1);
            assert!(output.is_finite());
            assert!(output.abs() < 10.0, "Output exceeded bounds: {}", output);
        }
    }

    #[test]
    fn test_phaser_reset() {
        let mut phaser = Phaser::new(44100.0);
        phaser.set_feedback(0.8);
        phaser.set_mix(1.0);

        // Fill with signal
        for _ in 0..500 {
            phaser.process(1.0);
        }

        phaser.reset();

        // After reset, processing zeros should give near-zero output
        let output = phaser.process(0.0);
        assert!(
            output.abs() < 0.01,
            "Should be silent after reset, got {}",
            output
        );
    }

    #[test]
    fn test_phaser_parameter_info() {
        let phaser = Phaser::new(44100.0);

        assert_eq!(phaser.param_count(), 10);

        let rate_info = phaser.param_info(0).unwrap();
        assert_eq!(rate_info.name, "Rate");
        assert_eq!(rate_info.min, 0.05);
        assert_eq!(rate_info.max, 5.0);

        let stages_info = phaser.param_info(2).unwrap();
        assert_eq!(stages_info.name, "Stages");
        assert_eq!(stages_info.min, 2.0);
        assert_eq!(stages_info.max, 12.0);

        let feedback_info = phaser.param_info(3).unwrap();
        assert_eq!(feedback_info.name, "Feedback");
        assert_eq!(feedback_info.max, 95.0);

        let min_freq_info = phaser.param_info(5).unwrap();
        assert_eq!(min_freq_info.name, "Min Freq");
        assert_eq!(min_freq_info.min, 20.0);
        assert_eq!(min_freq_info.max, 2000.0);

        let max_freq_info = phaser.param_info(6).unwrap();
        assert_eq!(max_freq_info.name, "Max Freq");
        assert_eq!(max_freq_info.min, 200.0);
        assert_eq!(max_freq_info.max, 20000.0);
    }

    #[test]
    fn test_phaser_parameter_get_set() {
        let mut phaser = Phaser::new(44100.0);

        phaser.set_param(0, 2.0);
        assert!((phaser.get_param(0) - 2.0).abs() < 0.01);

        phaser.set_param(1, 75.0);
        assert!((phaser.get_param(1) - 75.0).abs() < 0.01);

        phaser.set_param(2, 8.0);
        assert!((phaser.get_param(2) - 8.0).abs() < 0.01);

        phaser.set_param(3, 80.0);
        assert!((phaser.get_param(3) - 80.0).abs() < 0.01);

        phaser.set_param(4, 60.0);
        assert!((phaser.get_param(4) - 60.0).abs() < 0.01);
    }

    #[test]
    fn test_phaser_stages_range() {
        let mut phaser = Phaser::new(44100.0);

        phaser.set_stages(1);
        assert_eq!(phaser.stages(), 2);

        phaser.set_stages(20);
        assert_eq!(phaser.stages(), 12);

        phaser.set_stages(8);
        assert_eq!(phaser.stages(), 8);
    }

    #[test]
    fn test_phaser_rate_range() {
        let mut phaser = Phaser::new(44100.0);

        phaser.set_rate(0.01);
        assert!((phaser.rate() - 0.05).abs() < 0.001);

        phaser.set_rate(10.0);
        assert!((phaser.rate() - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_first_order_allpass() {
        let mut allpass = FirstOrderAllpass::new();
        allpass.set_frequency(1000.0, 44100.0);

        // Process impulse
        let first = allpass.process(1.0);
        assert!(first.is_finite());

        // Process more samples
        for _ in 0..100 {
            let out = allpass.process(0.0);
            assert!(out.is_finite());
        }
    }

    #[test]
    fn test_phaser_sweep_range_params() {
        let mut phaser = Phaser::new(44100.0);

        // Set via ParameterInfo
        phaser.set_param(5, 100.0); // min_freq = 100 Hz
        assert!((phaser.get_param(5) - 100.0).abs() < 0.01);

        phaser.set_param(6, 8000.0); // max_freq = 8000 Hz
        assert!((phaser.get_param(6) - 8000.0).abs() < 0.01);

        // Process should still be stable
        for _ in 0..5000 {
            let out = phaser.process(0.5);
            assert!(out.is_finite());
        }
    }

    #[test]
    fn test_phaser_sweep_range_clamping() {
        let mut phaser = Phaser::new(44100.0);

        // min_freq below range
        phaser.set_min_freq(5.0);
        assert!((phaser.min_freq() - 20.0).abs() < 0.01);

        // min_freq above range
        phaser.set_min_freq(5000.0);
        assert!((phaser.min_freq() - 2000.0).abs() < 0.01);

        // max_freq clamped to 0.45 × sample_rate
        phaser.set_max_freq(30000.0);
        let expected = 44100.0 * 0.45;
        assert!(
            (phaser.max_freq() - expected).abs() < 1.0,
            "max_freq should be clamped to 0.45 × sr, got {}",
            phaser.max_freq()
        );
    }

    #[test]
    fn test_phaser_sweep_range_sample_rate_reclamp() {
        let mut phaser = Phaser::new(96000.0);
        phaser.set_max_freq(20000.0);
        assert!((phaser.max_freq() - 20000.0).abs() < 0.01);

        // Lower sample rate should re-clamp
        phaser.set_sample_rate(22050.0);
        let expected = 22050.0 * 0.45;
        assert!(
            phaser.max_freq() <= expected + 1.0,
            "max_freq should be re-clamped on sample rate change, got {}",
            phaser.max_freq()
        );
    }

    #[test]
    fn test_phaser_tempo_sync() {
        let mut phaser = Phaser::new(44100.0);
        // At 120 BPM, Half note = 1 Hz
        phaser.set_sync(true);
        phaser.set_division(sonido_core::NoteDivision::Half);
        assert!((phaser.rate.target() - 1.0).abs() < 0.01);

        // At 60 BPM, Half note = 0.5 Hz
        phaser.set_tempo_context(&sonido_core::TempoContext {
            bpm: 60.0,
            ..Default::default()
        });
        assert!((phaser.rate.target() - 0.5).abs() < 0.01);
    }
}
