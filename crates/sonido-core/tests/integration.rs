//! Integration tests for sonido-core DSP primitives.
//!
//! Tests cross-module interactions and verifies DSP accuracy using signal-level
//! measurements: sine wave analysis for filters, sample-accurate delay verification,
//! LFO waveform shape validation, SmoothedParam convergence timing, and oversampler
//! frequency response.

use sonido_core::{
    Biquad, Effect, FixedDelayLine, InterpolatedDelay, Lfo, LfoWaveform, Oversampled,
    SmoothedParam, StateVariableFilter, SvfOutput, highpass_coefficients, lowpass_coefficients,
};

const SAMPLE_RATE: f32 = 48000.0;
const TAU: f32 = core::f32::consts::TAU;

/// Generate a sine wave buffer at the given frequency and sample rate.
fn generate_sine(freq_hz: f32, sample_rate: f32, num_samples: usize) -> Vec<f32> {
    (0..num_samples)
        .map(|n| libm::sinf(TAU * freq_hz * n as f32 / sample_rate))
        .collect()
}

/// Measure RMS amplitude of a signal buffer.
fn rms(signal: &[f32]) -> f32 {
    let sum_sq: f32 = signal.iter().map(|&s| s * s).sum();
    libm::sqrtf(sum_sq / signal.len() as f32)
}

/// Convert linear amplitude to dB.
fn to_db(linear: f32) -> f32 {
    20.0 * libm::log10f(linear.max(1e-10))
}

// ============================================================================
// 1. Filter frequency responses
// ============================================================================

/// Feed a sine wave through a filter and measure the output amplitude relative
/// to a passband reference. Returns gain in dB.
fn measure_biquad_response(biquad: &mut Biquad, freq_hz: f32) -> f32 {
    let num_samples = 4800; // 100ms at 48kHz — enough to settle a 2nd-order filter
    let settle_samples = 2400;
    let input = generate_sine(freq_hz, SAMPLE_RATE, num_samples);
    let mut output = vec![0.0_f32; num_samples];
    biquad.clear();
    for (i, &s) in input.iter().enumerate() {
        output[i] = biquad.process(s);
    }
    // Measure RMS of the settled portion
    let input_rms = rms(&input[settle_samples..]);
    let output_rms = rms(&output[settle_samples..]);
    to_db(output_rms / input_rms)
}

#[test]
fn biquad_lowpass_frequency_response() {
    let cutoff = 1000.0;
    let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(cutoff, 0.707, SAMPLE_RATE);
    let mut biquad = Biquad::new();
    biquad.set_coefficients(b0, b1, b2, a0, a1, a2);

    // Frequencies well below cutoff should pass (~0 dB)
    for &freq in &[50.0, 100.0, 200.0, 500.0] {
        let gain_db = measure_biquad_response(&mut biquad, freq);
        assert!(
            gain_db.abs() < 1.0,
            "Lowpass passband: {freq} Hz should be ~0 dB, got {gain_db:.1} dB"
        );
    }

    // Frequencies well above cutoff should be attenuated
    for &freq in &[4000.0, 8000.0, 16000.0] {
        let gain_db = measure_biquad_response(&mut biquad, freq);
        assert!(
            gain_db < -6.0,
            "Lowpass stopband: {freq} Hz should be attenuated, got {gain_db:.1} dB"
        );
    }

    // At cutoff, Butterworth should be approximately -3 dB
    let gain_at_cutoff = measure_biquad_response(&mut biquad, cutoff);
    assert!(
        (gain_at_cutoff - (-3.0)).abs() < 1.5,
        "Lowpass at cutoff: expected ~-3 dB, got {gain_at_cutoff:.1} dB"
    );
}

