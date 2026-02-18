//! Tape saturation effect
//!
//! Full tape-machine model inspired by J37/Kramer MPX, featuring:
//! - Asymmetric soft saturation with ADAA anti-aliasing
//! - Wow and flutter (transport instability via modulated delay)
//! - Simplified hysteresis feedback loop
//! - Head bump (low-frequency resonance)
//! - Level-dependent HF rolloff (self-erasure)
//!
//! Signal flow:
//! ```text
//! input → wow/flutter delay → hysteresis feedback → drive+bias → ADAA waveshaping
//!   → blend → head bump EQ → level-dependent HF rolloff → soft limit → output gain
//! ```
//!
//! ## References
//! - Jatin Chowdhury, "Real-time Physical Modelling for Analog Tape Machines"
//! - Valimaki & Smith, tape transport flutter modelling
//! - Magnetic recording self-erasure: HF loss proportional to record level

use sonido_core::math::soft_limit;
use sonido_core::{
    Adaa1, Biquad, Effect, EnvelopeFollower, InterpolatedDelay, Lfo, LfoWaveform, OnePole,
    ParamDescriptor, ParamId, ParamScale, ParamUnit, SmoothedParam, db_to_linear, linear_to_db,
    lowpass_coefficients, peaking_eq_coefficients, tape_sat_ad,
};

/// ADAA processor type for function-pointer waveshapers.
type FnAdaa = Adaa1<fn(f32) -> f32, fn(f32) -> f32>;

/// Base delay for wow/flutter modulation in seconds (5ms).
const WOW_BASE_DELAY_S: f32 = 0.005;

/// Maximum delay for wow/flutter buffer in seconds (10ms).
const WOW_MAX_DELAY_S: f32 = 0.010;

/// Maximum HF cutoff frequency for level-dependent rolloff (Hz).
const HF_MAX: f32 = 15000.0;

/// Minimum HF cutoff frequency at full self-erasure (Hz).
const HF_MIN: f32 = 3000.0;

/// Tape saturation waveshaping function (asymmetric exponential saturation).
///
/// Transfer function:
///
/// ```text
/// f(x) = 1 − exp(−2x)    for x ≥ 0
/// f(x) = −1 + exp(1.8x)   for x < 0
/// ```
///
/// The asymmetry between positive and negative branches creates even harmonics,
/// mimicking the soft compression characteristic of magnetic tape.
///
/// Companion antiderivative: [`sonido_core::tape_sat_ad`].
#[inline]
fn tape_waveshape(x: f32) -> f32 {
    if x >= 0.0 {
        1.0 - libm::expf(-2.0 * x)
    } else {
        -1.0 + libm::expf(1.8 * x)
    }
}

/// Tape saturation effect with full tape-machine modelling.
///
/// ## Parameters
///
/// | Index | Name | Range | Default | Stable ID |
/// |-------|------|-------|---------|-----------|
/// | 0 | Drive | 0.0–24.0 dB | 6.0 | 1400 |
/// | 1 | Saturation | 0–100% | 50.0 | 1401 |
/// | 2 | HF Rolloff | 1000.0–20000.0 Hz | 12000.0 | 1403 |
/// | 3 | Bias | -0.2–0.2 | 0.0 | 1404 |
/// | 4 | Wow | 0.0–1.0 | 0.3 | 1405 |
/// | 5 | Flutter | 0.0–1.0 | 0.2 | 1406 |
/// | 6 | Hysteresis | 0.0–0.5 | 0.15 | 1407 |
/// | 7 | Head Bump | 0.0–1.0 | 0.3 | 1408 |
/// | 8 | Bump Freq | 40–200 Hz | 80.0 | 1409 |
/// | 9 | Output | -12.0–12.0 dB | -6.0 | 1402 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::TapeSaturation;
/// use sonido_core::Effect;
///
/// let mut tape = TapeSaturation::new(48000.0);
/// tape.set_drive(2.0);
/// tape.set_saturation(0.6);
/// tape.set_hf_rolloff(10000.0);
///
/// let input = 0.5;
/// let output = tape.process(input);
/// ```
pub struct TapeSaturation {
    sample_rate: f32,
    /// Input gain (drive) with smoothing
    drive: SmoothedParam,
    /// Output level with smoothing
    output_gain: SmoothedParam,
    /// Saturation amount (0.0 - 1.0) with smoothing
    saturation: SmoothedParam,
    /// HF rolloff frequency in Hz (shadow for get_param readback)
    hf_freq: f32,
    /// Bias (affects harmonic content)
    bias: f32,
    /// ADAA processor for left/mono channel anti-aliased waveshaping
    adaa_l: FnAdaa,
    /// ADAA processor for right channel anti-aliased waveshaping
    adaa_r: FnAdaa,

