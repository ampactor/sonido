# Testing Guide

This guide covers testing practices, patterns, and commands for the Sonido project.

## Quick Start

```bash
# Run all tests
cargo test --workspace

# Run tests for a specific crate
cargo test -p sonido-core
cargo test -p sonido-effects

# Run a specific test
cargo test test_reverb_basic

# Run tests with output
cargo test -- --nocapture
```

## Test Organization

Tests in Sonido follow Rust conventions with inline unit tests and separate integration tests.

### Unit Tests

Each module contains a `#[cfg(test)]` block with unit tests:

```rust
// In crates/sonido-effects/src/reverb.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reverb_basic() {
        let mut reverb = Reverb::new(48000.0);
        let output = reverb.process(0.5);
        assert!(output.is_finite());
    }
}
```

### Integration Tests

Integration tests are in `tests/` directories within crates:

```
crates/sonido-config/tests/integration.rs
```

Integration tests verify end-to-end functionality across modules.

## Test Categories

### Audio Processing Tests

Every effect should verify:

1. **Basic processing**: Output is finite and not NaN
2. **Zero input**: Processing silence produces valid output
3. **Bypass behavior**: Bypassed effects pass audio unchanged
4. **Sample rate changes**: Effects work at multiple sample rates
5. **Block processing**: `process_block()` matches sample-by-sample results

Example pattern:

```rust
#[test]
fn test_distortion_basic() {
    let mut effect = Distortion::new(48000.0);
    effect.set_drive_db(20.0);

    // Test single sample
    let output = effect.process(0.5);
    assert!(output.is_finite());
    assert!(output.abs() <= 1.0);
}

#[test]
fn test_distortion_block() {
    let mut effect = Distortion::new(48000.0);
    let input = vec![0.5; 512];
    let mut output = vec![0.0; 512];

    effect.process_block(&input, &mut output);

    assert!(output.iter().all(|&s| s.is_finite()));
}
```

### Parameter Tests

Verify parameter behavior:

```rust
#[test]
fn test_compressor_parameters() {
    let mut comp = Compressor::new(48000.0);

    // Test parameter ranges
    comp.set_threshold_db(-40.0);
    assert_eq!(comp.threshold_db(), -40.0);

    comp.set_ratio(10.0);
    assert!(comp.ratio() >= 1.0);
    assert!(comp.ratio() <= 20.0);
}
```

### ParameterInfo Tests

Effects implementing `ParameterInfo` should verify introspection:

```rust
#[test]
fn test_reverb_parameter_info() {
    let reverb = Reverb::new(48000.0);

    assert!(reverb.param_count() > 0);

    let info = reverb.param_info(0).unwrap();
    assert!(!info.name.is_empty());
    assert!(info.min <= info.default);
    assert!(info.default <= info.max);
}
```

### Stereo Processing Tests

True stereo effects need additional tests:

```rust
#[test]
fn test_reverb_stereo_decorrelation() {
    let mut reverb = Reverb::new(48000.0);
    reverb.set_mix(1.0);  // Full wet

    // Feed identical signal to both channels
    for _ in 0..1000 {
        reverb.process_stereo(0.5, 0.5);
    }

    // After warmup, L and R should be different (decorrelated)
    let (l, r) = reverb.process_stereo(0.5, 0.5);
    assert!((l - r).abs() > 0.001, "stereo reverb should decorrelate L/R");
}
```

## no_std Compatibility Testing

Core crates must work without the standard library.

### Running no_std Tests

```bash
# Test no_std compatibility for core crates
cargo test --no-default-features -p sonido-core
cargo test --no-default-features -p sonido-effects
cargo test --no-default-features -p sonido-registry
cargo test --no-default-features -p sonido-platform
cargo test --no-default-features -p sonido-synth
```

### no_std Test Requirements

When writing tests for no_std crates:

1. Import `Vec` from `alloc`:
   ```rust
   #[cfg(test)]
   mod tests {
       extern crate alloc;
       use alloc::vec;
       use alloc::vec::Vec;
   }
   ```