#[test]
fn biquad_highpass_frequency_response() {
    let cutoff = 2000.0;
    let (b0, b1, b2, a0, a1, a2) = highpass_coefficients(cutoff, 0.707, SAMPLE_RATE);
    let mut biquad = Biquad::new();
    biquad.set_coefficients(b0, b1, b2, a0, a1, a2);

    // Frequencies well above cutoff should pass (~0 dB)
    for &freq in &[8000.0, 12000.0, 16000.0, 20000.0] {
        let gain_db = measure_biquad_response(&mut biquad, freq);
        assert!(
            gain_db.abs() < 1.0,
            "Highpass passband: {freq} Hz should be ~0 dB, got {gain_db:.1} dB"
        );
    }

    // Frequencies well below cutoff should be attenuated
    for &freq in &[100.0, 200.0, 500.0] {
        let gain_db = measure_biquad_response(&mut biquad, freq);
        assert!(
            gain_db < -6.0,
            "Highpass stopband: {freq} Hz should be attenuated, got {gain_db:.1} dB"
        );
    }
}

#[test]
fn svf_lowpass_frequency_response() {
    let cutoff = 1000.0;
    let mut svf = StateVariableFilter::new(SAMPLE_RATE);
    svf.set_cutoff(cutoff);
    svf.set_output_type(SvfOutput::Lowpass);

    // Below cutoff: pass
    let num_samples = 4800;
    let settle = 2400;
    for &freq in &[100.0, 200.0, 500.0] {
        svf.reset();
        let input = generate_sine(freq, SAMPLE_RATE, num_samples);
        let output: Vec<f32> = input.iter().map(|&s| svf.process(s)).collect();
        let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
        assert!(
            gain_db.abs() < 1.5,
            "SVF lowpass passband: {freq} Hz got {gain_db:.1} dB"
        );
    }

    // Above cutoff: attenuate
    for &freq in &[4000.0, 8000.0, 16000.0] {
        svf.reset();
        let input = generate_sine(freq, SAMPLE_RATE, num_samples);
        let output: Vec<f32> = input.iter().map(|&s| svf.process(s)).collect();
        let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
        assert!(
            gain_db < -6.0,
            "SVF lowpass stopband: {freq} Hz got {gain_db:.1} dB"
        );
    }
}

#[test]
fn svf_highpass_frequency_response() {
    let cutoff = 2000.0;
    let mut svf = StateVariableFilter::new(SAMPLE_RATE);
    svf.set_cutoff(cutoff);
    svf.set_output_type(SvfOutput::Highpass);

    let num_samples = 4800;
    let settle = 2400;

    // Above cutoff: pass
    for &freq in &[8000.0, 12000.0, 16000.0] {
        svf.reset();
        let input = generate_sine(freq, SAMPLE_RATE, num_samples);
        let output: Vec<f32> = input.iter().map(|&s| svf.process(s)).collect();
        let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
        assert!(
            gain_db.abs() < 1.5,
            "SVF highpass passband: {freq} Hz got {gain_db:.1} dB"
        );
    }

    // Below cutoff: attenuate
    for &freq in &[100.0, 200.0, 500.0] {
        svf.reset();
        let input = generate_sine(freq, SAMPLE_RATE, num_samples);
        let output: Vec<f32> = input.iter().map(|&s| svf.process(s)).collect();
        let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
        assert!(
            gain_db < -6.0,
            "SVF highpass stopband: {freq} Hz got {gain_db:.1} dB"
        );
    }
}

