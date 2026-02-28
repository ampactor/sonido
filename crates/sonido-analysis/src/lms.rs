//! LMS and NLMS adaptive filters for noise cancellation, echo cancellation,
//! and system identification.
//!
//! Adaptive filters adjust their coefficients over time to minimize an error signal,
//! making them suitable for applications where the target filter is unknown or
//! time-varying.
//!
//! # Algorithm
//!
//! ## LMS (Least Mean Squares)
//!
//! The LMS algorithm updates filter weights using a stochastic gradient descent step:
//!
//! ```text
//! y[n]   = w^T x[n]          (filter output)
//! e[n]   = d[n] - y[n]       (error: desired minus output)
//! w[k]  += μ * e[n] * x[n-k]  (weight update for tap k)
//! ```
//!
//! Stability requires: `0 < μ < 2 / (order * P_x)` where `P_x` is the input power.
//! A safe range in practice is `0.001 ≤ μ ≤ 0.1`.
//!
//! ## NLMS (Normalized LMS)
//!
//! NLMS normalizes the step size by the instantaneous input power, giving faster
//! convergence and better numerical stability across varying signal levels:
//!
//! ```text
//! μ_eff  = μ / (x^T x + δ)
//! w[k]  += μ_eff * e[n] * x[n-k]
//! ```
//!
//! The regularization term `δ` prevents division by zero when the input is silent.
//! Stability is guaranteed for `0 < μ < 2` (independent of signal power).
//!
//! # References
//!
//! - Haykin, "Adaptive Filter Theory" (5th ed.), chapters 5 (LMS) and 6 (NLMS).
//! - Widrow & Stearns, "Adaptive Signal Processing" (1985), chapter 6.

use std::f32;

/// Least Mean Squares adaptive filter.
///
/// Minimizes the mean square error between the filter output and a desired signal
/// by performing a stochastic gradient descent step on each input sample.
///
/// ## Parameters
/// - `order`: filter length (number of taps)
/// - `step_size`: μ (learning rate), typically 0.001 to 0.1
///
/// ## Algorithm
///
/// ```text
/// y[n]   = Σ w[k] * x[n-k]    (output)
/// e[n]   = d[n] - y[n]         (error)
/// w[k]  += μ * e[n] * x[n-k]  (weight update)
/// ```
///
/// Reference: Haykin, "Adaptive Filter Theory" (5th ed.), chapter 5.
pub struct LmsFilter {
    weights: Vec<f32>,
    buffer: Vec<f32>,
    pos: usize,
    step_size: f32,
    order: usize,
}

impl LmsFilter {
    /// Create a new LMS filter.
    ///
    /// Weights and buffer are initialized to zero.
    ///
    /// # Arguments
    ///
    /// * `order` — number of filter taps. Larger orders model more complex filters
    ///   but converge more slowly.
    /// * `step_size` — μ learning rate. Typical range: 0.001 to 0.1. Higher values
    ///   converge faster but may become unstable.
    pub fn new(order: usize, step_size: f32) -> Self {
        Self {
            weights: vec![0.0; order],
            buffer: vec![0.0; order],
            pos: 0,
            step_size,
            order,
        }
    }

    /// Process a single sample.
    ///
    /// Computes the filter output, calculates the error, then updates weights.
    ///
    /// # Arguments
    ///
    /// * `input` — current input sample x\[n\]
    /// * `desired` — desired output d\[n\] (the target signal)
    ///
    /// # Returns
    ///
    /// `(output, error)` — the filter output y\[n\] and the error e\[n\] = d\[n\] - y\[n\].
    pub fn process_sample(&mut self, input: f32, desired: f32) -> (f32, f32) {
        // Write new sample into the circular buffer
        self.buffer[self.pos] = input;

        // Compute filter output: y[n] = w^T x[n]
        let mut output = 0.0f32;
        for k in 0..self.order {
            let buf_idx = (self.pos + self.order - k) % self.order;
            output += self.weights[k] * self.buffer[buf_idx];
        }

        // Error signal
        let error = desired - output;

        // LMS weight update: w[k] += μ * e[n] * x[n-k]
        let mu_e = self.step_size * error;
        for k in 0..self.order {
            let buf_idx = (self.pos + self.order - k) % self.order;
            self.weights[k] += mu_e * self.buffer[buf_idx];
        }

        // Advance circular buffer position
        self.pos = (self.pos + 1) % self.order;

        (output, error)
    }