    // -- Wow/flutter --
    /// Wow LFO (sine, 0.5-3 Hz)
    wow_lfo: Lfo,
    /// Flutter LFO (triangle, 6-15 Hz)
    flutter_lfo: Lfo,
    /// Wow/flutter modulated delay (left/mono channel)
    wow_delay_l: InterpolatedDelay,
    /// Wow/flutter modulated delay (right channel)
    wow_delay_r: InterpolatedDelay,
    /// Wow depth parameter (0.0-1.0)
    wow_depth: f32,
    /// Flutter depth parameter (0.0-1.0)
    flutter_depth: f32,

    // -- Hysteresis --
    /// Hysteresis feedback amount (0.0-0.5)
    hysteresis: f32,
    /// Previous output sample (left channel) for hysteresis feedback
    prev_output_l: f32,
    /// Previous output sample (right channel) for hysteresis feedback
    prev_output_r: f32,

    // -- Head bump --
    /// Head bump peaking EQ (left channel)
    bump_filter_l: Biquad,
    /// Head bump peaking EQ (right channel)
    bump_filter_r: Biquad,
    /// Head bump amount (0.0-1.0, maps to 0-6 dB)
    head_bump: f32,
    /// Head bump center frequency in Hz
    bump_freq: f32,

    // -- Level-dependent HF rolloff --
    /// Level-dependent HF lowpass (left channel)
    hf_filter_l: Biquad,
    /// Level-dependent HF lowpass (right channel)
    hf_filter_r: Biquad,
    /// Envelope follower for left channel input level
    env_l: EnvelopeFollower,
    /// Envelope follower for right channel input level
    env_r: EnvelopeFollower,
    /// Static HF rolloff filter (left channel, legacy parameter)
    hf_static_l: OnePole,
    /// Static HF rolloff filter (right channel, legacy parameter)
    hf_static_r: OnePole,
}

impl Default for TapeSaturation {
    fn default() -> Self {
        Self::new(48000.0)
    }
}

impl TapeSaturation {
    /// Create a new tape saturation effect.
    ///
    /// Initializes all DSP components: ADAA waveshapers, wow/flutter delay lines,
    /// hysteresis state, head bump EQ, and level-dependent HF rolloff filters.
    ///
    /// # Arguments
    /// * `sample_rate` - Audio sample rate in Hz (e.g. 44100.0, 48000.0)
    pub fn new(sample_rate: f32) -> Self {
        let max_delay_samples = (WOW_MAX_DELAY_S * sample_rate) as usize + 1;
        let mut wow_lfo = Lfo::new(sample_rate, 1.5);
        wow_lfo.set_waveform(LfoWaveform::Sine);
        let mut flutter_lfo = Lfo::new(sample_rate, 10.0);
        flutter_lfo.set_waveform(LfoWaveform::Triangle);

        // Head bump: peaking EQ at 80 Hz, Q=1.5, default 0 dB (off)
        let mut bump_filter_l = Biquad::new();
        let mut bump_filter_r = Biquad::new();
        let bump_coeffs = peaking_eq_coefficients(80.0, 1.5, 0.0, sample_rate);
        bump_filter_l.set_coefficients(
            bump_coeffs.0,
            bump_coeffs.1,
            bump_coeffs.2,
            bump_coeffs.3,
            bump_coeffs.4,
            bump_coeffs.5,
        );
        bump_filter_r.set_coefficients(
            bump_coeffs.0,
            bump_coeffs.1,
            bump_coeffs.2,
            bump_coeffs.3,
            bump_coeffs.4,
            bump_coeffs.5,
        );

        // Level-dependent HF lowpass, starts at max (clamped to Nyquist)
        let mut hf_filter_l = Biquad::new();
        let mut hf_filter_r = Biquad::new();
        let safe_hf_max = HF_MAX.min(sample_rate * 0.45);
        let hf_coeffs = lowpass_coefficients(safe_hf_max, 0.707, sample_rate);
        hf_filter_l.set_coefficients(
            hf_coeffs.0,
            hf_coeffs.1,
            hf_coeffs.2,
            hf_coeffs.3,
            hf_coeffs.4,
            hf_coeffs.5,
        );
        hf_filter_r.set_coefficients(
            hf_coeffs.0,
            hf_coeffs.1,
            hf_coeffs.2,
            hf_coeffs.3,
            hf_coeffs.4,
            hf_coeffs.5,
        );

        Self {
            sample_rate,
            drive: SmoothedParam::standard(db_to_linear(6.0), sample_rate),
            output_gain: SmoothedParam::standard(db_to_linear(-6.0), sample_rate),
            saturation: SmoothedParam::standard(0.3, sample_rate),
            hf_freq: 12000.0,
            bias: 0.0,
            adaa_l: Adaa1::new(
                tape_waveshape as fn(f32) -> f32,
                tape_sat_ad as fn(f32) -> f32,
            ),
            adaa_r: Adaa1::new(
                tape_waveshape as fn(f32) -> f32,
                tape_sat_ad as fn(f32) -> f32,
            ),

            wow_lfo,
            flutter_lfo,
            wow_delay_l: InterpolatedDelay::new(max_delay_samples),
            wow_delay_r: InterpolatedDelay::new(max_delay_samples),
            wow_depth: 0.3,
            flutter_depth: 0.2,

            hysteresis: 0.15,
            prev_output_l: 0.0,
            prev_output_r: 0.0,

            bump_filter_l,
            bump_filter_r,
            head_bump: 0.3,
            bump_freq: 80.0,

            hf_filter_l,
            hf_filter_r,
            env_l: EnvelopeFollower::with_times(sample_rate, 1.0, 50.0),
            env_r: EnvelopeFollower::with_times(sample_rate, 1.0, 50.0),
            hf_static_l: OnePole::new(sample_rate, 12000.0),
            hf_static_r: OnePole::new(sample_rate, 12000.0),
        }
    }