#[test]
fn biquad_and_svf_lowpass_agree_on_cutoff_slope() {
    // Both lowpass implementations at the same cutoff should agree
    // on the general shape: similar attenuation at 2x and 4x cutoff.
    let cutoff = 1000.0;

    let (b0, b1, b2, a0, a1, a2) = lowpass_coefficients(cutoff, 0.707, SAMPLE_RATE);
    let mut biquad = Biquad::new();
    biquad.set_coefficients(b0, b1, b2, a0, a1, a2);

    let mut svf = StateVariableFilter::new(SAMPLE_RATE);
    svf.set_cutoff(cutoff);
    svf.set_output_type(SvfOutput::Lowpass);

    // At 4x cutoff (4kHz), both should attenuate by roughly 12 dB (2nd order, -12dB/oct)
    let biquad_db = measure_biquad_response(&mut biquad, 4000.0);

    svf.reset();
    let num_samples = 4800;
    let settle = 2400;
    let input = generate_sine(4000.0, SAMPLE_RATE, num_samples);
    let output: Vec<f32> = input.iter().map(|&s| svf.process(s)).collect();
    let svf_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));

    // Both should agree within 3 dB (different topologies have slight differences)
    assert!(
        (biquad_db - svf_db).abs() < 3.0,
        "Biquad ({biquad_db:.1} dB) and SVF ({svf_db:.1} dB) at 4 kHz should agree"
    );
}

// ============================================================================
// 2. Delay line accuracy
// ============================================================================

#[test]
fn interpolated_delay_integer_accuracy() {
    let max_delay = 512;
    let mut delay = InterpolatedDelay::new(max_delay);

    // Write an impulse followed by zeros
    delay.write(1.0);
    for _ in 1..max_delay {
        delay.write(0.0);
    }

    // Read at various integer delays — should recover the impulse exactly
    for &d in &[1.0, 5.0, 10.0, 50.0, 100.0, 256.0, 500.0] {
        // Re-create delay with impulse at known position
        let mut dl = InterpolatedDelay::new(max_delay);
        dl.write(1.0);
        for _ in 1..(d as usize + 1) {
            dl.write(0.0);
        }
        let output = dl.read(d);
        assert!(
            (output - 1.0).abs() < 1e-6,
            "Integer delay {d}: expected 1.0, got {output}"
        );
    }
}

#[test]
fn interpolated_delay_fractional_interpolation() {
    let mut delay = InterpolatedDelay::new(64);

    // Write a linear ramp: 0, 1, 2, 3, ...
    for i in 0..10 {
        delay.write(i as f32);
    }

    // Read at fractional delay 1.5 — should interpolate between samples
    let output = delay.read(1.5);
    // Last written = 9 (at delay 0), second-to-last = 8 (at delay 1)
    // At delay 1.5: interpolate between sample at delay 1 (=8) and delay 2 (=7)
    let expected = 7.5;
    assert!(
        (output - expected).abs() < 0.1,
        "Fractional delay 1.5: expected ~{expected}, got {output}"
    );
}

#[test]
fn fixed_delay_line_integer_accuracy() {
    let mut delay: FixedDelayLine<512> = FixedDelayLine::new();

    // Write an impulse then zeros
    delay.write(1.0);
    for _ in 0..200 {
        delay.write(0.0);
    }

    // Read at integer delay should recover impulse
    let output = delay.read(200.0);
    assert!(
        (output - 1.0).abs() < 1e-6,
        "Fixed delay at 200 samples: expected 1.0, got {output}"
    );
}

#[test]
fn fixed_delay_line_circular_buffer_wrap() {
    let mut delay: FixedDelayLine<16> = FixedDelayLine::new();

    // Write more samples than buffer size to force wrap
    for i in 0..32 {
        delay.write(i as f32);
    }

    // The most recent sample (31) should be at delay 0
    let output = delay.read(0.0);
    assert!(
        (output - 31.0).abs() < 0.01,
        "After wrap, delay 0 should be 31.0, got {output}"
    );

    // Sample written 5 ago (26) should be at delay 5
    let output = delay.read(5.0);
    assert!(
        (output - 26.0).abs() < 0.01,
        "After wrap, delay 5 should be 26.0, got {output}"
    );
}