2. Avoid `std`-only features in tests
3. Use `libm` for math functions instead of `std::f32`

## Test Data and Fixtures

### Generating Test Signals

Use the CLI to generate test signals:

```bash
# Generate test tone
sonido generate tone test_440hz.wav --freq 440 --duration 1.0

# Generate sweep for IR capture
sonido generate sweep sweep.wav --start 20 --end 20000 --duration 3.0

# Generate noise
sonido generate noise noise.wav --duration 1.0
```

### In-Code Test Signal Generation

```rust
fn generate_test_signal(size: usize, sample_rate: f32) -> Vec<f32> {
    (0..size)
        .map(|i| {
            let t = i as f32 / sample_rate;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect()
}
```

## Continuous Integration

All tests run automatically on pull requests via GitHub Actions.

### CI Test Matrix

| Platform | Tests | no_std |
|----------|-------|--------|
| Linux (Ubuntu) | Full workspace | sonido-core, sonido-effects, sonido-registry, sonido-platform |
| macOS | Full workspace | - |
| Windows | Full workspace | - |

### Running CI Checks Locally

```bash
# Run the same checks as CI
cargo test --workspace
cargo test --no-default-features -p sonido-core -p sonido-effects
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Common Test Patterns

### Effect Chain Testing

```rust
#[test]
fn test_effect_chain() {
    use sonido_core::EffectExt;

    let preamp = CleanPreamp::new(48000.0);
    let distortion = Distortion::new(48000.0);

    let mut chain = preamp.chain(distortion);

    let output = chain.process(0.5);
    assert!(output.is_finite());
}
```

### Preset Testing

```rust
#[test]
fn test_factory_presets() {
    let presets = factory_presets();

    for preset in presets {
        let chain = EffectChain::from_preset(&preset, 48000.0);
        assert!(chain.is_ok(), "preset '{}' should be valid", preset.name);

        let mut chain = chain.unwrap();
        let output = chain.process(0.5);
        assert!(output.is_finite());
    }
}
```

### Reset Behavior Testing

```rust
#[test]
fn test_delay_reset() {
    let mut delay = Delay::new(48000.0);
    delay.set_delay_time_ms(100.0);
    delay.set_feedback(0.5);

    // Fill the delay buffer
    for _ in 0..10000 {
        delay.process(0.5);
    }

    // Reset should clear the buffer
    delay.reset();

    let output = delay.process(0.0);
    assert!(output.abs() < 0.01, "reset should clear delay buffer");
}
```

## Debugging Failed Tests

### Verbose Output

```bash
# Show println! output from tests
cargo test -- --nocapture

# Run a single test with backtrace
RUST_BACKTRACE=1 cargo test test_reverb_basic -- --nocapture
```

### Test Isolation

Run tests in a single thread to isolate issues:

```bash
cargo test -- --test-threads=1
```

## Documentation Tests

Rustdoc examples are also tested:

```bash
# Run documentation tests
cargo test --doc

# Test docs for a specific crate
cargo test --doc -p sonido-core
```

## Test Coverage

While not required, you can measure test coverage using cargo-tarpaulin:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage
cargo tarpaulin --workspace --out Html
```

## Adding New Tests

When adding a new effect or feature:

1. Add unit tests in the module's `#[cfg(test)]` block
2. Verify basic processing, parameters, and edge cases
3. Test no_std compatibility if applicable
4. Add integration tests if the feature spans multiple modules
5. Ensure documentation examples are testable

### Test Checklist

- [ ] Basic processing produces finite output
- [ ] Zero/silence input handled correctly
- [ ] Parameter ranges validated
- [ ] Sample rate changes work
- [ ] Block processing matches sample-by-sample
- [ ] Reset clears internal state
- [ ] no_std compatible (for core crates)

## See Also

- [Contributing](CONTRIBUTING.md) - Development guidelines
- [Benchmarks](BENCHMARKS.md) - Performance testing
- [Architecture](ARCHITECTURE.md) - System design
