//! Mathematical utility functions for DSP.
//!
//! Provides common DSP math operations optimized for real-time audio processing.
//! All functions are designed to be allocation-free and suitable for `no_std`.
//!
//! # Level Conversions
//!
//! - [`db_to_linear`] / [`linear_to_db`] - Convert between dB and linear gain
//!
//! # Waveshaping / Clipping
//!
//! Different clipping functions produce different harmonic characteristics:
//!
//! | Function | Character | Harmonics | Use Case |
//! |----------|-----------|-----------|----------|
//! | [`soft_clip`] | Smooth, warm | Odd | Tube amp simulation |
//! | [`hard_clip`] | Harsh, buzzy | Odd (many) | Transistor fuzz |
//! | [`foldback`] | Complex, synthy | Even + Odd | Synth distortion |
//! | [`asymmetric_clip`] | Warm, tube-like | Even + Odd | Vintage amps |
//!
//! # Utilities
//!
//! - [`lerp`] - Linear interpolation
//! - [`clamp`] - Value limiting
//! - [`hz_to_omega`] - Frequency to angular frequency
//! - [`ms_to_samples`] / [`samples_to_ms`] - Time conversions

use libm::{expf, logf, tanhf};

/// Convert decibels to linear gain.
///
/// # Arguments
/// * `db` - Value in decibels
///
/// # Returns
/// Linear gain value (e.g., 0 dB → 1.0, -6 dB → 0.5, +6 dB → 2.0)
///
/// # Example
/// ```rust
/// use sonido_core::db_to_linear;
///
/// assert!((db_to_linear(0.0) - 1.0).abs() < 0.001);
/// assert!((db_to_linear(-6.02) - 0.5).abs() < 0.01);
/// ```
#[inline]
pub fn db_to_linear(db: f32) -> f32 {
    // 10^(dB/20) = e^(dB * ln(10)/20)
    const FACTOR: f32 = core::f32::consts::LN_10 / 20.0;
    expf(db * FACTOR)
}

/// Convert linear gain to decibels.
///
/// # Arguments
/// * `linear` - Linear gain value (must be > 0)
///
/// # Returns
/// Value in decibels
///
/// # Example
/// ```rust
/// use sonido_core::linear_to_db;
///
/// assert!((linear_to_db(1.0) - 0.0).abs() < 0.001);
/// assert!((linear_to_db(0.5) - (-6.02)).abs() < 0.01);
/// ```
#[inline]
pub fn linear_to_db(linear: f32) -> f32 {
    // 20 * log10(linear) = 20 * ln(linear) / ln(10)
    const FACTOR: f32 = 20.0 / core::f32::consts::LN_10;
    logf(linear.max(1e-10)) * FACTOR
}

/// Fast hyperbolic tangent approximation.
///
/// Uses the actual tanh function from libm for accuracy.
/// This is suitable for soft clipping and saturation effects.
///
/// # Arguments
/// * `x` - Input value
///
/// # Returns
/// tanh(x), in range (-1, 1)
#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    tanhf(x)
}

/// Soft clip using hyperbolic tangent.
///
/// Smooth saturation that approaches ±1 asymptotically.
/// Produces primarily odd harmonics, similar to tube amplifiers.
///
/// # Arguments
/// * `x` - Input sample (any range)
///
/// # Returns
/// Soft-clipped output in range (-1, 1)
#[inline]
pub fn soft_clip(x: f32) -> f32 {
    tanhf(x)
}

/// Hard clip to ±threshold range.
///
/// Abrupt limiting that creates flat tops on waveforms.
/// Produces harsh odd harmonics.
///
/// # Arguments
/// * `x` - Input sample
/// * `threshold` - Clipping threshold (default 1.0)
///
/// # Returns
/// Hard-clipped output in range [-threshold, threshold]
#[inline]
pub fn hard_clip(x: f32, threshold: f32) -> f32 {
    x.clamp(-threshold, threshold)
}