#[test]
fn delay_read_write_combined() {
    let mut delay = InterpolatedDelay::new(128);

    // Pump a known sequence through, verify output is delayed version.
    // read_write reads BEFORE writing, so at call i:
    //   - read returns the sample written (delay_samples + 1) calls ago
    //   - then writes the current sample
    // This means output[i] = input[i - delay_samples - 1] for integer delays.
    let sequence: Vec<f32> = (0..100).map(|i| (i as f32) * 0.01).collect();
    let delay_samples = 10.0;
    let total_delay = delay_samples as usize + 1;
    let mut outputs = Vec::new();

    for &s in &sequence {
        let out = delay.read_write(s, delay_samples);
        outputs.push(out);
    }

    // After the delay line fills, verify the delayed relationship
    for i in (total_delay + 5)..100 {
        let expected = sequence[i - total_delay];
        assert!(
            (outputs[i] - expected).abs() < 0.01,
            "read_write at sample {i}: expected {expected:.3}, got {:.3}",
            outputs[i]
        );
    }
}

// ============================================================================
// 3. LFO shape verification
// ============================================================================

/// Run one full cycle of an LFO and collect all samples.
fn run_lfo_one_cycle(waveform: LfoWaveform, freq_hz: f32) -> Vec<f32> {
    let mut lfo = Lfo::new(SAMPLE_RATE, freq_hz);
    lfo.set_waveform(waveform);
    let samples_per_cycle = (SAMPLE_RATE / freq_hz) as usize;
    (0..samples_per_cycle).map(|_| lfo.advance()).collect()
}

#[test]
fn lfo_sine_peaks_at_plus_minus_one() {
    let cycle = run_lfo_one_cycle(LfoWaveform::Sine, 1.0);

    let max_val = cycle.iter().cloned().fold(f32::MIN, f32::max);
    let min_val = cycle.iter().cloned().fold(f32::MAX, f32::min);

    assert!(
        (max_val - 1.0).abs() < 0.01,
        "Sine max should be ~1.0, got {max_val}"
    );
    assert!(
        (min_val - (-1.0)).abs() < 0.01,
        "Sine min should be ~-1.0, got {min_val}"
    );

    // Zero crossings: first sample should be near 0
    assert!(
        cycle[0].abs() < 0.01,
        "Sine should start near 0, got {}",
        cycle[0]
    );
}

#[test]
fn lfo_triangle_is_symmetric_and_linear() {
    let cycle = run_lfo_one_cycle(LfoWaveform::Triangle, 1.0);
    let n = cycle.len();

    // Triangle waveform: phase 0..0.5 maps to -1..+1 (rising), 0.5..1.0 maps to +1..-1 (falling).
    // Peak at +1.0 occurs at phase = 0.5 (midpoint of cycle).
    // Trough at -1.0 occurs at phase = 0 (start) and phase = 1.0 (end).
    let half = n / 2;

    assert!(
        (cycle[0] - (-1.0)).abs() < 0.01,
        "Triangle should start at -1.0, got {}",
        cycle[0]
    );
    assert!(
        (cycle[half] - 1.0).abs() < 0.01,
        "Triangle peak at midpoint: expected ~1.0, got {}",
        cycle[half]
    );

    // Verify linearity in the rising portion (first half)
    // Consecutive differences should be approximately constant
    let rising = &cycle[1..half];
    let diffs: Vec<f32> = rising.windows(2).map(|w| w[1] - w[0]).collect();
    let mean_diff = diffs.iter().sum::<f32>() / diffs.len() as f32;
    for (i, &d) in diffs.iter().enumerate() {
        assert!(
            (d - mean_diff).abs() < 0.01,
            "Triangle rising slope not constant at sample {i}: diff={d:.4}, mean={mean_diff:.4}"
        );
    }

    // Verify symmetry: rising and falling halves should be mirrors
    for i in 1..n / 4 {
        let rising_val = cycle[i];
        let falling_val = cycle[n - i];
        assert!(
            (rising_val - falling_val).abs() < 0.01,
            "Triangle not symmetric at offset {i}: rising={rising_val:.4}, falling={falling_val:.4}"
        );
    }
}