    /// Set input drive (1.0 = unity, higher = more saturation).
    ///
    /// Range: 0.1 to 10.0 (linear gain). Values are clamped.
    pub fn set_drive(&mut self, drive: f32) {
        self.drive.set_target(drive.clamp(0.1, 10.0));
    }

    /// Get current drive target (linear gain).
    pub fn drive(&self) -> f32 {
        self.drive.target()
    }

    /// Set output gain (linear).
    ///
    /// Range: 0.0 to 2.0. Values are clamped.
    pub fn set_output(&mut self, gain: f32) {
        self.output_gain.set_target(gain.clamp(0.0, 2.0));
    }

    /// Get current output gain target (linear).
    pub fn output(&self) -> f32 {
        self.output_gain.target()
    }

    /// Set saturation amount.
    ///
    /// Range: 0.0 (clean) to 1.0 (full saturation). Values are clamped.
    pub fn set_saturation(&mut self, sat: f32) {
        self.saturation.set_target(sat.clamp(0.0, 1.0));
    }

    /// Get current saturation target.
    pub fn saturation(&self) -> f32 {
        self.saturation.target()
    }

    /// Set static high frequency rolloff point in Hz.
    ///
    /// Range: 1000 to 20000 Hz. This is applied in addition to the
    /// level-dependent HF rolloff. Values are clamped.
    pub fn set_hf_rolloff(&mut self, freq: f32) {
        let freq = freq.clamp(1000.0, 20000.0);
        self.hf_freq = freq;
        self.hf_static_l.set_frequency(freq);
        self.hf_static_r.set_frequency(freq);
    }

    /// Set tape bias (affects harmonic character).
    ///
    /// Range: -0.2 to 0.2. Values are clamped.
    pub fn set_bias(&mut self, bias: f32) {
        self.bias = bias.clamp(-0.2, 0.2);
    }

    /// Get current bias.
    pub fn bias(&self) -> f32 {
        self.bias
    }

    /// Set wow depth (tape transport low-frequency wobble).
    ///
    /// Range: 0.0 (off) to 1.0 (max wobble). Values are clamped.
    /// At 1.0, wow modulates the delay by up to 2ms at 0.5-3 Hz.
    pub fn set_wow_depth(&mut self, depth: f32) {
        self.wow_depth = depth.clamp(0.0, 1.0);
    }

    /// Get current wow depth.
    pub fn wow_depth(&self) -> f32 {
        self.wow_depth
    }

    /// Set flutter depth (tape transport high-frequency instability).
    ///
    /// Range: 0.0 (off) to 1.0 (max flutter). Values are clamped.
    /// At 1.0, flutter modulates the delay by up to 0.2ms at 6-15 Hz.
    pub fn set_flutter_depth(&mut self, depth: f32) {
        self.flutter_depth = depth.clamp(0.0, 1.0);
    }

    /// Get current flutter depth.
    pub fn flutter_depth(&self) -> f32 {
        self.flutter_depth
    }