    /// Process a block of samples.
    ///
    /// Equivalent to calling `process_sample` for each sample in order.
    /// All slices must have the same length.
    ///
    /// # Arguments
    ///
    /// * `input` — input samples x\[n\]
    /// * `desired` — desired output samples d\[n\]
    /// * `output` — filter output samples y\[n\] (written in-place)
    /// * `error` — error samples e\[n\] = d\[n\] - y\[n\] (written in-place)
    pub fn process_block(
        &mut self,
        input: &[f32],
        desired: &[f32],
        output: &mut [f32],
        error: &mut [f32],
    ) {
        for i in 0..input.len() {
            let (y, e) = self.process_sample(input[i], desired[i]);
            output[i] = y;
            error[i] = e;
        }
    }

    /// Return a reference to the current filter weight vector.
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Reset the filter to its initial state.
    ///
    /// Zeroes all weights and the input buffer. The step size is preserved.
    pub fn reset(&mut self) {
        self.weights.fill(0.0);
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    /// Set the learning rate μ.
    ///
    /// Range: typically 0.001 to 0.1. Values outside 0 < μ < 2/(order * P_x) are unstable.
    pub fn set_step_size(&mut self, step_size: f32) {
        self.step_size = step_size;
    }
}

/// Normalized Least Mean Squares adaptive filter.
///
/// Extends LMS by normalizing the effective step size by the instantaneous input power.
/// This gives faster convergence and better robustness across varying signal levels.
///
/// ## Parameters
/// - `order`: filter length (number of taps)
/// - `step_size`: μ_normalized (0.0 to 2.0 for guaranteed stability)
/// - `regularization`: δ — prevents division by zero when input is silent (default: 1e-6)
///
/// ## Algorithm
///
/// ```text
/// μ_eff  = μ / (x^T x + δ)
/// y[n]   = Σ w[k] * x[n-k]
/// e[n]   = d[n] - y[n]
/// w[k]  += μ_eff * e[n] * x[n-k]
/// ```
///
/// Reference: Haykin, "Adaptive Filter Theory" (5th ed.), chapter 6.
pub struct NlmsFilter {
    weights: Vec<f32>,
    buffer: Vec<f32>,
    pos: usize,
    step_size: f32,
    order: usize,
    regularization: f32,
}

impl NlmsFilter {
    /// Create a new NLMS filter with default regularization (δ = 1e-6).
    ///
    /// # Arguments
    ///
    /// * `order` — number of filter taps
    /// * `step_size` — μ in range (0.0, 2.0); 0.5 is a good default
    pub fn new(order: usize, step_size: f32) -> Self {
        Self::with_regularization(order, step_size, 1e-6)
    }

    /// Create a new NLMS filter with explicit regularization.
    ///
    /// # Arguments
    ///
    /// * `order` — number of filter taps
    /// * `step_size` — μ in range (0.0, 2.0)
    /// * `regularization` — δ added to denominator to prevent divide-by-zero.
    ///   Typical: 1e-8 (tight) to 1e-4 (more stable with low-power input).
    pub fn with_regularization(order: usize, step_size: f32, regularization: f32) -> Self {
        Self {
            weights: vec![0.0; order],
            buffer: vec![0.0; order],
            pos: 0,
            step_size,
            order,
            regularization,
        }
    }

    /// Process a single sample.
    ///
    /// # Arguments
    ///
    /// * `input` — current input sample x\[n\]
    /// * `desired` — desired output d\[n\]
    ///
    /// # Returns
    ///
    /// `(output, error)` — filter output y\[n\] and error e\[n\] = d\[n\] - y\[n\].
    pub fn process_sample(&mut self, input: f32, desired: f32) -> (f32, f32) {
        // Write new sample into the circular buffer
        self.buffer[self.pos] = input;

        // Compute input power: x^T x = Σ x[n-k]^2
        let power: f32 = self.buffer.iter().map(|&x| x * x).sum();

        // Effective step size normalized by input power
        let mu_eff = self.step_size / (power + self.regularization);

        // Filter output: y[n] = w^T x[n]
        let mut output = 0.0f32;
        for k in 0..self.order {
            let buf_idx = (self.pos + self.order - k) % self.order;
            output += self.weights[k] * self.buffer[buf_idx];
        }

        // Error signal
        let error = desired - output;

        // NLMS weight update: w[k] += μ_eff * e[n] * x[n-k]
        let mu_e = mu_eff * error;
        for k in 0..self.order {
            let buf_idx = (self.pos + self.order - k) % self.order;
            self.weights[k] += mu_e * self.buffer[buf_idx];
        }

        self.pos = (self.pos + 1) % self.order;

        (output, error)
    }

    /// Process a block of samples.
    ///
    /// Equivalent to calling `process_sample` for each sample in order.
    /// All slices must have the same length.
    ///
    /// # Arguments
    ///
    /// * `input` — input samples x\[n\]
    /// * `desired` — desired output samples d\[n\]
    /// * `output` — filter output samples (written in-place)
    /// * `error` — error samples (written in-place)
    pub fn process_block(
        &mut self,
        input: &[f32],
        desired: &[f32],
        output: &mut [f32],
        error: &mut [f32],
    ) {
        for i in 0..input.len() {
            let (y, e) = self.process_sample(input[i], desired[i]);
            output[i] = y;
            error[i] = e;
        }
    }