#[test]
fn lfo_square_clean_transitions() {
    // Use 10 Hz to reduce f32 phase accumulation drift (4800 samples/cycle).
    let cycle = run_lfo_one_cycle(LfoWaveform::Square, 10.0);
    let n = cycle.len();

    // Square waveform: phase < 0.5 => +1.0, phase >= 0.5 => -1.0.
    // Every sample must be exactly +1.0 or -1.0 (no intermediate values).
    for (i, &v) in cycle.iter().enumerate() {
        assert!(
            (v - 1.0).abs() < 0.001 || (v - (-1.0)).abs() < 0.001,
            "Square at sample {i}: expected ±1.0, got {v}"
        );
    }

    // First sample (phase=0) should be +1.0
    assert!(
        (cycle[0] - 1.0).abs() < 0.001,
        "Square should start at +1.0, got {}",
        cycle[0]
    );

    // Count the transition: there should be exactly one falling edge
    // (from +1 to -1) in one cycle.
    let mut transitions = 0;
    let mut transition_idx = 0;
    for i in 1..n {
        if (cycle[i] - cycle[i - 1]).abs() > 1.0 {
            transitions += 1;
            transition_idx = i;
        }
    }
    assert!(
        transitions == 1,
        "Square should have exactly 1 transition in one cycle, got {transitions}"
    );

    // Transition should occur near the midpoint (phase = 0.5)
    let half = n / 2;
    let drift_tolerance = n / 50; // 2% tolerance for f32 drift
    assert!(
        (transition_idx as i32 - half as i32).unsigned_abs() as usize <= drift_tolerance,
        "Square transition at sample {transition_idx}, expected ~{half} (±{drift_tolerance})"
    );
}

#[test]
fn lfo_saw_ramps_linearly() {
    // Use a higher frequency to reduce f32 phase accumulation drift.
    // At 10 Hz / 48 kHz, one cycle = 4800 samples.
    let cycle = run_lfo_one_cycle(LfoWaveform::Saw, 10.0);

    // Saw formula: 2*phase - 1, where phase goes from 0 to ~1 over one cycle.
    // First sample (phase=0): 2*0 - 1 = -1.0
    assert!(
        (cycle[0] - (-1.0)).abs() < 0.01,
        "Saw should start at -1.0, got {}",
        cycle[0]
    );

    // Max should approach +1.0, min should be -1.0
    let max_val = cycle.iter().cloned().fold(f32::MIN, f32::max);
    let min_val = cycle.iter().cloned().fold(f32::MAX, f32::min);
    assert!(
        max_val > 0.99,
        "Saw max should approach +1.0, got {max_val}"
    );
    assert!(
        (min_val - (-1.0)).abs() < 0.01,
        "Saw min should be -1.0, got {min_val}"
    );

    // Verify monotonically increasing for the bulk of the cycle.
    // Due to f32 phase accumulation, the wrap may happen slightly early,
    // causing one falling transition. Check that at least 99% are rising.
    let mut rising_count = 0;
    for w in cycle.windows(2) {
        if w[1] >= w[0] - 0.001 {
            rising_count += 1;
        }
    }
    let pct_rising = rising_count as f32 / (cycle.len() - 1) as f32;
    assert!(
        pct_rising > 0.99,
        "Saw should be >99% monotonically increasing, got {:.1}%",
        pct_rising * 100.0
    );
}

#[test]
fn lfo_all_waveforms_stay_in_range() {
    // Run several cycles of each waveform at various frequencies
    for &freq in &[0.5, 1.0, 5.0, 10.0, 20.0] {
        for waveform in [
            LfoWaveform::Sine,
            LfoWaveform::Triangle,
            LfoWaveform::Saw,
            LfoWaveform::Square,
        ] {
            let mut lfo = Lfo::new(SAMPLE_RATE, freq);
            lfo.set_waveform(waveform);

            for _ in 0..10000 {
                let val = lfo.advance();
                assert!(
                    (-1.001..=1.001).contains(&val),
                    "{waveform:?} at {freq} Hz out of range: {val}"
                );
            }
        }
    }
}

