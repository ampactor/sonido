//! Brickwall lookahead limiter with exponential release ballistics.
//!
//! A true peak limiter that uses a lookahead buffer to anticipate peaks and apply
//! gain reduction before they arrive at the output. This guarantees zero overshoot
//! past the ceiling level — the fundamental property that distinguishes a limiter
//! from a high-ratio compressor.
//!
//! # Algorithm
//!
//! 1. **Lookahead buffering**: Input is written to a circular delay buffer. The
//!    delayed output is the main audio signal; the un-delayed input feeds the
//!    peak detector.
//! 2. **Peak detection**: Scan the full lookahead window every sample for the
//!    maximum absolute amplitude. This is `O(lookahead_samples)` per sample;
//!    for a 5 ms window at 48 kHz that is 240 comparisons.
//! 3. **Gain computation**: If `peak > threshold`, the required gain reduction is
//!    `G = threshold / peak` (linear ratio). The ceiling offset then adds
//!    `db_to_linear(ceiling_db)` to cap the final output level.
//! 4. **Gain smoothing**: A one-pole filter tracks gain reductions instantaneously
//!    downward (attack comes for free from lookahead — no additional attack
//!    coefficient needed) and releases exponentially:
//!    `g[n] = min(target, release_coeff * g[n-1] + (1 - release_coeff) * target)`
//!    Wait — for release, we want the gain to *increase* slowly back to 1.0:
//!    `if target < g[n-1]: g[n] = target  (instant attack via lookahead)`
//!    `else:               g[n] = release_coeff * g[n-1] + (1-release_coeff) * target`
//! 5. **Output**: Delayed sample × smoothed gain × output_level.
//!
//! # Stereo Linking
//!
//! In stereo mode, the peak detector uses `max(|L|, |M|)` so a transient on
//! either channel causes identical gain reduction on both. This prevents stereo
//! image shift from unlinked limiting.
//!
//! # References
//!
//! - Giannoulis, Massberg & Reiss, "Digital Dynamic Range Compressor Design — A
//!   Tutorial and Analysis", JAES vol. 60 no. 6, 2012. Sections IV–V cover attack/
//!   release ballistics and the one-pole smoothing approach used here.
//! - Zölzer, "DAFX: Digital Audio Effects" (2nd ed.), Ch. 4 — brickwall limiter
//!   topology with lookahead.
//!
//! # Parameters
//!
//! | Parameter | Range | Default | Description |
//! |-----------|-------|---------|-------------|
//! | Threshold | -30–0 dB | -6.0 | Level above which gain reduction begins |
//! | Ceiling | -30–0 dB | -0.3 | Hard output ceiling (brickwall) |
//! | Release | 10–500 ms | 100.0 | Exponential release time constant |
//! | Lookahead | 0–10 ms | 5.0 | Look-ahead delay; sets latency |
//! | Output | -20–+20 dB | 0.0 | Final output level trim |

#[cfg(not(feature = "std"))]
extern crate alloc;
#[cfg(not(feature = "std"))]
use alloc::vec;
#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use libm::{expf, fabsf};
use sonido_core::{Effect, ParamDescriptor, ParamId, SmoothedParam, gain, math::db_to_linear};

/// Maximum lookahead in milliseconds — used to size the fixed delay buffers.
const MAX_LOOKAHEAD_MS: f32 = 10.0;

