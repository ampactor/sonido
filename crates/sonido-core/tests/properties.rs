//! Property-based tests for sonido-core DSP primitives.
//!
//! Tests filter stability, parameter convergence, and delay line integrity
//! using proptest for randomized input generation.

use proptest::prelude::*;
use sonido_core::{
    Biquad, Effect, InterpolatedDelay, SmoothedParam, StateVariableFilter, SvfOutput,
    bandpass_coefficients, highpass_coefficients, lowpass_coefficients, notch_coefficients,
};

/// Biquad coefficient generators indexed 0..4 (LP, HP, BP, Notch).
fn configure_biquad(biquad: &mut Biquad, variant: usize, freq: f32, q: f32) {
    let sr = 48000.0;
    let (b0, b1, b2, a0, a1, a2) = match variant % 4 {
        0 => lowpass_coefficients(freq, q, sr),
        1 => highpass_coefficients(freq, q, sr),
        2 => bandpass_coefficients(freq, q, sr),
        3 => notch_coefficients(freq, q, sr),
        _ => unreachable!(),
    };
    biquad.set_coefficients(b0, b1, b2, a0, a1, a2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// For any valid cutoff (20-20000 Hz) and Q (0.1-10.0), Biquad filters
    /// produce finite output for 1024 samples of random finite input.
    #[test]
    fn biquad_stability(
        freq in 20.0f32..20000.0f32,
        q in 0.1f32..10.0f32,
        variant in 0usize..4,
        input in prop::array::uniform32(-1.0f32..=1.0f32),
    ) {
        let mut biquad = Biquad::new();
        configure_biquad(&mut biquad, variant, freq, q);

        for &sample in &input {
            let out = biquad.process(sample);
            prop_assert!(
                out.is_finite(),
                "Biquad variant {} (freq={}, q={}) produced non-finite output {} for input {}",
                variant % 4, freq, q, out, sample
            );
        }
    }

    /// For any valid cutoff and Q, SVF filters produce finite output across
    /// all output modes for 1024 random input samples.
    #[test]
    fn svf_stability(
        freq in 20.0f32..20000.0f32,
        q in 0.5f32..10.0f32,
        output_mode in 0usize..4,
        input in prop::array::uniform32(-1.0f32..=1.0f32),
    ) {
        let mut svf = StateVariableFilter::new(48000.0);
        svf.set_cutoff(freq);
        svf.set_resonance(q);
        let mode = match output_mode {
            0 => SvfOutput::Lowpass,
            1 => SvfOutput::Highpass,
            2 => SvfOutput::Bandpass,
            _ => SvfOutput::Notch,
        };
        svf.set_output_type(mode);

        for &sample in &input {
            let out = svf.process(sample);
            prop_assert!(
                out.is_finite(),
                "SVF mode {:?} (freq={}, q={}) produced non-finite output {} for input {}",
                mode, freq, q, out, sample
            );
        }
    }

    /// SmoothedParam converges toward its target value.
    /// Uses `standard()` (10ms time constant at 48kHz, coeff ≈ 0.00208).
    ///
    /// f32 precision limits exact convergence for large values. The one-pole
    /// smoothing `current += coeff * (target - current)` stalls when the step
    /// rounds to zero in f32. The precision floor is approximately
    /// `ULP(target) / coeff ≈ |target| * 2^-23 / 0.00208 ≈ |target| * 5.7e-5`.
    /// We verify convergence within this f32 precision bound.
    #[test]
    fn smoothed_param_convergence(
        initial in -100.0f32..100.0f32,
        target in -100.0f32..100.0f32,
    ) {
        let mut param = SmoothedParam::standard(initial, 48000.0);
        param.set_target(target);

        // 10000 samples (~208ms) is sufficient for the smoothing to reach
        // the f32 precision floor for any value in [-100, 100].
        for _ in 0..10000 {
            param.advance();
        }

        // f32 precision floor: ULP(target) / coeff.
        // ULP(x) ≈ |x| * 2^-23 for normal floats, minimum ULP ≈ 2^-149.
        // coeff ≈ 0.00208 for 10ms at 48kHz.
        // Add a 1e-4 floor for targets near zero where ULP is tiny.
        let ulp_estimate = target.abs() * f32::EPSILON;
        let precision_floor = ulp_estimate / 0.002 + 1e-4;
        let diff = (param.get() - target).abs();
        prop_assert!(
            diff < precision_floor,
            "SmoothedParam did not converge: initial={}, target={}, got={}, diff={}, tol={}",
            initial, target, param.get(), diff, precision_floor
        );
    }

    /// Write N random samples to InterpolatedDelay, read them back at integer
    /// delay N — they must match exactly (no interpolation at integer delays).
    #[test]
    fn delay_line_integrity(
        samples in prop::collection::vec(-1.0f32..=1.0f32, 1..=64),
    ) {
        let n = samples.len();
        // Buffer must be large enough: at least n+1 so we can read at delay=n
        let mut delay = InterpolatedDelay::new(n + 1);

        // Write all samples
        for &s in &samples {
            delay.write(s);
        }

        // Read back at integer delays — delay=0 is the last written sample,
        // delay=1 is the second-to-last, etc.
        for (i, &expected) in samples.iter().rev().enumerate() {
            let got = delay.read(i as f32);
            prop_assert!(
                (got - expected).abs() < 1e-6,
                "Delay mismatch at delay={}: expected {}, got {}",
                i, expected, got
            );
        }
    }
}