    /// Set hysteresis feedback amount.
    ///
    /// Range: 0.0 (off) to 0.5 (max feedback). Values are clamped.
    /// Creates frequency-dependent saturation by feeding output back into input.
    pub fn set_hysteresis(&mut self, hyst: f32) {
        self.hysteresis = hyst.clamp(0.0, 0.5);
    }

    /// Get current hysteresis amount.
    pub fn hysteresis(&self) -> f32 {
        self.hysteresis
    }

    /// Set head bump amount (low-frequency resonance).
    ///
    /// Range: 0.0 (off) to 1.0 (+6 dB boost at bump frequency). Values are clamped.
    pub fn set_head_bump(&mut self, amount: f32) {
        let amount = amount.clamp(0.0, 1.0);
        self.head_bump = amount;
        self.update_bump_filter();
    }

    /// Get current head bump amount.
    pub fn head_bump(&self) -> f32 {
        self.head_bump
    }

    /// Set head bump center frequency in Hz.
    ///
    /// Range: 40 to 200 Hz. Values are clamped.
    pub fn set_bump_freq(&mut self, freq: f32) {
        let freq = freq.clamp(40.0, 200.0);
        self.bump_freq = freq;
        self.update_bump_filter();
    }

    /// Get current bump frequency in Hz.
    pub fn bump_freq(&self) -> f32 {
        self.bump_freq
    }

    /// Recalculate head bump peaking EQ coefficients.
    fn update_bump_filter(&mut self) {
        let gain_db = self.head_bump * 6.0;
        let coeffs = peaking_eq_coefficients(self.bump_freq, 1.5, gain_db, self.sample_rate);
        self.bump_filter_l
            .set_coefficients(coeffs.0, coeffs.1, coeffs.2, coeffs.3, coeffs.4, coeffs.5);
        self.bump_filter_r
            .set_coefficients(coeffs.0, coeffs.1, coeffs.2, coeffs.3, coeffs.4, coeffs.5);
    }

    /// Compute wow/flutter delay modulation in samples.
    ///
    /// Returns the combined delay time from the base delay plus wow and flutter
    /// LFO modulation. Both LFOs are advanced as a side effect.
    #[inline]
    fn compute_wow_flutter_delay(&mut self) -> f32 {
        let base_samples = WOW_BASE_DELAY_S * self.sample_rate;

        // Wow: sine, depth 0.5-2ms mapped by wow_depth
        let wow_mod = self.wow_lfo.advance();
        let wow_depth_ms = 0.5 + self.wow_depth * 1.5; // 0.5ms to 2ms
        let wow_samples = wow_mod * wow_depth_ms * 0.001 * self.sample_rate * self.wow_depth;

        // Flutter: triangle, depth 0.05-0.2ms mapped by flutter_depth
        let flutter_mod = self.flutter_lfo.advance();
        let flutter_depth_ms = 0.05 + self.flutter_depth * 0.15; // 0.05ms to 0.2ms
        let flutter_samples =
            flutter_mod * flutter_depth_ms * 0.001 * self.sample_rate * self.flutter_depth;

        let total = base_samples + wow_samples + flutter_samples;
        // Clamp to valid delay range (1 sample to buffer capacity)
        let max = WOW_MAX_DELAY_S * self.sample_rate;
        total.clamp(1.0, max)
    }

    /// Maximum safe filter frequency: 0.45 * sample_rate (below Nyquist).
    #[inline]
    fn safe_nyquist(&self) -> f32 {
        self.sample_rate * 0.45
    }

    /// Compute level-dependent HF cutoff and update filter coefficients.
    ///
    /// Higher input levels combined with higher drive push the cutoff lower,
    /// simulating magnetic tape self-erasure where loud, heavily-driven signals
    /// lose more high-frequency content. Cutoff is clamped below Nyquist.
    #[inline]
    fn update_hf_cutoff(&mut self, level_l: f32, level_r: f32, drive_linear: f32) {
        let nyquist = self.safe_nyquist();
        let hf_max = HF_MAX.min(nyquist);
        let hf_min = HF_MIN.min(nyquist * 0.5);
        // Normalize drive from linear (0.1..10.0) to 0..1 range for modulation
        let drive_norm = ((drive_linear - 0.1) / 9.9).clamp(0.0, 1.0);

        let cutoff_l = (hf_max - (hf_max - hf_min) * level_l * drive_norm).clamp(hf_min, hf_max);
        let coeffs_l = lowpass_coefficients(cutoff_l, 0.707, self.sample_rate);
        self.hf_filter_l.set_coefficients(
            coeffs_l.0, coeffs_l.1, coeffs_l.2, coeffs_l.3, coeffs_l.4, coeffs_l.5,
        );

        let cutoff_r = (hf_max - (hf_max - hf_min) * level_r * drive_norm).clamp(hf_min, hf_max);
        let coeffs_r = lowpass_coefficients(cutoff_r, 0.707, self.sample_rate);
        self.hf_filter_r.set_coefficients(
            coeffs_r.0, coeffs_r.1, coeffs_r.2, coeffs_r.3, coeffs_r.4, coeffs_r.5,
        );
    }