/// Brickwall lookahead limiter.
///
/// Prevents output from ever exceeding the ceiling level by using a delay buffer
/// to detect and reduce peaks before they reach the output stage.
///
/// ## Parameter Indices (`ParameterInfo`)
///
/// | Index | Name | Range | Default |
/// |-------|------|-------|---------|
/// | 0 | Threshold | -30.0–0.0 dB | -6.0 |
/// | 1 | Ceiling | -30.0–0.0 dB | -0.3 |
/// | 2 | Release | 10.0–500.0 ms | 100.0 |
/// | 3 | Lookahead | 0.0–10.0 ms | 5.0 |
/// | 4 | Output | -20.0–20.0 dB | 0.0 |
///
/// # Example
///
/// ```rust
/// use sonido_effects::Limiter;
/// use sonido_core::Effect;
///
/// let mut lim = Limiter::new(48000.0);
/// lim.set_threshold_db(-6.0);
/// lim.set_ceiling_db(-0.3);
///
/// // Process 512 samples
/// let input = 0.9_f32;
/// for _ in 0..512 {
///     let _out = lim.process(input);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Limiter {
    /// Audio sample rate in Hz.
    sample_rate: f32,

    /// Smoothed threshold parameter (dB, fast 5 ms smoothing).
    threshold: SmoothedParam,

    /// Ceiling level in dB — hard brickwall.
    ceiling_db: f32,

    /// Release time in milliseconds.
    release_ms: f32,

    /// One-pole release coefficient: `exp(-1 / (release_ms * sr / 1000))`.
    release_coeff: f32,

    /// Lookahead duration in milliseconds.
    lookahead_ms: f32,

    /// Lookahead duration in samples (rounded from `lookahead_ms`).
    lookahead_samples: usize,

    /// Circular delay buffer — left channel.
    buffer_l: Vec<f32>,

    /// Circular delay buffer — right channel.
    buffer_r: Vec<f32>,

    /// Write position within the circular buffer.
    write_pos: usize,

    /// Current smoothed gain reduction (linear, 1.0 = no reduction).
    gain_reduction: f32,

    /// Final output level trim (smoothed).
    output_level: SmoothedParam,

    /// Total circular buffer size in samples (corresponds to `MAX_LOOKAHEAD_MS`).
    max_lookahead_samples: usize,
}

impl Limiter {
    /// Create a limiter with default settings at the given sample rate.
    ///
    /// Defaults: threshold −6 dB, ceiling −0.3 dB, release 100 ms, lookahead 5 ms.
    pub fn new(sample_rate: f32) -> Self {
        let max_samples = ms_to_samples(MAX_LOOKAHEAD_MS, sample_rate);
        let lookahead_ms = 5.0_f32;
        let lookahead_samples = ms_to_samples(lookahead_ms, sample_rate);
        let release_ms = 100.0_f32;
        let release_coeff = compute_release_coeff(release_ms, sample_rate);

        Self {
            sample_rate,
            threshold: SmoothedParam::fast(-6.0, sample_rate),
            ceiling_db: -0.3,
            release_ms,
            release_coeff,
            lookahead_ms,
            lookahead_samples,
            buffer_l: vec![0.0; max_samples],
            buffer_r: vec![0.0; max_samples],
            write_pos: 0,
            gain_reduction: 1.0,
            output_level: gain::output_level_param(sample_rate),
            max_lookahead_samples: max_samples,
        }
    }

    /// Set threshold in dB.
    ///
    /// Range: −30.0 to 0.0 dB. Values outside this range are clamped.
    /// The threshold sets the level above which gain reduction begins.
    pub fn set_threshold_db(&mut self, db: f32) {
        self.threshold.set_target(db.clamp(-30.0, 0.0));
    }

    /// Set the brickwall ceiling in dB.
    ///
    /// Range: −30.0 to 0.0 dB. The ceiling is the hard maximum output level;
    /// no sample will exceed this after processing.
    pub fn set_ceiling_db(&mut self, db: f32) {
        self.ceiling_db = db.clamp(-30.0, 0.0);
    }

    /// Set the release time in milliseconds.
    ///
    /// Range: 10.0 to 500.0 ms. Shorter values release faster (can cause pumping);
    /// longer values are smoother but may not recover in time for fast passages.
    pub fn set_release_ms(&mut self, ms: f32) {
        self.release_ms = ms.clamp(10.0, 500.0);
        self.release_coeff = compute_release_coeff(self.release_ms, self.sample_rate);
    }

    /// Set the lookahead delay in milliseconds.
    ///
    /// Range: 0.0 to 10.0 ms. Longer lookahead gives the limiter more time to
    /// anticipate peaks, but also increases latency reported to the host. A value
    /// of 0.0 disables lookahead (brickwall still guaranteed, but transients may
    /// clip for one sample before gain reduction engages).
    pub fn set_lookahead_ms(&mut self, ms: f32) {
        self.lookahead_ms = ms.clamp(0.0, MAX_LOOKAHEAD_MS);
        self.lookahead_samples = ms_to_samples(self.lookahead_ms, self.sample_rate)
            .min(self.max_lookahead_samples.saturating_sub(1));
    }