    /// Return a reference to the current filter weight vector.
    pub fn weights(&self) -> &[f32] {
        &self.weights
    }

    /// Reset the filter to its initial state.
    ///
    /// Zeroes all weights and the input buffer. Step size and regularization are preserved.
    pub fn reset(&mut self) {
        self.weights.fill(0.0);
        self.buffer.fill(0.0);
        self.pos = 0;
    }

    /// Set the normalized learning rate μ.
    ///
    /// Range: (0.0, 2.0) for stability. Typical: 0.5 for fast convergence.
    pub fn set_step_size(&mut self, step_size: f32) {
        self.step_size = step_size;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple reproducible PRNG (Park–Miller style) for test determinism.
    fn next_rand(state: &mut u32) -> f32 {
        *state = state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        // Map to [-1, 1]
        (*state as i32 as f32) / (i32::MAX as f32)
    }

    /// Apply a fixed 3-tap FIR [0.5, -0.3, 0.1] to a signal.
    fn apply_fir(signal: &[f32]) -> Vec<f32> {
        let taps = [0.5f32, -0.3, 0.1];
        let mut out = vec![0.0f32; signal.len()];
        for n in 0..signal.len() {
            let mut y = 0.0;
            for (k, &tap) in taps.iter().enumerate() {
                if n >= k {
                    y += tap * signal[n - k];
                }
            }
            out[n] = y;
        }
        out
    }

    #[test]
    fn test_lms_converges_to_known_filter() {
        let target_taps = [0.5f32, -0.3, 0.1];
        let order = 3;
        let mut filter = LmsFilter::new(order, 0.01);

        let mut state = 42u32;
        for _ in 0..5000 {
            let x = next_rand(&mut state);
            // Desired signal is the target FIR applied to the current (single) input.
            // We pass only the scalar here; using the desired from the true FIR at each step
            // requires a proper delay line, so instead drive with a block approach.
            let _ = filter.process_sample(x, 0.0); // warm-up pass (discarded)
        }

        // Proper convergence test: feed noise through target FIR, then train LMS.
        let mut filter = LmsFilter::new(order, 0.01);
        let n_samples = 5000;
        let mut noise = vec![0.0f32; n_samples];
        let mut st = 1u32;
        for s in &mut noise {
            *s = next_rand(&mut st);
        }
        let desired = apply_fir(&noise);

        let mut out = vec![0.0f32; n_samples];
        let mut err = vec![0.0f32; n_samples];
        filter.process_block(&noise, &desired, &mut out, &mut err);

        // Check that weights converged within 0.05 of target
        for (k, &tap) in target_taps.iter().enumerate() {
            let w = filter.weights()[k];
            assert!(
                (w - tap).abs() < 0.05,
                "LMS weight[{k}] = {w:.4}, expected {tap:.4}"
            );
        }
    }

    #[test]
    fn test_nlms_converges_faster() {
        // NLMS should converge to the same target in fewer samples than LMS.
        let target_taps = [0.5f32, -0.3, 0.1];
        let order = 3;
        let convergence_samples = 2000; // fewer than LMS's 5000

        let mut noise = vec![0.0f32; convergence_samples];
        let mut st = 7u32;
        for s in &mut noise {
            *s = next_rand(&mut st);
        }
        let desired = apply_fir(&noise);

        let mut nlms = NlmsFilter::new(order, 0.5);
        let mut out = vec![0.0f32; convergence_samples];
        let mut err = vec![0.0f32; convergence_samples];
        nlms.process_block(&noise, &desired, &mut out, &mut err);

        for (k, &tap) in target_taps.iter().enumerate() {
            let w = nlms.weights()[k];
            assert!(
                (w - tap).abs() < 0.05,
                "NLMS weight[{k}] = {w:.4}, expected {tap:.4} after {convergence_samples} samples"
            );
        }
    }

    #[test]
    fn test_lms_noise_cancellation() {
        // Noise cancellation setup:
        //   desired[n] = signal[n] + noise_path * noise_ref[n]
        //   LMS input = noise_ref
        //   LMS desired = desired
        //
        // After convergence, LMS learns to predict the noise component.
        // The error signal approaches the clean signal.
        // We verify this by checking that the filter weight converges to the
        // noise path gain (0.8 for a 1-tap path).
        let n = 8000;
        let noise_path_gain = 0.8f32;
        let mut state = 99u32;

        let mut signal = vec![0.0f32; n];
        let mut noise_ref = vec![0.0f32; n];
        for i in 0..n {
            signal[i] = (2.0 * std::f32::consts::PI * 0.05 * i as f32).sin();
            noise_ref[i] = next_rand(&mut state);
        }

        // desired = clean signal + noise passed through a known 1-tap path
        let desired: Vec<f32> = signal
            .iter()
            .zip(noise_ref.iter())
            .map(|(&s, &nr)| s + noise_path_gain * nr)
            .collect();

        // 1-tap filter to identify the noise path coefficient
        let mut filter = LmsFilter::new(1, 0.01);
        let mut out = vec![0.0f32; n];
        let mut err = vec![0.0f32; n];
        filter.process_block(&noise_ref, &desired, &mut out, &mut err);

        // The converged weight should match the noise path gain
        let learned_gain = filter.weights()[0];
        assert!(
            (learned_gain - noise_path_gain).abs() < 0.05,
            "LMS noise path weight converged to {learned_gain:.4}, expected {noise_path_gain:.4}"
        );

        // After convergence the error (cleaned output) should have much lower
        // noise power than the original desired signal: compare noise in late error
        // vs noise in late desired, measured as residual variance above the sine.
        let late_desired_noise: f32 = desired[n - 1000..]
            .iter()
            .zip(signal[n - 1000..].iter())
            .map(|(&d, &s)| (d - s).powi(2))
            .sum::<f32>();
        let late_error_noise: f32 = err[n - 1000..]
            .iter()
            .zip(signal[n - 1000..].iter())
            .map(|(&e, &s)| (e - s).powi(2))
            .sum::<f32>();

        let reduction_db = 10.0 * (late_desired_noise / late_error_noise.max(1e-12)).log10();
        assert!(
            reduction_db > 20.0,
            "Expected >20 dB noise reduction in error signal, got {reduction_db:.1} dB"
        );
    }

    #[test]
    fn test_nlms_regularization() {
        // Zero input must not produce NaN or inf even without regularization fudge.
        let mut filter = NlmsFilter::with_regularization(8, 0.5, 1e-6);
        let zeros = vec![0.0f32; 100];
        let desired = vec![0.5f32; 100];
        let mut out = vec![0.0f32; 100];
        let mut err = vec![0.0f32; 100];
        filter.process_block(&zeros, &desired, &mut out, &mut err);

        for (i, &y) in out.iter().enumerate() {
            assert!(y.is_finite(), "output[{i}] is not finite: {y}");
        }
        for (i, &e) in err.iter().enumerate() {
            assert!(e.is_finite(), "error[{i}] is not finite: {e}");
        }
        for (i, &w) in filter.weights().iter().enumerate() {
            assert!(w.is_finite(), "weight[{i}] is not finite: {w}");
        }
    }

    #[test]
    fn test_process_block_matches_sample() {
        let order = 4;
        let step = 0.02;
        let n = 200;
        let mut state = 333u32;

        let noise: Vec<f32> = (0..n).map(|_| next_rand(&mut state)).collect();
        let desired: Vec<f32> = (0..n).map(|i| (i as f32 * 0.1).sin()).collect();

        // Sample-by-sample
        let mut f1 = LmsFilter::new(order, step);
        let sample_out: Vec<(f32, f32)> = noise
            .iter()
            .zip(desired.iter())
            .map(|(&x, &d)| f1.process_sample(x, d))
            .collect();

        // Block
        let mut f2 = LmsFilter::new(order, step);
        let mut out = vec![0.0f32; n];
        let mut err = vec![0.0f32; n];
        f2.process_block(&noise, &desired, &mut out, &mut err);

        for i in 0..n {
            assert!(
                (sample_out[i].0 - out[i]).abs() < 1e-6,
                "output mismatch at {i}: sample={}, block={}",
                sample_out[i].0,
                out[i]
            );
            assert!(
                (sample_out[i].1 - err[i]).abs() < 1e-6,
                "error mismatch at {i}: sample={}, block={}",
                sample_out[i].1,
                err[i]
            );
        }
    }

    #[test]
    fn test_reset_clears_state() {
        let order = 4;
        let mut filter = LmsFilter::new(order, 0.05);
        let mut state = 55u32;

        // Run some samples to dirty the state
        for _ in 0..200 {
            let x = next_rand(&mut state);
            filter.process_sample(x, x * 0.5);
        }

        filter.reset();

        // All weights and buffer must be zero
        assert!(
            filter.weights().iter().all(|&w| w == 0.0),
            "weights not zeroed after reset"
        );
        // Feed a zero input — output must be zero (buffer is clear)
        let (out, err) = filter.process_sample(0.0, 0.0);
        assert_eq!(
            out, 0.0,
            "output should be 0 immediately after reset with zero input"
        );
        assert_eq!(
            err, 0.0,
            "error should be 0 immediately after reset with zero input"
        );
    }
}
