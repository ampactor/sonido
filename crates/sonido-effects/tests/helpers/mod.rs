//! Shared test helpers for sonido-effects integration tests.
//!
//! Each integration test compiles as a separate binary, so not every helper
//! is used in every binary. The `allow(dead_code)` on shared functions is
//! intentional — it communicates "this is shared infrastructure, not orphaned code."

use sonido_registry::EffectRegistry;

const SAMPLE_RATE: f32 = 48000.0;

/// All effect IDs currently registered.
#[allow(dead_code)]
pub fn all_ids() -> Vec<String> {
    EffectRegistry::new()
        .all_effects()
        .into_iter()
        .map(|d| d.id.to_string())
        .collect()
}

/// All effect IDs from an existing registry (avoids constructing a second one).
#[allow(dead_code)]
pub fn all_ids_from(registry: &EffectRegistry) -> Vec<String> {
    registry
        .all_effects()
        .into_iter()
        .map(|d| d.id.to_string())
        .collect()
}

/// Generate a 440 Hz sine wave at 0.5 amplitude.
#[allow(dead_code)]
pub fn sine_440(len: usize) -> Vec<f32> {
    (0..len)
        .map(|i| {
            let t = i as f32 / SAMPLE_RATE;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect()
}

/// RMS level of a signal in dB.
#[allow(dead_code)]
pub fn rms_db(signal: &[f32]) -> f32 {
    let sum_sq: f32 = signal.iter().map(|&s| s * s).sum();
    let rms = (sum_sq / signal.len() as f32).sqrt();
    if rms > 1e-10 {
        20.0 * rms.log10()
    } else {
        -200.0
    }
}

/// Collect violations and panic with a formatted summary if any exist.
#[allow(dead_code)]
pub fn assert_no_violations(test_name: &str, context: &str, violations: Vec<String>) {
    assert!(
        violations.is_empty(),
        "{test_name}: {} violation(s) — {context}:\n{}",
        violations.len(),
        violations.join("\n")
    );
}