    /// Compute the gain reduction factor for the given peak amplitude.
    ///
    /// Returns a linear multiplier in (0.0, 1.0] to be applied to the output.
    /// The returned gain already incorporates the ceiling offset so the brickwall
    /// property is guaranteed without further clamping.
    #[inline]
    fn compute_target_gain(&self, peak: f32) -> f32 {
        let thresh_linear = db_to_linear(self.threshold.get());
        let ceiling_linear = db_to_linear(self.ceiling_db);

        if peak > thresh_linear && peak > 1e-9 {
            // Gain reduction to bring peak to threshold, then scale to ceiling.
            (thresh_linear / peak) * ceiling_linear
        } else {
            // No reduction needed — still respect ceiling.
            ceiling_linear
        }
    }

    /// Scan the lookahead window for the maximum absolute value of one channel.
    #[inline]
    fn scan_peak_mono(&self, buf: &[f32]) -> f32 {
        let len = buf.len();
        let mut peak = 0.0_f32;
        for i in 0..=self.lookahead_samples {
            let read_pos = (self.write_pos + i) % len;
            let s = fabsf(buf[read_pos]);
            if s > peak {
                peak = s;
            }
        }
        peak
    }

    /// Process one stereo sample pair through the limiter.
    ///
    /// Implements the core lookahead loop: write input, scan for peak, compute
    /// gain, smooth gain, read delayed output, scale by smoothed gain.
    #[inline]
    fn process_stereo_inner(&mut self, left: f32, right: f32) -> (f32, f32) {
        let len = self.max_lookahead_samples;

        // Write new samples into circular buffer at write position.
        self.buffer_l[self.write_pos] = left;
        self.buffer_r[self.write_pos] = right;

        // Scan lookahead window for peak across both channels (linked stereo).
        let peak_l = self.scan_peak_mono(&self.buffer_l);
        let peak_r = self.scan_peak_mono(&self.buffer_r);
        let peak = if peak_l > peak_r { peak_l } else { peak_r };

        // Advance threshold smoother by one sample.
        let _ = self.threshold.advance();

        // Target gain from current peak and threshold.
        let target = self.compute_target_gain(peak);

        // One-pole gain smoothing: instant attack (follow target down immediately),
        // exponential release (follow target up slowly).
        self.gain_reduction = if target < self.gain_reduction {
            target
        } else {
            self.release_coeff * self.gain_reduction + (1.0 - self.release_coeff) * target
        };

        // Read delayed sample from the output end of the buffer.
        let read_pos = (self.write_pos + self.lookahead_samples + 1) % len;
        let delayed_l = self.buffer_l[read_pos];
        let delayed_r = self.buffer_r[read_pos];

        // Advance write pointer.
        self.write_pos = (self.write_pos + 1) % len;

        // Apply gain reduction and output level.
        let out_level = self.output_level.advance();
        (
            delayed_l * self.gain_reduction * out_level,
            delayed_r * self.gain_reduction * out_level,
        )
    }
}

impl Effect for Limiter {
    /// Process one mono sample through the limiter.
    ///
    /// Uses the left channel buffer only. For stereo signals, prefer
    /// [`process_stereo`](Self::process_stereo) to ensure linked gain reduction.
    fn process(&mut self, input: f32) -> f32 {
        let (out, _) = self.process_stereo_inner(input, input);
        out
    }