    /// Process a single channel through the full tape chain.
    ///
    /// Shared logic for mono `process()` — uses left-channel state for all filters.
    #[inline]
    fn process_channel_l(
        &mut self,
        input: f32,
        drive: f32,
        saturation: f32,
        output_gain: f32,
    ) -> f32 {
        // 1. Wow/flutter: modulated delay (bypass when both depths zero)
        let wow_out = if self.wow_depth > 0.0 || self.flutter_depth > 0.0 {
            let delay_samples = self.compute_wow_flutter_delay();
            self.wow_delay_l.read_write(input, delay_samples)
        } else {
            self.wow_lfo.advance();
            self.flutter_lfo.advance();
            input
        };

        // 2. Hysteresis feedback (pre-filter, magnetic domain)
        let x_eff = wow_out + self.hysteresis * self.prev_output_l;

        // 3. Drive + bias → ADAA waveshaping
        let driven = x_eff * drive + self.bias;
        let saturated = self.adaa_l.process(driven);
        let clean = driven.clamp(-1.0, 1.0);
        let blended = clean * (1.0 - saturation) + saturated * saturation;
        self.prev_output_l = blended;

        // 4. Head bump EQ
        let bumped = self.bump_filter_l.process(blended);

        // 5. Level-dependent HF rolloff
        let level = self.env_l.process(libm::fabsf(input));
        let nyquist = self.safe_nyquist();
        let hf_max = HF_MAX.min(nyquist);
        let hf_min = HF_MIN.min(nyquist * 0.5);
        let drive_norm = ((drive - 0.1) / 9.9).clamp(0.0, 1.0);
        let cutoff = (hf_max - (hf_max - hf_min) * level * drive_norm).clamp(hf_min, hf_max);
        let coeffs = lowpass_coefficients(cutoff, 0.707, self.sample_rate);
        self.hf_filter_l
            .set_coefficients(coeffs.0, coeffs.1, coeffs.2, coeffs.3, coeffs.4, coeffs.5);
        let hf_out = self.hf_filter_l.process(bumped);

        // 6. Static HF rolloff
        let filtered = self.hf_static_l.process(hf_out);

        // 7. Soft limit + output gain
        soft_limit(filtered, 1.0) * output_gain
    }
}