/// Foldback distortion.
///
/// When |x| exceeds threshold, the signal "folds" back instead of clipping.
/// Creates rich harmonic content, popular in synthesizers.
///
/// # Arguments
/// * `x` - Input sample
/// * `threshold` - Folding threshold
///
/// # Returns
/// Foldback-distorted output
#[inline]
pub fn foldback(x: f32, threshold: f32) -> f32 {
    if x.abs() <= threshold {
        x
    } else {
        // Fold the signal back
        let sign = x.signum();
        let excess = x.abs() - threshold;
        let folded = threshold - excess;
        // Recursive fold for high drive
        if folded < -threshold {
            foldback(sign * folded, threshold)
        } else {
            sign * folded
        }
    }
}

/// Asymmetric soft clipping.
///
/// Positive and negative halves clip differently, producing
/// both even and odd harmonics (warmer, tube-like character).
///
/// # Arguments
/// * `x` - Input sample
///
/// # Returns
/// Asymmetrically clipped output
#[inline]
pub fn asymmetric_clip(x: f32) -> f32 {
    if x >= 0.0 {
        // Positive: gentler clipping
        tanhf(x)
    } else {
        // Negative: harder clipping (reaches limit faster)
        tanhf(x * 1.5) / 1.5 * 1.2
    }
}

/// Linear interpolation between two values.
///
/// # Arguments
/// * `a` - Start value (at t=0)
/// * `b` - End value (at t=1)
/// * `t` - Interpolation factor (0.0 to 1.0)
///
/// # Returns
/// Interpolated value
#[inline]
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Clamp a value to a range.
///
/// # Arguments
/// * `x` - Input value
/// * `min` - Minimum value
/// * `max` - Maximum value
///
/// # Returns
/// Clamped value
#[inline]
pub fn clamp(x: f32, min: f32, max: f32) -> f32 {
    x.clamp(min, max)
}

/// Convert frequency in Hz to angular frequency (radians/sample).
///
/// # Arguments
/// * `freq_hz` - Frequency in Hz
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Angular frequency in radians per sample
#[inline]
pub fn hz_to_omega(freq_hz: f32, sample_rate: f32) -> f32 {
    core::f32::consts::TAU * freq_hz / sample_rate
}

/// Convert milliseconds to samples.
///
/// # Arguments
/// * `ms` - Time in milliseconds
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Time in samples
#[inline]
pub fn ms_to_samples(ms: f32, sample_rate: f32) -> f32 {
    ms * sample_rate / 1000.0
}

/// Convert samples to milliseconds.
///
/// # Arguments
/// * `samples` - Time in samples
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Time in milliseconds
#[inline]
pub fn samples_to_ms(samples: f32, sample_rate: f32) -> f32 {
    samples * 1000.0 / sample_rate
}

/// Flush subnormal (denormalized) floats to zero.
///
/// Subnormal floats (~1e-38 to 1e-45) cause severe CPU performance
/// degradation on most architectures (up to 100x slowdown). This function
/// replaces values below 1e-20 with zero, providing margin before the
/// IEEE 754 subnormal range begins.
///
/// Use this in feedback loops (comb filters, delay lines, allpass chains)
/// where signal can decay indefinitely toward zero.
///
/// Reference: IEEE 754-2008, Section 3.4 (Subnormal numbers)
#[allow(clippy::inline_always)]
#[inline(always)]
pub fn flush_denormal(x: f32) -> f32 {
    if x.abs() < 1e-20 { 0.0 } else { x }
}

/// Crossfade between dry and wet signals.
///
/// Equivalent to `dry * (1 - mix) + wet * mix` but uses one fewer multiply:
/// `dry + (wet - dry) * mix`.
///
/// # Arguments
///
/// * `dry` - Unprocessed signal
/// * `wet` - Processed signal
/// * `mix` - Blend factor in \[0.0, 1.0\]: 0.0 = all dry, 1.0 = all wet
#[inline]
pub fn wet_dry_mix(dry: f32, wet: f32, mix: f32) -> f32 {
    dry + (wet - dry) * mix
}

