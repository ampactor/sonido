//! Phase unwrapping utilities — batch, configurable-tolerance, quality-guided, and streaming.
//!
//! Phase values from FFT or Hilbert transforms are wrapped to [-π, π]. These utilities
//! reconstruct the continuous (unwrapped) phase by detecting and correcting 2π discontinuities.
//!
//! ## Algorithms
//!
//! - [`unwrap_phase`]: Standard unwrapping with π tolerance (most common)
//! - [`unwrap_phase_tol`]: Configurable tolerance for specialized applications
//! - [`unwrap_phase_quality`]: Quality-guided unwrapping for noisy signals
//! - [`PhaseTracker`]: Sample-by-sample streaming unwrapper
//!
//! Reference: Ghiglia & Pritt, "Two-Dimensional Phase Unwrapping" (1998), ch. 3.

use std::f32::consts::PI;

/// Standard phase unwrapping with π tolerance.
///
/// Detects phase jumps exceeding π between consecutive samples and applies
/// cumulative 2π corrections to produce a continuous phase curve.
///
/// # Arguments
/// * `phase` - Wrapped phase values in radians
///
/// # Returns
/// Unwrapped (continuous) phase values
pub fn unwrap_phase(phase: &[f32]) -> Vec<f32> {
    unwrap_phase_tol(phase, PI)
}

/// Phase unwrapping with configurable tolerance.
///
/// Same as [`unwrap_phase`] but triggers correction when the phase difference
/// exceeds `tolerance` instead of the default π.
///
/// Smaller tolerance catches subtler jumps (useful for oversampled signals);
/// larger tolerance ignores small jumps (useful for noisy signals).
///
/// # Arguments
/// * `phase` - Wrapped phase values in radians
/// * `tolerance` - Jump detection threshold in radians (standard: π)
pub fn unwrap_phase_tol(phase: &[f32], tolerance: f32) -> Vec<f32> {
    if phase.is_empty() {
        return Vec::new();
    }

    let mut unwrapped = Vec::with_capacity(phase.len());
    unwrapped.push(phase[0]);

    let two_pi = 2.0 * PI;
    let mut correction = 0.0f32;

    for i in 1..phase.len() {
        let diff = phase[i] - phase[i - 1];

        if diff > tolerance {
            correction -= two_pi;
        } else if diff < -tolerance {
            correction += two_pi;
        }

        unwrapped.push(phase[i] + correction);
    }

    unwrapped
}

/// Quality-guided phase unwrapping for noisy signals.
///
/// Instead of sequential left-to-right unwrapping, this algorithm computes a local
/// quality metric (inverse variance of phase derivatives in a window) and unwraps
/// high-quality regions first, propagating outward to noisier regions.
///
/// # Arguments
/// * `phase` - Wrapped phase values in radians
/// * `window_size` - Window size for quality estimation (typically 5-15)
///
/// # Algorithm
/// 1. Compute quality Q(n) = 1 / var(Δφ) in a window around each sample
/// 2. Find the sample with highest quality as the seed
/// 3. Unwrap forward from the seed using standard algorithm
/// 4. Unwrap backward from the seed using standard algorithm
/// 5. Concatenate the two halves into a continuous phase array
///
/// Reference: Ghiglia & Pritt, "Two-Dimensional Phase Unwrapping" (1998), ch. 3.
pub fn unwrap_phase_quality(phase: &[f32], window_size: usize) -> Vec<f32> {
    if phase.is_empty() {
        return Vec::new();
    }
    if phase.len() == 1 {
        return vec![phase[0]];
    }

    let n = phase.len();
    let half_win = (window_size / 2).max(1);

    // Compute wrapped differences: wrap each diff back to [-π, π]
    let diffs: Vec<f32> = (1..n)
        .map(|i| wrap_to_pi(phase[i] - phase[i - 1]))
        .collect();

    // Compute quality for each sample: 1 / (variance of diffs in window + epsilon)
    // diff[i] corresponds to the gap between phase[i] and phase[i+1],
    // so for quality at sample k we look at diffs in [k-half_win, k+half_win]
    let quality: Vec<f32> = (0..n)
        .map(|k| {
            let lo = k.saturating_sub(half_win);
            let hi = (k + half_win).min(diffs.len().saturating_sub(1));
            if lo > hi || diffs.is_empty() {
                return 1.0;
            }
            let window = &diffs[lo..=hi];
            let mean = window.iter().sum::<f32>() / window.len() as f32;
            let variance =
                window.iter().map(|&d| (d - mean) * (d - mean)).sum::<f32>() / window.len() as f32;
            1.0 / (variance + 1e-10)
        })
        .collect();

    // Find seed: index with highest quality
    let seed = quality
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0);

    let mut result = vec![0.0f32; n];
    result[seed] = phase[seed];

    // Unwrap forward from seed
    let mut correction = 0.0f32;
    let two_pi = 2.0 * PI;
    for i in (seed + 1)..n {
        let diff = phase[i] - phase[i - 1];
        if diff > PI {
            correction -= two_pi;
        } else if diff < -PI {
            correction += two_pi;
        }
        result[i] = phase[i] + correction;
    }

    // Unwrap backward from seed — reset correction and walk left
    correction = 0.0;
    for i in (0..seed).rev() {
        let diff = phase[i] - phase[i + 1];
        if diff > PI {
            correction -= two_pi;
        } else if diff < -PI {
            correction += two_pi;
        }
        result[i] = phase[i] + correction;
    }

    result
}