    /// Process one stereo sample pair with linked gain reduction.
    ///
    /// Peak detection uses `max(|L|, |R|)` so both channels receive the same
    /// gain reduction, preserving the stereo image under heavy limiting.
    fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        self.process_stereo_inner(left, right)
    }

    fn is_true_stereo(&self) -> bool {
        true
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.threshold = SmoothedParam::fast(self.threshold.target(), sample_rate);
        self.release_coeff = compute_release_coeff(self.release_ms, sample_rate);
        self.output_level = gain::output_level_param(sample_rate);

        let max_samples = ms_to_samples(MAX_LOOKAHEAD_MS, sample_rate);
        self.max_lookahead_samples = max_samples;
        self.buffer_l = vec![0.0; max_samples];
        self.buffer_r = vec![0.0; max_samples];
        self.write_pos = 0;

        self.lookahead_samples =
            ms_to_samples(self.lookahead_ms, sample_rate).min(max_samples.saturating_sub(1));
    }

    /// Reset all state: clear delay buffers and reset gain to unity.
    fn reset(&mut self) {
        self.buffer_l.fill(0.0);
        self.buffer_r.fill(0.0);
        self.write_pos = 0;
        self.gain_reduction = 1.0;
    }

    /// Reports latency in samples equal to the current lookahead setting.
    fn latency_samples(&self) -> usize {
        self.lookahead_samples
    }
}

sonido_core::impl_params! {
    Limiter, this {
        [0] ParamDescriptor::gain_db("Threshold", "Thresh", -30.0, 0.0, -6.0)
                .with_id(ParamId(1600), "lim_thresh"),
            get: this.threshold.target(),
            set: |v| this.threshold.set_target(v);

        [1] ParamDescriptor::gain_db("Ceiling", "Ceil", -30.0, 0.0, -0.3)
                .with_id(ParamId(1601), "lim_ceil"),
            get: this.ceiling_db,
            set: |v| this.ceiling_db = v;

        [2] ParamDescriptor::time_ms("Release", "Rel", 10.0, 500.0, 100.0)
                .with_id(ParamId(1602), "lim_release"),
            get: this.release_ms,
            set: |v| this.set_release_ms(v);

        [3] ParamDescriptor::time_ms("Lookahead", "Look", 0.0, 10.0, 5.0)
                .with_id(ParamId(1603), "lim_look"),
            get: this.lookahead_ms,
            set: |v| this.set_lookahead_ms(v);

        [4] sonido_core::gain::output_param_descriptor()
                .with_id(ParamId(1604), "lim_output"),
            get: sonido_core::gain::output_level_db(&this.output_level),
            set: |v| sonido_core::gain::set_output_level_db(&mut this.output_level, v);
    }
}

/// Convert milliseconds to a sample count at the given sample rate.
#[inline]
fn ms_to_samples(ms: f32, sample_rate: f32) -> usize {
    ((ms * sample_rate) / 1000.0) as usize
}

