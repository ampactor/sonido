//! Host-side verification of the Daisy noon preset table and biased knob mapping.
//!
//! The noon presets and `adc_to_param_biased` live in `sonido-daisy` (a `no_std`
//! Cortex-M7 crate) where unit tests can't run. Both are pure math depending only
//! on `ParamDescriptor`, so we inline them here with `std` math substitutions
//! (`f32::log2` instead of `libm::log2f`, etc.) and run exhaustive checks against
//! every registered effect.

mod helpers;

use helpers::{all_ids_from, assert_no_violations};
use sonido_core::{ParamFlags, ParamScale};
use sonido_registry::EffectRegistry;

const SAMPLE_RATE: f32 = 48000.0;

// ─────────────────────────────────────────────────────────────────────────────
// NOTE: This file does NOT use `all_ids()` — it uses `all_ids_from(&registry)`
// to avoid constructing a second EffectRegistry per test.
// ─────────────────────────────────────────────────────────────────────────────

// ─────────────────────────────────────────────────────────────────────────────
// Inlined from crates/sonido-daisy/src/param_map.rs — keep in sync
// Uses std math (f32::log2, f32::exp2, f32::powf) instead of libm.
// ─────────────────────────────────────────────────────────────────────────────

fn interpolate_scaled(lo: f32, hi: f32, t: f32, scale: ParamScale) -> f32 {
    match scale {
        ParamScale::Linear => lo + t * (hi - lo),
        ParamScale::Logarithmic => {
            let log_lo = if lo > 1e-6 { lo } else { 1e-6 }.log2();
            let log_hi = if hi > 1e-6 { hi } else { 1e-6 }.log2();
            (log_lo + t * (log_hi - log_lo)).exp2()
        }
        ParamScale::Power(exp) => lo + t.powf(exp) * (hi - lo),
        _ => lo + t * (hi - lo),
    }
}

fn adc_to_param(desc: &sonido_core::ParamDescriptor, normalized: f32) -> f32 {
    let val = match desc.scale {
        ParamScale::Linear => desc.min + normalized * (desc.max - desc.min),
        ParamScale::Logarithmic => {
            let log_min = if desc.min > 1e-6 { desc.min } else { 1e-6 }.log2();
            let log_max = if desc.max > 1e-6 { desc.max } else { 1e-6 }.log2();
            (log_min + normalized * (log_max - log_min)).exp2()
        }
        ParamScale::Power(exp) => desc.min + normalized.powf(exp) * (desc.max - desc.min),
        _ => desc.min + normalized * (desc.max - desc.min),
    };
    if desc.flags.contains(ParamFlags::STEPPED) {
        val.round()
    } else {
        val
    }
}