/// Stereo crossfade between dry and wet signals.
///
/// Applies [`wet_dry_mix`] independently to left and right channels.
#[inline]
pub fn wet_dry_mix_stereo(dry_l: f32, dry_r: f32, wet_l: f32, wet_r: f32, mix: f32) -> (f32, f32) {
    (
        wet_dry_mix(dry_l, wet_l, mix),
        wet_dry_mix(dry_r, wet_r, mix),
    )
}

/// Sum stereo to mono (equal-power average).
#[inline]
pub fn mono_sum(left: f32, right: f32) -> f32 {
    (left + right) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_linear_roundtrip() {
        let original = 0.5;
        let db = linear_to_db(original);
        let back = db_to_linear(db);
        assert!(
            (original - back).abs() < 1e-5,
            "Roundtrip failed: {} -> {} -> {}",
            original,
            db,
            back
        );
    }

    #[test]
    fn test_db_known_values() {
        // 0 dB = 1.0 linear
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        // -6 dB ≈ 0.5 linear
        assert!((db_to_linear(-6.0206) - 0.5).abs() < 0.001);
        // +6 dB ≈ 2.0 linear
        assert!((db_to_linear(6.0206) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_soft_clip_bounds() {
        assert!(soft_clip(3.0) < 1.0);
        assert!(soft_clip(3.0) > 0.99);
        assert!(soft_clip(-3.0) > -1.0);
        assert!(soft_clip(-3.0) < -0.99);
    }

    #[test]
    fn test_foldback() {
        let threshold = 0.8;
        // Below threshold: unchanged
        assert!((foldback(0.5, threshold) - 0.5).abs() < 1e-6);
        // At threshold: unchanged
        assert!((foldback(0.8, threshold) - 0.8).abs() < 1e-6);
        // Above threshold: folds back
        let folded = foldback(1.0, threshold);
        assert!((folded - 0.6).abs() < 1e-6, "Expected 0.6, got {}", folded);
    }

    #[test]
    fn test_lerp() {
        assert_eq!(lerp(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp(0.0, 10.0, 0.5), 5.0);
        assert_eq!(lerp(0.0, 10.0, 1.0), 10.0);
    }

    #[test]
    fn test_ms_samples_conversion() {
        let sample_rate = 48000.0;
        let ms = 10.0;
        let samples = ms_to_samples(ms, sample_rate);
        assert_eq!(samples, 480.0);
        let back = samples_to_ms(samples, sample_rate);
        assert!((back - ms).abs() < 1e-6);
    }

    #[test]
    fn test_wet_dry_mix() {
        // All dry
        assert_eq!(wet_dry_mix(1.0, 0.5, 0.0), 1.0);
        // All wet
        assert_eq!(wet_dry_mix(1.0, 0.5, 1.0), 0.5);
        // 50/50
        assert!((wet_dry_mix(0.0, 1.0, 0.5) - 0.5).abs() < 1e-6);
        // Equivalent to dry*(1-mix)+wet*mix
        let dry = 0.3;
        let wet = 0.8;
        let mix = 0.7;
        let expected = dry * (1.0 - mix) + wet * mix;
        assert!((wet_dry_mix(dry, wet, mix) - expected).abs() < 1e-6);
    }

    #[test]
    fn test_wet_dry_mix_stereo() {
        let (l, r) = wet_dry_mix_stereo(1.0, 0.5, 0.0, 1.0, 0.5);
        assert!((l - 0.5).abs() < 1e-6);
        assert!((r - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_mono_sum() {
        assert_eq!(mono_sum(1.0, 1.0), 1.0);
        assert_eq!(mono_sum(1.0, -1.0), 0.0);
        assert_eq!(mono_sum(0.5, 0.3), 0.4);
    }

    #[test]
    fn test_flush_denormal() {
        // Normal values pass through
        assert_eq!(flush_denormal(1.0), 1.0);
        assert_eq!(flush_denormal(-0.5), -0.5);
        assert_eq!(flush_denormal(1e-10), 1e-10);

        // Subnormal-range values are flushed to zero
        assert_eq!(flush_denormal(1e-21), 0.0);
        assert_eq!(flush_denormal(-1e-21), 0.0);
        assert_eq!(flush_denormal(1e-38), 0.0);
        assert_eq!(flush_denormal(0.0), 0.0);
    }
}