impl Effect for TapeSaturation {
    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let drive = self.drive.advance();
        let output_gain = self.output_gain.advance();
        let saturation = self.saturation.advance();
        self.process_channel_l(input, drive, saturation, output_gain)
    }

    #[inline]
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.advance();
        let output_gain = self.output_gain.advance();
        let saturation = self.saturation.advance();

        // Compute shared wow/flutter delay (bypass when both depths zero)
        let (wow_l, wow_r) = if self.wow_depth > 0.0 || self.flutter_depth > 0.0 {
            let delay_samples = self.compute_wow_flutter_delay();
            (
                self.wow_delay_l.read_write(left, delay_samples),
                self.wow_delay_r.read_write(right, delay_samples),
            )
        } else {
            self.wow_lfo.advance();
            self.flutter_lfo.advance();
            (left, right)
        };

        // --- Left channel ---
        let x_eff_l = wow_l + self.hysteresis * self.prev_output_l;
        let driven_l = x_eff_l * drive + self.bias;
        let sat_l = self.adaa_l.process(driven_l);
        let clean_l = driven_l.clamp(-1.0, 1.0);
        let blended_l = clean_l * (1.0 - saturation) + sat_l * saturation;

        // --- Right channel ---
        let x_eff_r = wow_r + self.hysteresis * self.prev_output_r;
        let driven_r = x_eff_r * drive + self.bias;
        let sat_r = self.adaa_r.process(driven_r);
        let clean_r = driven_r.clamp(-1.0, 1.0);
        let blended_r = clean_r * (1.0 - saturation) + sat_r * saturation;

        // Hysteresis feedback (pre-filter, magnetic domain)
        self.prev_output_l = blended_l;
        self.prev_output_r = blended_r;

        // Head bump EQ
        let bumped_l = self.bump_filter_l.process(blended_l);
        let bumped_r = self.bump_filter_r.process(blended_r);

        // Level-dependent HF rolloff (per-channel envelope)
        let level_l = self.env_l.process(libm::fabsf(left));
        let level_r = self.env_r.process(libm::fabsf(right));
        self.update_hf_cutoff(level_l, level_r, drive);
        let hf_l = self.hf_filter_l.process(bumped_l);
        let hf_r = self.hf_filter_r.process(bumped_r);

        // Static HF rolloff
        let filt_l = self.hf_static_l.process(hf_l);
        let filt_r = self.hf_static_r.process(hf_r);

        // Soft limit + output gain
        let out_l = soft_limit(filt_l, 1.0) * output_gain;
        let out_r = soft_limit(filt_r, 1.0) * output_gain;

        (out_l, out_r)
    }

    fn reset(&mut self) {
        self.drive.snap_to_target();
        self.output_gain.snap_to_target();
        self.saturation.snap_to_target();
        self.adaa_l.reset();
        self.adaa_r.reset();

        // Wow/flutter
        self.wow_lfo.reset();
        self.flutter_lfo.reset();
        self.wow_delay_l.clear();
        self.wow_delay_r.clear();

        // Hysteresis: converge DC operating point for bias + hysteresis feedback.
        // Without this, the feedback loop (which may have loop gain near 1) takes
        // thousands of samples to settle when SmoothedParams start at different values.
        // Note: exact reset-vs-fresh matching is limited by path-dependent convergence
        // through the nonlinear feedback loop (snapped params vs smoothed params reach
        // slightly different DC operating points).
        self.prev_output_l = 0.0;
        self.prev_output_r = 0.0;
        if self.hysteresis > 0.0 && libm::fabsf(self.bias) > 1e-10 {
            let drive = self.drive.target();
            let sat = self.saturation.target();
            for _ in 0..2048 {
                let x_eff = self.hysteresis * self.prev_output_l;
                let driven = x_eff * drive + self.bias;
                let saturated = tape_waveshape(driven);
                let clean = driven.clamp(-1.0, 1.0);
                self.prev_output_l = clean * (1.0 - sat) + saturated * sat;
            }
            self.prev_output_r = self.prev_output_l;
        }

        // Head bump
        self.bump_filter_l.clear();
        self.bump_filter_r.clear();

        // HF filters
        self.hf_filter_l.clear();
        self.hf_filter_r.clear();
        self.env_l.reset();
        self.env_r.reset();
        self.hf_static_l.reset();
        self.hf_static_r.reset();
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.drive.set_sample_rate(sample_rate);
        self.output_gain.set_sample_rate(sample_rate);
        self.saturation.set_sample_rate(sample_rate);
        self.wow_lfo.set_sample_rate(sample_rate);
        self.flutter_lfo.set_sample_rate(sample_rate);
        // Rebuild delay buffers at new sample rate
        let max_delay_samples = (WOW_MAX_DELAY_S * sample_rate) as usize + 1;
        self.wow_delay_l = InterpolatedDelay::new(max_delay_samples);
        self.wow_delay_r = InterpolatedDelay::new(max_delay_samples);
        // Rebuild envelope followers
        self.env_l = EnvelopeFollower::with_times(sample_rate, 1.0, 50.0);
        self.env_r = EnvelopeFollower::with_times(sample_rate, 1.0, 50.0);
        // Update filter sample rates
        self.hf_static_l.set_sample_rate(sample_rate);
        self.hf_static_r.set_sample_rate(sample_rate);
        // Recompute bump EQ coefficients
        self.update_bump_filter();
        // Recompute HF lowpass (clamped to Nyquist)
        let safe_hf_max = HF_MAX.min(sample_rate * 0.45);
        let hf_coeffs = lowpass_coefficients(safe_hf_max, 0.707, sample_rate);
        self.hf_filter_l.set_coefficients(
            hf_coeffs.0,
            hf_coeffs.1,
            hf_coeffs.2,
            hf_coeffs.3,
            hf_coeffs.4,
            hf_coeffs.5,
        );
        self.hf_filter_r.set_coefficients(
            hf_coeffs.0,
            hf_coeffs.1,
            hf_coeffs.2,
            hf_coeffs.3,
            hf_coeffs.4,
            hf_coeffs.5,
        );
    }
}