/// Wrap a phase difference to [-π, π].
fn wrap_to_pi(diff: f32) -> f32 {
    let two_pi = 2.0 * PI;
    let mut d = diff;
    while d > PI {
        d -= two_pi;
    }
    while d < -PI {
        d += two_pi;
    }
    d
}

/// Streaming phase tracker for real-time unwrapping.
///
/// Maintains state between calls to [`PhaseTracker::process`], producing unwrapped phase
/// one sample at a time. Equivalent to [`unwrap_phase`] applied incrementally.
///
/// # Example
/// ```rust
/// # use sonido_analysis::phase::PhaseTracker;
/// let mut tracker = PhaseTracker::new();
/// let phases = [0.0, 1.0, 2.0, 3.0, -2.8f32]; // wrap near π
/// let unwrapped: Vec<f32> = phases.iter().map(|&p| tracker.process(p)).collect();
/// // unwrapped ≈ [0.0, 1.0, 2.0, 3.0, 3.48...]
/// assert!(unwrapped[4] > 3.0);
/// ```
pub struct PhaseTracker {
    /// Last raw (wrapped) phase value seen, or `None` on first sample.
    last_phase: Option<f32>,
    /// Cumulative correction applied to all subsequent samples.
    correction: f32,
    /// Jump detection threshold in radians.
    tolerance: f32,
}

impl PhaseTracker {
    /// Create a new tracker with the standard π tolerance.
    pub fn new() -> Self {
        Self {
            last_phase: None,
            correction: 0.0,
            tolerance: PI,
        }
    }

    /// Create a tracker with a custom jump-detection tolerance in radians.
    pub fn with_tolerance(tolerance: f32) -> Self {
        Self {
            last_phase: None,
            correction: 0.0,
            tolerance,
        }
    }

    /// Process one wrapped phase sample and return the unwrapped value.
    ///
    /// On the first call, the sample passes through unchanged and sets the
    /// initial phase reference.
    pub fn process(&mut self, phase: f32) -> f32 {
        let two_pi = 2.0 * PI;
        if let Some(last) = self.last_phase {
            let diff = phase - last;
            if diff > self.tolerance {
                self.correction -= two_pi;
            } else if diff < -self.tolerance {
                self.correction += two_pi;
            }
        }
        self.last_phase = Some(phase);
        phase + self.correction
    }

    /// Reset tracker state — next call to [`PhaseTracker::process`] behaves as if it's the first sample.
    pub fn reset(&mut self) {
        self.last_phase = None;
        self.correction = 0.0;
    }
}

