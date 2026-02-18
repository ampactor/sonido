//! Property-based tests for all registered effects.
//!
//! Uses proptest to verify that every effect in the registry satisfies
//! fundamental invariants: finite output, bounded output, and clean reset.

use proptest::prelude::*;
use sonido_registry::{EffectRegistry, EffectWithParams};

/// All effect IDs in the registry.
fn all_effect_ids() -> Vec<&'static str> {
    let registry = EffectRegistry::new();
    registry.all_effects().into_iter().map(|d| d.id).collect()
}

/// Set random valid parameters on an effect using normalized [0,1] values.
fn set_random_params(effect: &mut Box<dyn EffectWithParams + Send>, rng_values: &[f32; 16]) {
    let count = effect.effect_param_count();
    for i in 0..count {
        if let Some(desc) = effect.effect_param_info(i) {
            let t = rng_values[i % 16];
            let value = desc.min + t * (desc.max - desc.min);
            effect.effect_set_param(i, value);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// For any finite input in [-1, 1] and valid parameter values,
    /// every registered effect must produce finite (non-NaN, non-Inf) output.
    #[test]
    fn all_effects_finite_output(
        input in prop::array::uniform32(-1.0f32..=1.0f32),
        param_values in prop::array::uniform16(0.0f32..=1.0f32),
        effect_idx in 0usize..15,
    ) {
        let ids = all_effect_ids();
        let id = ids[effect_idx % ids.len()];
        let registry = EffectRegistry::new();
        let mut effect = registry.create(id, 48000.0).unwrap();

        set_random_params(&mut effect, &param_values);

        // Warm up so internal state settles
        for _ in 0..64 {
            effect.process(0.0);
        }

        for &sample in &input {
            let out = effect.process(sample);
            prop_assert!(
                out.is_finite(),
                "Effect '{}' produced non-finite mono output {} for input {}",
                id, out, sample
            );

            let (l, r) = effect.process_stereo(sample, sample);
            prop_assert!(
                l.is_finite() && r.is_finite(),
                "Effect '{}' produced non-finite stereo output ({}, {}) for input {}",
                id, l, r, sample
            );
        }
    }

    /// For input in [-1, 1], output should stay within [-10, 10].
    /// Effects with gain stages can exceed unity but shouldn't blow up.
    #[test]
    fn all_effects_bounded_output(
        input in prop::array::uniform32(-1.0f32..=1.0f32),
        param_values in prop::array::uniform16(0.0f32..=1.0f32),
        effect_idx in 0usize..15,
    ) {
        let ids = all_effect_ids();
        let id = ids[effect_idx % ids.len()];
        let registry = EffectRegistry::new();
        let mut effect = registry.create(id, 48000.0).unwrap();

        set_random_params(&mut effect, &param_values);

        // Process enough samples for state to settle
        for _ in 0..256 {
            effect.process(0.0);
        }

        let bound = 10.0;
        for &sample in &input {
            let out = effect.process(sample);
            prop_assert!(
                out.abs() <= bound,
                "Effect '{}' output {} exceeds bound +/-{} for input {}",
                id, out, bound, sample
            );
        }
    }

    /// After reset(), the effect's internal state should be cleared.
    /// We verify this by comparing: the output of a reset effect processing
    /// silence should match a freshly created effect (with the same params)
    /// processing silence. This accounts for effects with DC bias parameters.
    #[test]
    fn all_effects_reset_clears_state(
        input in prop::array::uniform32(-1.0f32..=1.0f32),
        param_values in prop::array::uniform16(0.0f32..=1.0f32),
        effect_idx in 0usize..15,
    ) {
        let ids = all_effect_ids();
        let id = ids[effect_idx % ids.len()];
        let registry = EffectRegistry::new();
        let mut effect = registry.create(id, 48000.0).unwrap();

        set_random_params(&mut effect, &param_values);

        // Feed random input to build up internal state
        for &sample in &input {
            effect.process(sample);
        }

        // Reset the effect
        effect.reset();

        // Create a fresh reference effect with identical params
        let mut fresh = registry.create(id, 48000.0).unwrap();
        set_random_params(&mut fresh, &param_values);

        // Both should produce the same output on silence.
        // Allow 4800 samples (100ms) for smoothed params to converge.
        let mut reset_out = 0.0f32;
        let mut fresh_out = 0.0f32;
        for _ in 0..4800 {
            reset_out = effect.process(0.0);
            fresh_out = fresh.process(0.0);
        }

        // The reset effect should match the fresh effect within tolerance.
        // SmoothedParam convergence at f32 precision limits exactness.
        // Hysteresis feedback loops (tape saturation) cause path-dependent
        // convergence: snapped params (reset) vs smoothed params (fresh)
        // reach slightly different DC operating points through nonlinear feedback.
        let diff = (reset_out - fresh_out).abs();
        prop_assert!(
            diff < 0.02,
            "Effect '{}': reset output {} differs from fresh output {} (diff={})",
            id, reset_out, fresh_out, diff
        );
    }
}
