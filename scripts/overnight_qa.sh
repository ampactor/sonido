#!/usr/bin/env bash
# Overnight QA pipeline for Sonido DSP Framework
# Runs the full test, lint, doc, and bench suite.
# Usage: ./scripts/overnight_qa.sh [--quick]
#   --quick  Skip benchmarks and long-running tests

set -euo pipefail

QUICK=false
if [[ "${1:-}" == "--quick" ]]; then
    QUICK=true
fi

LOGDIR="target/qa-logs"
mkdir -p "$LOGDIR"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
LOG="$LOGDIR/qa_${TIMESTAMP}.log"

passed=0
failed=0
skipped=0

section() {
    echo ""
    echo "================================================================"
    echo "  $1"
    echo "================================================================"
    echo ""
}

run_step() {
    local name="$1"
    shift
    echo -n "[$name] ... "
    if "$@" >> "$LOG" 2>&1; then
        echo "PASS"
        ((passed++))
    else
        echo "FAIL (see $LOG)"
        ((failed++))
    fi
}

echo "Sonido Overnight QA — $(date)" | tee "$LOG"
echo "Log: $LOG"
echo ""

# ── 1. Formatting ──────────────────────────────────────────────────
section "Formatting"
run_step "cargo fmt check" cargo fmt --all -- --check

# ── 2. Clippy ──────────────────────────────────────────────────────
section "Clippy Lints"
run_step "clippy (all features)" cargo clippy --workspace --all-features --all-targets -- -D warnings
run_step "clippy (no default features)" cargo clippy --workspace --no-default-features -- -D warnings

# ── 3. Build ───────────────────────────────────────────────────────
section "Build"
run_step "build (release)" cargo build --workspace --release
run_step "build (no default features)" cargo build --workspace --no-default-features

# ── 4. Tests ───────────────────────────────────────────────────────
section "Tests"
run_step "unit + integration tests" cargo test --workspace --all-features
run_step "no_std core tests" cargo test --no-default-features -p sonido-core
run_step "no_std effects tests" cargo test --no-default-features -p sonido-effects
run_step "no_std synth tests" cargo test --no-default-features -p sonido-synth
run_step "no_std registry tests" cargo test --no-default-features -p sonido-registry
run_step "no_std platform tests" cargo test --no-default-features -p sonido-platform
run_step "doc tests" cargo test --doc --workspace
run_step "regression tests" cargo test --test regression -p sonido-effects
run_step "extreme param tests" cargo test --test extreme_params -p sonido-effects

# ── 5. Documentation ──────────────────────────────────────────────
section "Documentation"
run_step "rustdoc (no warnings)" cargo doc --no-deps --all-features 2>&1 | (! grep -i "warning")

# ── 6. Dependency audit ───────────────────────────────────────────
section "Dependency Audit"
if command -v cargo-deny &> /dev/null; then
    run_step "cargo deny" cargo deny check
else
    echo "[cargo deny] SKIPPED (not installed: cargo install cargo-deny)"
    ((skipped++))
fi

# ── 7. Benchmarks (skip in quick mode) ────────────────────────────
if [[ "$QUICK" == false ]]; then
    section "Benchmarks"
    run_step "benchmarks" cargo bench --workspace
else
    echo ""
    echo "[benchmarks] SKIPPED (--quick mode)"
    ((skipped++))
fi

# ── 8. Examples ───────────────────────────────────────────────────
section "Examples"
run_step "example: tempo_sync_demo" cargo run --example tempo_sync_demo -p sonido-core
run_step "example: synthesis_demo" cargo run --example synthesis_demo -p sonido-synth
run_step "example: modulation_demo" cargo run --example modulation_demo -p sonido-synth
run_step "example: preset_demo" cargo run --example preset_demo -p sonido-config
run_step "example: analysis_demo" cargo run --example analysis_demo -p sonido-analysis

# ── Summary ───────────────────────────────────────────────────────
section "Summary"
total=$((passed + failed + skipped))
echo "Total: $total  |  Passed: $passed  |  Failed: $failed  |  Skipped: $skipped"
echo ""

if [[ $failed -gt 0 ]]; then
    echo "QA FAILED — review $LOG for details"
    exit 1
else
    echo "QA PASSED"
    exit 0
fi
