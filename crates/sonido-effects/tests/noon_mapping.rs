//! Host-side verification of noon presets and biased knob mapping.
//!
//! The biased ADC mapping (`adc_to_param_biased`) lives in `sonido-daisy`
//! (no_std Cortex-M7) where unit tests can't run. It's pure math depending
//! only on `ParamDescriptor`, so we inline it here with `std` math
//! substitutions and run exhaustive checks against every registered effect.
//!
//! The noon preset table itself lives in `sonido_core::noon` (single source
//! of truth) — no inlined copy needed.

mod helpers;

use helpers::{all_ids_from, assert_no_violations};
use sonido_core::noon::{HARDWARE_MAPPED, noon_value};
use sonido_core::{ParamFlags, ParamScale};
use sonido_registry::EffectRegistry;

const SAMPLE_RATE: f32 = 48000.0;

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
    let val = interpolate_scaled(desc.min, desc.max, normalized, desc.scale);
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
// Test A: Noon Coverage for Hardware-Mapped Effects
// ─────────────────────────────────────────────────────────────────────────────

/// Every writable (non-READ_ONLY) parameter of a hardware-mapped effect must
/// have a noon preset. Effects not in `HARDWARE_MAPPED` are not checked.
#[test]
fn noon_coverage_hardware_mapped() {
    let registry = EffectRegistry::new();
    let mut violations = Vec::new();

    for &id in HARDWARE_MAPPED {
        let Some(effect) = registry.create(id, SAMPLE_RATE) else {
            violations.push(format!(
                "  {id}: listed in HARDWARE_MAPPED but not in registry"
            ));
            continue;
        };
        let count = effect.effect_param_count();

        for idx in 0..count {
            if let Some(desc) = effect.effect_param_info(idx) {
                let is_read_only = desc.flags.contains(ParamFlags::READ_ONLY);
                let has_noon = noon_value(id, idx).is_some();

                if !is_read_only && !has_noon {
                    violations.push(format!(
                        "  {id}[{idx}] \"{}\": writable param missing noon preset",
                        desc.name
                    ));
                }
            }
        }
    }

    assert_no_violations(
        "noon_coverage_hardware_mapped",
        "every writable param of a hardware-mapped effect needs a noon preset",
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
