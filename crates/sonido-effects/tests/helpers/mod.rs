//! Shared test helpers for sonido-effects integration tests.
//!
//! Each integration test compiles as a separate binary, so not every helper
//! is used in every binary. The `allow(dead_code)` on shared functions is
//! intentional — it communicates "this is shared infrastructure, not orphaned code."

use sonido_registry::EffectRegistry;

/// All effect IDs currently registered.
#[allow(dead_code)]
pub fn all_ids() -> Vec<String> {
    EffectRegistry::new()
        .all_effects()
        .into_iter()
        .map(|d| d.id.to_string())
        .collect()
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