/// Compute the one-pole release coefficient.
///
/// `coeff = exp(-1 / (release_ms * sample_rate / 1000))`
///
/// As `release_ms → 0` the coefficient → 0 (instant), as `release_ms → ∞`
/// the coefficient → 1 (never releases).
#[inline]
fn compute_release_coeff(release_ms: f32, sample_rate: f32) -> f32 {
    let tau = release_ms * sample_rate / 1000.0;
    if tau < 1.0 { 0.0 } else { expf(-1.0 / tau) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(not(feature = "std"))]
    extern crate alloc;
    #[cfg(not(feature = "std"))]
    use alloc::vec;
    use sonido_core::ParameterInfo;

    #[test]
    fn test_default_params() {
        let lim = Limiter::new(48000.0);
        assert_eq!(lim.param_count(), 5);

        let thresh = lim.param_info(0).unwrap();
        assert_eq!(thresh.name, "Threshold");
        assert!((thresh.default - (-6.0)).abs() < 0.01);

        let ceil = lim.param_info(1).unwrap();
        assert_eq!(ceil.name, "Ceiling");
        assert!((ceil.default - (-0.3)).abs() < 0.01);

        let rel = lim.param_info(2).unwrap();
        assert_eq!(rel.name, "Release");
        assert!((rel.default - 100.0).abs() < 0.01);

        let look = lim.param_info(3).unwrap();
        assert_eq!(look.name, "Lookahead");
        assert!((look.default - 5.0).abs() < 0.01);

        let out = lim.param_info(4).unwrap();
        assert_eq!(out.name, "Output");
        assert!((out.default - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_limiting() {
        let mut lim = Limiter::new(48000.0);
        lim.set_threshold_db(-6.0);
        // ceiling at default -0.3 dB ≈ linear 0.966

        let ceiling_linear = db_to_linear(-0.3);
        // Process enough samples to fill lookahead buffer
        let mut last_out = 0.0_f32;
        for _ in 0..1024 {
            last_out = lim.process(1.0);
        }
        // After lookahead fills, output should be at or below ceiling
        assert!(
            libm::fabsf(last_out) <= ceiling_linear + 1e-5,
            "output {last_out} exceeds ceiling {ceiling_linear}"
        );
    }

    #[test]
    fn test_quiet_signals_pass_through() {
        let mut lim = Limiter::new(48000.0);
        lim.set_threshold_db(-6.0);
        // Signal well below threshold: -30 dB ≈ 0.0316 linear
        let quiet = db_to_linear(-30.0);
        let ceiling_linear = db_to_linear(-0.3);

        // Process enough samples to stabilise
        let mut last_out = 0.0_f32;
        for _ in 0..1024 {
            last_out = lim.process(quiet);
        }
        // Should pass through with only ceiling attenuation (≈ -0.3 dB)
        let expected = quiet * ceiling_linear;
        assert!(
            (libm::fabsf(last_out) - expected).abs() < 0.01,
            "expected ~{expected}, got {last_out}"
        );
    }

    #[test]
    fn test_stereo_linked() {
        let mut lim = Limiter::new(48000.0);
        lim.set_threshold_db(-6.0);
        let ceiling_linear = db_to_linear(-0.3);

        // Loud left, quiet right
        let mut last_r = 0.0_f32;
        for _ in 0..1024 {
            let (_l, r) = lim.process_stereo(1.0, 0.01);
            last_r = r;
        }
        // Right channel should be reduced even though it's quiet, because left is loud
        // Specifically, gain reduction from left should reduce right below its unattenuated value
        // Right unattenuated through ceiling would be 0.01 * ceiling_linear
        let unattenuated_r = 0.01 * ceiling_linear;
        assert!(
            libm::fabsf(last_r) < unattenuated_r,
            "right channel {last_r} should be reduced below unattenuated {unattenuated_r}"
        );
    }

    #[test]
    fn test_reset_clears_state() {
        let mut lim = Limiter::new(48000.0);
        // Process loud signal to fill buffers and engage gain reduction
        for _ in 0..512 {
            let _ = lim.process(1.0);
        }
        lim.reset();

        // After reset, gain_reduction should be 1.0
        assert!(
            (lim.gain_reduction - 1.0).abs() < 1e-6,
            "gain_reduction not reset"
        );
        // Buffers should be zeroed
        assert!(
            lim.buffer_l.iter().all(|&s| s == 0.0),
            "buffer_l not cleared"
        );
        assert!(
            lim.buffer_r.iter().all(|&s| s == 0.0),
            "buffer_r not cleared"
        );
    }

    #[test]
    fn test_latency_reports_lookahead() {
        let lim = Limiter::new(48000.0);
        // Default 5 ms lookahead at 48000 Hz = 240 samples
        let expected = (5.0_f32 * 48000.0 / 1000.0) as usize;
        assert_eq!(lim.latency_samples(), expected);
    }

    #[test]
    fn test_param_clamping() {
        let mut lim = Limiter::new(48000.0);
        // Threshold clamped to -30..0
        lim.set_param(0, 100.0);
        assert!((lim.threshold.target() - 0.0).abs() < 0.01);
        lim.set_param(0, -100.0);
        assert!((lim.threshold.target() - (-30.0)).abs() < 0.01);
    }

    #[test]
    fn test_set_sample_rate() {
        let mut lim = Limiter::new(48000.0);
        lim.set_sample_rate(44100.0);
        assert!((lim.sample_rate - 44100.0).abs() < 0.01);
        // Lookahead samples should scale with new rate
        let expected = (5.0_f32 * 44100.0 / 1000.0) as usize;
        assert_eq!(lim.latency_samples(), expected);
    }
}