// ============================================================================
// 4. SmoothedParam convergence timing
// ============================================================================

/// Count samples until the param reaches within `threshold` of its target.
fn count_convergence_samples(param: &mut SmoothedParam, target: f32, threshold: f32) -> usize {
    param.set_target(target);
    let mut count = 0;
    for _ in 0..100_000 {
        param.advance();
        count += 1;
        if (param.get() - target).abs() < threshold {
            return count;
        }
    }
    count
}

#[test]
fn smoothed_param_fast_convergence() {
    // fast = 5ms. One-pole reaches ~99.3% at 5*tau = 25ms.
    // At 48kHz, 25ms = 1200 samples.
    let mut param = SmoothedParam::fast(0.0, SAMPLE_RATE);
    let samples = count_convergence_samples(&mut param, 1.0, 0.01);
    let expected = (SAMPLE_RATE * 0.025) as usize; // 5 * 5ms = 25ms
    let tolerance = (expected as f32 * 0.2) as usize; // 20% tolerance
    assert!(
        samples <= expected + tolerance,
        "fast: converged in {samples} samples, expected ~{expected} (±{tolerance})"
    );
    assert!(
        samples >= expected / 3,
        "fast: converged too quickly in {samples} samples, expected ~{expected}"
    );
}

#[test]
fn smoothed_param_standard_convergence() {
    // standard = 10ms. 5*tau = 50ms = 2400 samples at 48kHz.
    let mut param = SmoothedParam::standard(0.0, SAMPLE_RATE);
    let samples = count_convergence_samples(&mut param, 1.0, 0.01);
    let expected = (SAMPLE_RATE * 0.050) as usize;
    let tolerance = (expected as f32 * 0.2) as usize;
    assert!(
        samples <= expected + tolerance,
        "standard: converged in {samples} samples, expected ~{expected} (±{tolerance})"
    );
    assert!(
        samples >= expected / 3,
        "standard: converged too quickly in {samples} samples, expected ~{expected}"
    );
}

#[test]
fn smoothed_param_slow_convergence() {
    // slow = 20ms. 5*tau = 100ms = 4800 samples at 48kHz.
    let mut param = SmoothedParam::slow(0.0, SAMPLE_RATE);
    let samples = count_convergence_samples(&mut param, 1.0, 0.01);
    let expected = (SAMPLE_RATE * 0.100) as usize;
    let tolerance = (expected as f32 * 0.2) as usize;
    assert!(
        samples <= expected + tolerance,
        "slow: converged in {samples} samples, expected ~{expected} (±{tolerance})"
    );
    assert!(
        samples >= expected / 3,
        "slow: converged too quickly in {samples} samples, expected ~{expected}"
    );
}

#[test]
fn smoothed_param_interpolated_convergence() {
    // interpolated = 50ms. 5*tau = 250ms = 12000 samples at 48kHz.
    let mut param = SmoothedParam::interpolated(0.0, SAMPLE_RATE);
    let samples = count_convergence_samples(&mut param, 1.0, 0.01);
    let expected = (SAMPLE_RATE * 0.250) as usize;
    let tolerance = (expected as f32 * 0.2) as usize;
    assert!(
        samples <= expected + tolerance,
        "interpolated: converged in {samples} samples, expected ~{expected} (±{tolerance})"
    );
    assert!(
        samples >= expected / 3,
        "interpolated: converged too quickly in {samples} samples, expected ~{expected}"
    );
}