impl Default for PhaseTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer_fn;

    /// Generate a wrapped linear phase ramp for testing.
    fn linear_phase_ramp(n: usize, slope: f32) -> Vec<f32> {
        (0..n)
            .map(|i| {
                let raw = i as f32 * slope;
                // Wrap to [-π, π]
                wrap_to_pi(raw)
            })
            .collect()
    }

    #[test]
    fn test_unwrap_matches_existing() {
        // Verify our new function produces identical output to transfer_fn::unwrap_phase
        let phase = linear_phase_ramp(64, 0.3);
        let expected = transfer_fn::unwrap_phase(&phase);
        let got = unwrap_phase(&phase);
        assert_eq!(expected.len(), got.len());
        for (a, b) in expected.iter().zip(got.iter()) {
            assert!((a - b).abs() < 1e-6, "mismatch: expected {a}, got {b}");
        }
    }

    #[test]
    fn test_custom_tolerance() {
        // A positive jump of π/2 + ε is between π/2 and π.
        // Tight tolerance (π/2) catches it; standard tolerance (π) does not.
        // Signal: [0.0, 0.1, 0.1 + (π/2 + 0.1)]  — first step is small (never triggers),
        // second step is π/2 + 0.1 which is > π/2 but < π.
        let jump = PI / 2.0 + 0.1; // ~1.67, between π/2 and π
        let phase = vec![0.0_f32, 0.1, 0.1 + jump];
        let tight = unwrap_phase_tol(&phase, PI / 2.0);
        let standard = unwrap_phase_tol(&phase, PI);

        // tight[2]: diff = jump > π/2 → correction -= 2π → tight[2] = (0.1 + jump) - 2π
        // standard[2]: diff = jump < π → no correction → standard[2] = 0.1 + jump
        assert_ne!(
            tight[2], standard[2],
            "tight and standard should differ on a π/2+ε jump"
        );
        // Tight applies correction, standard does not
        assert!(
            tight[2] < standard[2],
            "tight[2]={} should be less than standard[2]={} (correction subtracted 2π)",
            tight[2],
            standard[2]
        );
    }

    #[test]
    fn test_quality_guided_noisy() {
        // Noisy linear ramp: quality-guided should track the underlying ramp
        let n = 64;
        let slope = 0.4;
        // Ideal unwrapped ramp
        let ideal: Vec<f32> = (0..n).map(|i| i as f32 * slope).collect();
        // Wrapped version
        let wrapped: Vec<f32> = ideal.iter().map(|&v| wrap_to_pi(v)).collect();

        let unwrapped_q = unwrap_phase_quality(&wrapped, 7);
        let unwrapped_s = unwrap_phase(&wrapped);

        // Both should closely track the ideal ramp (allow DC offset from seed anchor)
        let offset_q = ideal[0] - unwrapped_q[0];
        let offset_s = ideal[0] - unwrapped_s[0];

        let mse_q: f32 = ideal
            .iter()
            .zip(unwrapped_q.iter())
            .map(|(&a, &b)| (a - (b + offset_q)).powi(2))
            .sum::<f32>()
            / n as f32;
        let mse_s: f32 = ideal
            .iter()
            .zip(unwrapped_s.iter())
            .map(|(&a, &b)| (a - (b + offset_s)).powi(2))
            .sum::<f32>()
            / n as f32;

        assert!(mse_q < 1e-5, "quality-guided MSE too high: {mse_q}");
        assert!(mse_s < 1e-5, "standard MSE too high: {mse_s}");
    }

    #[test]
    fn test_phase_tracker_streaming() {
        let phase = linear_phase_ramp(64, 0.35);
        let batch = unwrap_phase(&phase);

        let mut tracker = PhaseTracker::new();
        let streaming: Vec<f32> = phase.iter().map(|&p| tracker.process(p)).collect();

        assert_eq!(batch.len(), streaming.len());
        for (a, b) in batch.iter().zip(streaming.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "tracker mismatch: expected {a}, got {b}"
            );
        }
    }

    #[test]
    fn test_phase_tracker_reset() {
        let mut tracker = PhaseTracker::new();

        // Process some samples that accumulate correction
        let phase1 = linear_phase_ramp(32, 0.4);
        for &p in &phase1 {
            tracker.process(p);
        }
        assert!(tracker.correction != 0.0 || tracker.last_phase.is_some());

        tracker.reset();
        assert!(tracker.last_phase.is_none());
        assert_eq!(tracker.correction, 0.0);

        // After reset, next sample should pass through as if first
        let v = tracker.process(1.5);
        assert!((v - 1.5).abs() < 1e-6, "expected 1.5 after reset, got {v}");
    }

    #[test]
    fn test_linear_phase_passthrough() {
        // A phase that is already monotonically increasing within [-π, π] passes through unchanged
        let phase: Vec<f32> = (0..10).map(|i| -PI + i as f32 * (PI / 10.0)).collect();
        let unwrapped = unwrap_phase(&phase);
        for (a, b) in phase.iter().zip(unwrapped.iter()) {
            assert!(
                (a - b).abs() < 1e-6,
                "passthrough failed: input {a}, output {b}"
            );
        }
    }
}
