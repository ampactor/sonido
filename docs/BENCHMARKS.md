# Benchmarks Guide

This guide covers performance benchmarking for the Sonido DSP framework.

## Quick Start

```bash
# Run all benchmarks
cargo bench

# Run benchmarks for a specific crate
cargo bench -p sonido-effects

# Run a specific benchmark
cargo bench -- Distortion

# Run with baseline comparison
cargo bench -- --save-baseline main
cargo bench -- --baseline main
```

## Benchmark Framework

Sonido uses [Criterion](https://github.com/bheisler/criterion.rs) for benchmarking, which provides:

- Statistical analysis with confidence intervals
- Automatic outlier detection
- HTML reports with graphs
- Baseline comparisons for regression detection

## Running Benchmarks

### Full Benchmark Suite

```bash
cargo bench
```

Results are saved to `target/criterion/` with HTML reports.

### View Reports

After running benchmarks, open the HTML report:

```bash
# Linux
xdg-open target/criterion/report/index.html

# macOS
open target/criterion/report/index.html

# Windows
start target/criterion/report/index.html
```

### Baseline Comparisons

Compare against a saved baseline to detect regressions:

```bash
# Save current performance as baseline
cargo bench -- --save-baseline before_changes

# Make changes, then compare
cargo bench -- --baseline before_changes
```

## Benchmark Organization

Benchmarks are located in `crates/*/benches/`:

```
crates/sonido-effects/benches/effects_bench.rs
```

### Structure

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sonido_core::Effect;
use sonido_effects::Distortion;

const SAMPLE_RATE: f32 = 48000.0;
const BLOCK_SIZES: &[usize] = &[64, 128, 256, 512, 1024];

fn bench_distortion(c: &mut Criterion) {
    let mut effect = Distortion::new(SAMPLE_RATE);
    effect.set_drive_db(20.0);

    let mut group = c.benchmark_group("Distortion");

    for &block_size in BLOCK_SIZES {
        let input: Vec<f32> = (0..block_size)
            .map(|i| (i as f32 * 0.01).sin() * 0.5)
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut output = vec![0.0; block_size];
                b.iter(|| {
                    effect.process_block(black_box(&input), &mut output);
                    black_box(output[0])
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_distortion);
criterion_main!(benches);
```

## Standard Block Sizes

All effect benchmarks test these block sizes:

| Block Size | Latency @ 48kHz | Use Case |
|------------|-----------------|----------|
| 64 samples | 1.3 ms | Ultra-low latency |
| 128 samples | 2.7 ms | Low latency monitoring |
| 256 samples | 5.3 ms | Standard real-time |
| 512 samples | 10.7 ms | Comfortable real-time |
| 1024 samples | 21.3 ms | Offline processing |

## Effect Benchmarks

### Individual Effects

Each effect is benchmarked with typical settings:

| Effect | Key Parameters |
|--------|----------------|
| Distortion | drive=20dB, tone=4kHz |
| Compressor | threshold=-20dB, ratio=4:1 |
| Chorus | rate=2Hz, depth=0.7 |
| Delay | time=375ms, feedback=0.5 |
| Reverb | room_size=0.7, decay=0.8 |
| Filter | cutoff=1kHz, Q=0.707 |
| MultiVibrato | depth=1.0 |
| TapeSaturation | drive=2.0 |
| CleanPreamp | gain=12dB |

### Effect Chain

A typical guitar chain is benchmarked:

```
Preamp -> Distortion -> Chorus -> Delay
```

This represents real-world usage with multiple effects active.

## Performance Expectations

### Target Performance

For real-time audio at 48kHz with 256-sample buffers:

- Single effect: < 0.5ms per buffer
- 4-effect chain: < 2ms per buffer
- CPU headroom: > 50% idle

### Sample Benchmarks

Typical results on modern hardware (2023 desktop):

| Effect | 256 samples | 512 samples |
|--------|-------------|-------------|
| Distortion | ~15 us | ~30 us |
| Compressor | ~20 us | ~40 us |
| Chorus | ~50 us | ~100 us |
| Delay | ~25 us | ~50 us |
| Reverb | ~200 us | ~400 us |
| 4-Effect Chain | ~100 us | ~200 us |

Note: These are approximate values. Actual performance depends on hardware.

## Writing New Benchmarks

### Adding an Effect Benchmark

1. Open `crates/sonido-effects/benches/effects_bench.rs`
2. Add a benchmark function:

```rust
fn bench_new_effect(c: &mut Criterion) {
    let mut effect = NewEffect::new(SAMPLE_RATE);
    // Set typical parameters
    bench_effect(c, "NewEffect", effect);
}
```

3. Add to the criterion group:

```rust
criterion_group!(
    benches,
    bench_distortion,
    // ... existing benchmarks
    bench_new_effect,
);
```

### Benchmark Helper

Use the shared `bench_effect` helper for consistency:

```rust
fn bench_effect<E: Effect>(c: &mut Criterion, name: &str, mut effect: E) {
    let mut group = c.benchmark_group(name);

    for &block_size in BLOCK_SIZES {
        let input = generate_test_signal(block_size);

        group.bench_with_input(
            BenchmarkId::from_parameter(block_size),
            &block_size,
            |b, _| {
                let mut output = vec![0.0; block_size];
                b.iter(|| {
                    effect.process_block(black_box(&input), &mut output);
                    black_box(output[0])
                })
            },
        );
    }

    group.finish();
}
```

## Profiling

### CPU Profiling with perf (Linux)

```bash
# Build with debug symbols
cargo build --release

# Profile a specific benchmark
perf record cargo bench -- Reverb --profile-time 10
perf report
```

### Flamegraph

```bash
# Install flamegraph
cargo install flamegraph

# Generate flamegraph for benchmark
cargo flamegraph --bench effects_bench -- --bench Reverb
```

### Instruments (macOS)

```bash
# Build with debug symbols
cargo build --release

# Run with Instruments
instruments -t "Time Profiler" target/release/sonido-cli process input.wav output.wav -e reverb
```

## Optimization Guidelines

### DSP Performance Tips

1. **Avoid allocations in process loops**: Pre-allocate buffers
2. **Use SmoothedParam**: Prevents zipper noise without per-sample branching
3. **Batch operations**: `process_block()` over `process()` when possible
4. **Minimize function calls**: Inline hot paths
5. **Use SIMD when available**: Compiler auto-vectorization helps

### Measuring Optimization Impact

```bash
# Before optimization
cargo bench -- Reverb --save-baseline before

# After optimization
cargo bench -- Reverb --baseline before
```

Criterion will show percentage improvement/regression.

## CI Benchmarks

Benchmarks are not run in CI by default (too variable across runners), but you can run them locally before merging:

```bash
# Compare against main branch
git stash
git checkout main
cargo bench -- --save-baseline main
git checkout -
git stash pop
cargo bench -- --baseline main
```

## Benchmark Reports

### Interpreting Results

```
Distortion/256          time:   [14.532 us 14.672 us 14.823 us]
                        change: [-2.1234% -0.8756% +0.3892%] (p = 0.18 > 0.05)
                        No change in performance detected.
```

- **time**: [lower bound, estimate, upper bound]
- **change**: Comparison to baseline (if set)
- **p-value**: Statistical confidence (< 0.05 = significant change)

### Outliers

Criterion automatically detects outliers:

```
Found 2 outliers among 100 measurements (2.00%)
  2 (2.00%) high mild
```

High outlier counts may indicate system interference.

## See Also

- [Testing](TESTING.md) - Test guide
- [Contributing](CONTRIBUTING.md) - Development guidelines
- [Architecture](ARCHITECTURE.md) - System design