#[test]
fn smoothed_param_one_time_constant_reaches_63_percent() {
    // After exactly one time constant (tau = smoothing_time), the one-pole
    // filter reaches ~63.2% (1 - e^-1) of the target.
    let tau_ms = 10.0;
    let mut param = SmoothedParam::with_config(0.0, SAMPLE_RATE, tau_ms);
    param.set_target(1.0);

    let tau_samples = (SAMPLE_RATE * tau_ms / 1000.0) as usize;
    for _ in 0..tau_samples {
        param.advance();
    }

    let expected = 1.0 - libm::expf(-1.0); // ~0.6321
    assert!(
        (param.get() - expected).abs() < 0.05,
        "After one time constant, expected ~{expected:.3}, got {:.3}",
        param.get()
    );
}

// ============================================================================
// 5. Oversampling frequency response
// ============================================================================

/// Passthrough effect for testing the oversampler in isolation.
struct Passthrough;

impl Effect for Passthrough {
    fn process(&mut self, input: f32) -> f32 {
        input
    }
    fn set_sample_rate(&mut self, _: f32) {}
    fn reset(&mut self) {}
}

#[test]
fn oversampler_passband_flat() {
    // With passthrough, the oversampler should have ~0 dB gain in the passband.
    let mut os = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);

    for &freq in &[100.0, 500.0, 1000.0, 2000.0, 5000.0] {
        os.reset();
        let num_samples = 9600; // 200ms
        let settle = 4800;
        let input = generate_sine(freq, SAMPLE_RATE, num_samples);
        let mut output = vec![0.0_f32; num_samples];
        for (i, &s) in input.iter().enumerate() {
            output[i] = os.process(s);
        }
        let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
        assert!(
            gain_db.abs() < 1.0,
            "Oversampler 4x passband at {freq} Hz: {gain_db:.1} dB (expected ~0 dB)"
        );
    }
}

#[test]
fn oversampler_dc_unity_all_factors() {
    // Verify DC passthrough for all oversampling factors.
    for factor_label in ["2x", "4x", "8x"] {
        let mut output = 0.0_f32;
        match factor_label {
            "2x" => {
                let mut os = Oversampled::<2, _>::new(Passthrough, SAMPLE_RATE);
                for _ in 0..1000 {
                    output = os.process(1.0);
                }
            }
            "4x" => {
                let mut os = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);
                for _ in 0..1000 {
                    output = os.process(1.0);
                }
            }
            "8x" => {
                let mut os = Oversampled::<8, _>::new(Passthrough, SAMPLE_RATE);
                for _ in 0..1000 {
                    output = os.process(1.0);
                }
            }
            _ => unreachable!(),
        }
        assert!(
            (output - 1.0).abs() < 0.01,
            "Oversampler {factor_label} DC: expected 1.0, got {output}"
        );
    }
}

#[test]
fn oversampler_attenuates_near_nyquist() {
    // A sine near Nyquist/2 (12 kHz at 48 kHz) should be attenuated by the
    // anti-aliasing filter, especially for higher oversampling factors where
    // the cutoff is lower in the oversampled domain.
    let mut os = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);
    let freq = 20000.0; // Near Nyquist
    let num_samples = 9600;
    let settle = 4800;
    let input = generate_sine(freq, SAMPLE_RATE, num_samples);
    let mut output = vec![0.0_f32; num_samples];
    for (i, &s) in input.iter().enumerate() {
        output[i] = os.process(s);
    }
    let gain_db = to_db(rms(&output[settle..]) / rms(&input[settle..]));
    // The anti-aliasing filter should attenuate near-Nyquist content
    assert!(
        gain_db < -3.0,
        "Oversampler at 20 kHz should attenuate, got {gain_db:.1} dB"
    );
}

#[test]
fn oversampler_block_processing_matches_sample() {
    // Block processing should produce identical results to sample-by-sample.
    let mut os_sample = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);
    let mut os_block = Oversampled::<4, _>::new(Passthrough, SAMPLE_RATE);

    let input: Vec<f32> = (0..256)
        .map(|n| libm::sinf(TAU * 1000.0 * n as f32 / SAMPLE_RATE))
        .collect();

    let sample_output: Vec<f32> = input.iter().map(|&s| os_sample.process(s)).collect();
    let mut block_output = vec![0.0_f32; 256];
    os_block.process_block(&input, &mut block_output);

    for (i, (&s, &b)) in sample_output.iter().zip(block_output.iter()).enumerate() {
        assert!(
            (s - b).abs() < 1e-6,
            "Sample vs block mismatch at {i}: {s} != {b}"
        );
    }
}