sonido_core::impl_params! {
    TapeSaturation, this {
        [0] ParamDescriptor {
                name: "Drive",
                short_name: "Drive",
                unit: ParamUnit::Decibels,
                min: 0.0,
                max: 24.0,
                default: 6.0,
                step: 0.5,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1400), "tape_drive"),
            get: linear_to_db(this.drive.target()),
            set: |v| this.set_drive(db_to_linear(v));

        [1] ParamDescriptor {
                name: "Saturation",
                short_name: "Sat",
                unit: ParamUnit::Percent,
                min: 0.0,
                max: 100.0,
                default: 30.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1401), "tape_saturation"),
            get: this.saturation.target() * 100.0,
            set: |v| this.set_saturation(v / 100.0);

        [2] ParamDescriptor {
                name: "HF Rolloff",
                short_name: "HFRoll",
                unit: ParamUnit::Hertz,
                min: 1000.0,
                max: 20000.0,
                default: 12000.0,
                step: 100.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1403), "tape_hf_rolloff")
            .with_scale(ParamScale::Logarithmic),
            get: this.hf_freq,
            set: |v| this.set_hf_rolloff(v);

        [3] ParamDescriptor {
                name: "Bias",
                short_name: "Bias",
                unit: ParamUnit::None,
                min: -0.2,
                max: 0.2,
                default: 0.0,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1404), "tape_bias"),
            get: this.bias,
            set: |v| this.set_bias(v);

        [4] ParamDescriptor {
                name: "Wow",
                short_name: "Wow",
                unit: ParamUnit::None,
                min: 0.0,
                max: 1.0,
                default: 0.3,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1405), "tape_wow"),
            get: this.wow_depth,
            set: |v| this.set_wow_depth(v);

        [5] ParamDescriptor {
                name: "Flutter",
                short_name: "Flut",
                unit: ParamUnit::None,
                min: 0.0,
                max: 1.0,
                default: 0.2,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1406), "tape_flutter"),
            get: this.flutter_depth,
            set: |v| this.set_flutter_depth(v);

        [6] ParamDescriptor {
                name: "Hysteresis",
                short_name: "Hyst",
                unit: ParamUnit::None,
                min: 0.0,
                max: 0.5,
                default: 0.15,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1407), "tape_hysteresis"),
            get: this.hysteresis,
            set: |v| this.set_hysteresis(v);

        [7] ParamDescriptor {
                name: "Head Bump",
                short_name: "Bump",
                unit: ParamUnit::None,
                min: 0.0,
                max: 1.0,
                default: 0.3,
                step: 0.01,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1408), "tape_head_bump"),
            get: this.head_bump,
            set: |v| this.set_head_bump(v);

        [8] ParamDescriptor {
                name: "Bump Freq",
                short_name: "BmpHz",
                unit: ParamUnit::Hertz,
                min: 40.0,
                max: 200.0,
                default: 80.0,
                step: 1.0,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1409), "tape_bump_freq")
            .with_scale(ParamScale::Logarithmic),
            get: this.bump_freq,
            set: |v| this.set_bump_freq(v);

        [9] ParamDescriptor {
                name: "Output",
                short_name: "Output",
                unit: ParamUnit::Decibels,
                min: -12.0,
                max: 12.0,
                default: -6.0,
                step: 0.5,
                ..ParamDescriptor::mix()
            }
            .with_id(ParamId(1402), "tape_output"),
            get: linear_to_db(this.output_gain.target()),
            set: |v| this.set_output(db_to_linear(v));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tape_saturation_basic() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_drive(2.0);
        tape.reset();

        for _ in 0..100 {
            let output = tape.process(0.5);
            assert!(output.is_finite());
            assert!(output.abs() <= 2.0);
        }
    }

    #[test]
    fn test_tape_saturation_asymmetry() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_saturation(1.0);
        tape.set_drive(3.0);
        // Disable wow/flutter/hysteresis for deterministic test
        tape.set_wow_depth(0.0);
        tape.set_flutter_depth(0.0);
        tape.set_hysteresis(0.0);
        tape.set_head_bump(0.0);
        tape.reset();

        // Let filter settle
        for _ in 0..200 {
            tape.process(0.0);
        }

        let pos = tape.process(0.5);
        tape.reset();
        for _ in 0..200 {
            tape.process(0.0);
        }
        let neg = tape.process(-0.5);

        assert!(
            (pos.abs() - neg.abs()).abs() > 0.001,
            "Should be asymmetric, pos={pos}, neg={neg}"
        );
    }

    #[test]
    fn test_tape_saturation_reset() {
        let mut tape = TapeSaturation::new(48000.0);

        for _ in 0..100 {
            tape.process(1.0);
        }

        tape.reset();

        // After reset, processing zero should produce near-zero output
        // (allow slightly more tolerance due to wow/flutter delay line)
        let output = tape.process(0.0);
        assert!(
            output.abs() < 0.01,
            "After reset, zero input should produce near-zero output, got {output}"
        );
    }

    #[test]
    fn test_tape_saturation_smoothing() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_drive(1.0);
        tape.reset();

        tape.set_drive(5.0);
        let first = tape.process(0.5);

        for _ in 0..1000 {
            tape.process(0.5);
        }
        let settled = tape.process(0.5);

        assert!(settled != first, "Smoothing should gradually change drive");
    }

    #[test]
    fn test_wow_flutter_modulation() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.set_wow_depth(1.0);
        tape.set_flutter_depth(1.0);
        tape.set_saturation(0.0);
        tape.set_hysteresis(0.0);
        tape.set_head_bump(0.0);
        tape.reset();

        // Feed constant signal; wow/flutter should cause variation
        let mut outputs = [0.0f32; 500];
        for out in &mut outputs {
            *out = tape.process(0.5);
        }

        // Not all samples should be identical
        let min = outputs.iter().copied().fold(f32::INFINITY, f32::min);
        let max = outputs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        assert!(
            max - min > 1e-6,
            "Wow/flutter should cause output variation, min={min}, max={max}"
        );
    }

    #[test]
    fn test_hysteresis_feedback() {
        let sr = 48000.0;
        let mut with_hyst = TapeSaturation::new(sr);
        with_hyst.set_hysteresis(0.4);
        with_hyst.set_wow_depth(0.0);
        with_hyst.set_flutter_depth(0.0);
        with_hyst.set_head_bump(0.0);
        with_hyst.reset();

        let mut without_hyst = TapeSaturation::new(sr);
        without_hyst.set_hysteresis(0.0);
        without_hyst.set_wow_depth(0.0);
        without_hyst.set_flutter_depth(0.0);
        without_hyst.set_head_bump(0.0);
        without_hyst.reset();

        // Process identical input, outputs should differ due to feedback
        let mut diff_sum = 0.0f32;
        for i in 0..200 {
            let input = libm::sinf(2.0 * core::f32::consts::PI * 440.0 * i as f32 / sr);
            let a = with_hyst.process(input);
            let b = without_hyst.process(input);
            diff_sum += libm::fabsf(a - b);
        }
        assert!(
            diff_sum > 0.01,
            "Hysteresis feedback should change output, diff_sum={diff_sum}"
        );
    }

    #[test]
    fn test_head_bump_boost() {
        let sr = 48000.0;
        let mut bumped = TapeSaturation::new(sr);
        bumped.set_head_bump(1.0);
        bumped.set_bump_freq(80.0);
        bumped.set_saturation(0.0);
        bumped.set_wow_depth(0.0);
        bumped.set_flutter_depth(0.0);
        bumped.set_hysteresis(0.0);
        bumped.reset();

        let mut flat = TapeSaturation::new(sr);
        flat.set_head_bump(0.0);
        flat.set_saturation(0.0);
        flat.set_wow_depth(0.0);
        flat.set_flutter_depth(0.0);
        flat.set_hysteresis(0.0);
        flat.reset();

        // Feed 80 Hz sine; head bump should boost
        let mut energy_bumped = 0.0f32;
        let mut energy_flat = 0.0f32;
        for i in 0..2000 {
            let input = 0.3 * libm::sinf(2.0 * core::f32::consts::PI * 80.0 * i as f32 / sr);
            let a = bumped.process(input);
            let b = flat.process(input);
            energy_bumped += a * a;
            energy_flat += b * b;
        }
        assert!(
            energy_bumped > energy_flat * 1.1,
            "Head bump should boost 80Hz, bumped={energy_bumped}, flat={energy_flat}"
        );
    }

    #[test]
    fn test_stereo_processing() {
        let mut tape = TapeSaturation::new(48000.0);
        tape.reset();

        for _ in 0..100 {
            let (l, r) = tape.process_stereo(0.5, -0.3);
            assert!(l.is_finite());
            assert!(r.is_finite());
        }
    }

    #[test]
    fn test_param_count() {
        use sonido_core::ParameterInfo;
        let tape = TapeSaturation::new(48000.0);
        assert_eq!(tape.param_count(), 10);
    }
}