fn adc_to_param_biased(desc: &sonido_core::ParamDescriptor, noon: f32, normalized: f32) -> f32 {
    if desc.flags.contains(ParamFlags::STEPPED) {
        return adc_to_param(desc, normalized);
    }

    let noon = noon.clamp(desc.min, desc.max);
    let range = desc.max - desc.min;

    if range < 1e-6 || (noon - desc.min) / range < 0.05 || (desc.max - noon) / range < 0.05 {
        return adc_to_param(desc, normalized);
    }

    if normalized <= 0.5 {
        let t = normalized * 2.0;
        interpolate_scaled(desc.min, noon, t, desc.scale)
    } else {
        let t = (normalized - 0.5) * 2.0;
        interpolate_scaled(noon, desc.max, t, desc.scale)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Inlined from crates/sonido-daisy/src/noon_presets.rs — keep in sync
// ─────────────────────────────────────────────────────────────────────────────

fn noon_value(effect_id: &str, param_idx: usize) -> Option<f32> {
    let values: &[f32] = match effect_id {
        "distortion" => &[8.0, 0.0, 0.0, 0.0, 50.0, 0.0],
        "preamp" => &[0.0, 0.0, 0.0],
        "compressor" => &[-18.0, 4.0, 10.0, 100.0, 0.0, 6.0, 0.0, 80.0, 0.0, 0.0, 50.0],
        "gate" => &[-40.0, 1.0, 100.0, 50.0, -80.0, 3.0, 80.0, 0.0],
        "eq" => &[100.0, 0.0, 1.0, 500.0, 0.0, 1.0, 5000.0, 0.0, 1.0, 0.0],
        "wah" => &[800.0, 5.0, 50.0, 0.0, 0.0],
        "chorus" => &[1.0, 50.0, 50.0, 2.0, 0.0, 15.0, 0.0, 3.0, 0.0],
        "flanger" => &[0.5, 35.0, 50.0, 50.0, 0.0, 0.0, 3.0, 0.0],
        "phaser" => &[0.3, 50.0, 6.0, 50.0, 50.0, 20.0, 4000.0, 0.0, 3.0, 0.0],
        "tremolo" => &[5.0, 50.0, 0.0, 0.0, 0.0, 3.0, 0.0],
        "delay" => &[300.0, 40.0, 50.0, 0.0, 20000.0, 20.0, 0.0, 0.0, 2.0, 0.0],
        "filter" => &[1000.0, 0.707, 0.0, 0.0],
        "vibrato" => &[100.0, 50.0, 0.0],
        "tape" => &[6.0, 30.0, 12000.0, 0.0, 0.3, 0.2, 0.15, 0.3, 80.0, -6.0],
        "reverb" => &[50.0, 50.0, 30.0, 10.0, 50.0, 100.0, 50.0, 0.0],
        "limiter" => &[-6.0, -0.3, 100.0, 5.0, 0.0],
        "bitcrusher" => &[8.0, 1.0, 0.0, 50.0, 0.0],
        "ringmod" => &[440.0, 50.0, 0.0, 50.0, 0.0],
        "stage" => &[
            0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 120.0, 0.0, 0.0, 0.0,
        ],
        "looper" => &[0.0, 80.0, 0.0, 0.0, 50.0, 0.0],
        _ => return None,
    };
    values.get(param_idx).copied()
}

// ─────────────────────────────────────────────────────────────────────────────
// Test A: Noon Coverage Completeness
// ─────────────────────────────────────────────────────────────────────────────

/// Every writable (non-READ_ONLY) parameter must have a noon preset.
/// READ_ONLY diagnostic params must NOT have one.
#[test]
fn noon_coverage_completeness() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();

    for id in all_ids_from(&registry) {
        let effect = registry.create(&id, SAMPLE_RATE).unwrap();
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx) {
                let is_read_only = desc.flags.contains(ParamFlags::READ_ONLY);
                let has_noon = noon_value(&id, idx).is_some();

                if !is_read_only && !has_noon {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": writable param missing noon preset",
                        desc.name
                    ));
                }
                if is_read_only && has_noon {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": READ_ONLY param has unnecessary noon preset",
                        desc.name
                    ));
                }
            }
        }
    }

    assert_no_violations(
        "noon_coverage_completeness",
        "every writable param needs a noon preset; READ_ONLY params must not have one",
        violations,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test B: Noon Values In Range
// ─────────────────────────────────────────────────────────────────────────────

/// Every noon value must be within the descriptor's [min, max] range.
/// Catches stale noon values after descriptor range changes.
#[test]
fn noon_values_in_range() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();

    for id in all_ids_from(&registry) {
        let effect = registry.create(&id, SAMPLE_RATE).unwrap();
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx)
                && let Some(noon) = noon_value(&id, idx)
                && (noon < desc.min || noon > desc.max)
            {
                violations.push(format!(
                    "  {id}[{idx}] \"{}\": noon={noon} outside [{}, {}]",
                    desc.name, desc.min, desc.max
                ));
            }
        }
    }

    assert_no_violations(
        "noon_values_in_range",
        "noon values must be within descriptor [min, max]",
        violations,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test C: Biased Mapping Endpoints
// ─────────────────────────────────────────────────────────────────────────────

/// Knob at 0.0 must produce min, knob at 1.0 must produce max.
/// No dead zones from biased split.
#[test]
fn biased_mapping_endpoints() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();

    for id in all_ids_from(&registry) {
        let effect = registry.create(&id, SAMPLE_RATE).unwrap();
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx) {
                if desc.flags.contains(ParamFlags::READ_ONLY) {
                    continue;
                }
                let noon = noon_value(&id, idx).unwrap_or(desc.default);
                let range = (desc.max - desc.min).abs();
                let tolerance = if range > 0.0 { range * 0.01 } else { 1e-6 };

                let at_zero = adc_to_param_biased(&desc, noon, 0.0);
                let at_one = adc_to_param_biased(&desc, noon, 1.0);

                if (at_zero - desc.min).abs() > tolerance {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": knob=0 → {at_zero:.4}, expected min={:.4}",
                        desc.name, desc.min
                    ));
                }
                if (at_one - desc.max).abs() > tolerance {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": knob=1 → {at_one:.4}, expected max={:.4}",
                        desc.name, desc.max
                    ));
                }
            }
        }
    }

    assert_no_violations(
        "biased_mapping_endpoints",
        "biased knob 0/1 must still reach descriptor min/max",
        violations,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test D: Biased Noon at Center
// ─────────────────────────────────────────────────────────────────────────────

/// Knob at 0.5 must produce the noon sweet-spot value (within 1% of range).
#[test]
fn biased_noon_at_center() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();

    for id in all_ids_from(&registry) {
        let effect = registry.create(&id, SAMPLE_RATE).unwrap();
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx) {
                if desc.flags.contains(ParamFlags::READ_ONLY) {
                    continue;
                }
                // STEPPED params and near-extreme noons fall back to linear — skip
                if desc.flags.contains(ParamFlags::STEPPED) {
                    continue;
                }
                let Some(noon) = noon_value(&id, idx) else {
                    continue;
                };
                let range = desc.max - desc.min;
                if range < 1e-6
                    || (noon - desc.min) / range < 0.05
                    || (desc.max - noon) / range < 0.05
                {
                    continue; // falls back to linear — noon won't be at center
                }

                let at_center = adc_to_param_biased(&desc, noon, 0.5);
                let tolerance = range * 0.01;
                if (at_center - noon).abs() > tolerance {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": knob=0.5 → {at_center:.4}, expected noon={noon:.4}",
                        desc.name
                    ));
                }
            }
        }
    }

    assert_no_violations(
        "biased_noon_at_center",
        "knob center must produce the noon sweet-spot value",
        violations,
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Test E: Biased Mapping Monotonic
// ─────────────────────────────────────────────────────────────────────────────

/// Sweeping knob 0→1 through the biased mapping must produce monotonically
/// increasing output. STEPPED params allow flat regions; continuous params
/// use a small tolerance for floating-point noise.
#[test]
fn biased_mapping_monotonic() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();
    let steps = 200;

    for id in all_ids_from(&registry) {
        let effect = registry.create(&id, SAMPLE_RATE).unwrap();
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx) {
                if desc.flags.contains(ParamFlags::READ_ONLY) {
                    continue;
                }
                let noon = noon_value(&id, idx).unwrap_or(desc.default);
                let stepped = desc.flags.contains(ParamFlags::STEPPED);

                let mut prev = adc_to_param_biased(&desc, noon, 0.0);
                for i in 1..=steps {
                    let normalized = i as f32 / steps as f32;
                    let val = adc_to_param_biased(&desc, noon, normalized);

                    let is_violation = if stepped {
                        val < prev
                    } else {
                        val < prev - 1e-6
                    };

                    if is_violation {
                        violations.push(format!(
                            "  {id}[{idx}] \"{}\": non-monotonic at step {i}/{steps}: \
                             {val:.6} < {prev:.6}",
                            desc.name
                        ));
                        break;
                    }
                    prev = val;
                }
            }
        }
    }

    assert_no_violations(
        "biased_mapping_monotonic",
        "biased knob sweep must be monotonically increasing",
        violations,
    );
}