// ============================================================================
// 6. Cross-module integration
// ============================================================================

#[test]
fn lfo_modulates_filter_cutoff_smoothly() {
    // Verify that an LFO-modulated SVF produces continuous (non-clicking) output.
    let mut svf = StateVariableFilter::new(SAMPLE_RATE);
    svf.set_output_type(SvfOutput::Lowpass);
    let mut lfo = Lfo::new(SAMPLE_RATE, 2.0); // 2 Hz modulation

    let num_samples = 48000; // 1 second
    let mut prev_output = 0.0_f32;
    let mut max_jump = 0.0_f32;

    // Feed white-ish noise (use a simple deterministic sequence)
    for n in 0..num_samples {
        // Modulate cutoff between 200 Hz and 5000 Hz
        let mod_val = lfo.advance_unipolar(); // 0..1
        let cutoff = 200.0 + mod_val * 4800.0;
        svf.set_cutoff(cutoff);

        // Simple deterministic "noise" (triangle at odd frequency)
        let input = libm::sinf(TAU * 1337.0 * n as f32 / SAMPLE_RATE);
        let output = svf.process(input);

        let jump = (output - prev_output).abs();
        if n > 100 && jump > max_jump {
            max_jump = jump;
        }
        prev_output = output;
    }

    // Max sample-to-sample jump should be bounded (no clicks/pops)
    assert!(
        max_jump < 0.5,
        "LFO-modulated filter produced jumps of {max_jump:.3} (potential click)"
    );
}

#[test]
fn smoothed_param_applied_to_filter() {
    // SmoothedParam should prevent clicks when changing filter cutoff.
    let mut svf = StateVariableFilter::new(SAMPLE_RATE);
    svf.set_cutoff(500.0);
    svf.set_output_type(SvfOutput::Lowpass);
    let mut cutoff_param = SmoothedParam::standard(500.0, SAMPLE_RATE);

    // Abruptly change target cutoff
    cutoff_param.set_target(5000.0);

    let mut max_jump = 0.0_f32;
    let mut prev = 0.0_f32;

    for n in 0..4800 {
        let cutoff = cutoff_param.advance();
        svf.set_cutoff(cutoff);
        let input = libm::sinf(TAU * 440.0 * n as f32 / SAMPLE_RATE);
        let output = svf.process(input);

        if n > 10 {
            let jump = (output - prev).abs();
            if jump > max_jump {
                max_jump = jump;
            }
        }
        prev = output;
    }

    // Smoothed transitions should not produce large jumps
    assert!(
        max_jump < 0.3,
        "Smoothed filter cutoff change produced jump of {max_jump:.3}"
    );
}

#[test]
fn effect_chain_oversampled_preserves_dc() {
    // Chaining an oversampled passthrough should still pass DC.
    use sonido_core::EffectExt;

    /// Simple gain effect for chain testing.
    struct TestGain(f32);
    impl Effect for TestGain {
        fn process(&mut self, input: f32) -> f32 {
            input * self.0
        }
        fn set_sample_rate(&mut self, _: f32) {}
        fn reset(&mut self) {}
    }

    let os = Oversampled::<2, _>::new(Passthrough, SAMPLE_RATE);
    let mut chain = os.chain(TestGain(0.5));

    // Let it settle
    for _ in 0..500 {
        chain.process(1.0);
    }
    let output = chain.process(1.0);
    assert!(
        (output - 0.5).abs() < 0.02,
        "Oversampled chain should output ~0.5, got {output}"
    );
}
