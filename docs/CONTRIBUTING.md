# Contributing

Guidelines for contributing to the Sonido project.

## Development Setup

```bash
git clone https://github.com/suds/sonido
cd sonido
cargo build
cargo test
```

## Continuous Integration

All pull requests are automatically tested via GitHub Actions:

- **Test matrix**: Linux, macOS, Windows
- **no_std checks**: sonido-core, sonido-effects, sonido-registry, sonido-platform
- **Linting**: `cargo clippy --all-targets -- -D warnings`
- **Formatting**: `cargo fmt --all -- --check`

### Running CI Checks Locally

```bash
# Run the same checks as CI
cargo test --workspace
cargo test --no-default-features -p sonido-core -p sonido-effects
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Releases

Releases are built automatically when a version tag is pushed:

```bash
git tag v0.1.0
git push --tags
```

This triggers the release workflow which:
1. Builds binaries for Linux x64, macOS x64/ARM64, Windows x64
2. Packages with factory presets and documentation
3. Creates a GitHub release with downloadable artifacts

## Documentation Protocol

### When to Update Docs

- **New public API**: Update EFFECTS_REFERENCE.md or relevant crate docs
- **New CLI command**: Update CLI_GUIDE.md
- **Breaking change**: Update CHANGELOG.md with migration notes
- **New feature**: Update README.md features list

### Doc Checklist (before PR merge)

- [ ] Rustdoc on all public items
- [ ] Example in rustdoc if non-obvious
- [ ] CLI help text updated (clap derives)
- [ ] README updated if user-facing change
- [ ] CHANGELOG entry added

### Running Doc Checks

```bash
cargo doc --no-deps --all-features
cargo test --doc
```

## Commit Guidelines

### Message Format

```
<type>: <short description>

<optional body with details>
```

### Types

| Type | Description |
|------|-------------|
| `feat` | New feature |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `refactor` | Code change that neither fixes nor adds |
| `test` | Adding/updating tests |
| `bench` | Benchmark changes |
| `chore` | Maintenance (deps, CI, configs) |

### Examples

```
feat: Add reverb effect with room size parameter

fix: Correct delay feedback clipping at high values

docs: Add CLI usage examples to README

refactor: Extract common filter code to biquad module

chore: Update cpal to 0.15.3
```

### Rules

- Present tense ("Add feature" not "Added feature")
- No period at end of subject line
- Subject line ≤ 72 characters
- Reference issues: `fix: Resolve delay click (#42)`

## Code Style

### General

- Follow `rustfmt` defaults
- Use `cargo clippy` and address warnings
- Prefer explicit types in public APIs
- Document all public items

### DSP Code

- Use `f32` for audio samples (not `f64`)
- Use `SmoothedParam` for all user-controllable parameters
- Avoid allocations in audio processing hot paths
- Test for both `std` and `no_std` compatibility

### Testing

```bash
# Run all tests
cargo test

# Test no_std compatibility
cargo test --no-default-features -p sonido-core
cargo test --no-default-features -p sonido-effects

# Run benchmarks
cargo bench
```

### Benchmarks

Effects should be benchmarked with multiple block sizes:

```rust
fn bench_effect(c: &mut Criterion) {
    for block_size in [64, 128, 256, 512, 1024] {
        // benchmark code
    }
}
```

## Pull Request Process

1. Fork and create a feature branch
2. Make your changes
3. Ensure tests pass: `cargo test`
4. Ensure clippy passes: `cargo clippy --all-targets`
5. Update documentation as needed
6. Submit PR with clear description

### PR Title Format

Use the same format as commits:

```
feat: Add reverb effect
fix: Correct compressor attack time calculation
```

## Adding a New Effect

1. Create the effect in `crates/sonido-effects/src/`
2. Implement the `Effect` trait from `sonido-core`
3. Use `SmoothedParam` for parameters
4. Add tests in a `#[cfg(test)] mod tests` block
5. Export from `crates/sonido-effects/src/lib.rs`
6. Add to CLI in `crates/sonido-cli/src/effects.rs`
7. Document in `docs/EFFECTS_REFERENCE.md`
8. Add to README.md features list

### Effect Template

```rust
use sonido_core::{Effect, SmoothedParam};

pub struct MyEffect {
    sample_rate: f32,
    my_param: SmoothedParam,
    // internal state...
}

impl MyEffect {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            my_param: SmoothedParam::with_config(1.0, sample_rate, 10.0),
        }
    }

    pub fn set_my_param(&mut self, value: f32) {
        self.my_param.set_target(value);
    }
}

impl Effect for MyEffect {
    fn process(&mut self, input: f32) -> f32 {
        let param = self.my_param.advance();  // Use advance() instead of next()
        // process audio...
        input * param
    }

    fn process_block(&mut self, input: &[f32], output: &mut [f32]) {
        for (inp, out) in input.iter().zip(output.iter_mut()) {
            *out = self.process(*inp);
        }
    }

    fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.my_param.set_sample_rate(sample_rate);
    }

    fn reset(&mut self) {
        self.my_param.snap_to_target();
    }

    fn latency_samples(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_processing() {
        let mut effect = MyEffect::new(48000.0);
        let output = effect.process(0.5);
        assert!(output.is_finite());
    }
}
```

## Issue Reporting

When reporting bugs, include:

- Sonido version (`cargo pkgid sonido-core`)
- OS and version
- Steps to reproduce
- Expected vs actual behavior
- Audio file samples if relevant

## License

By contributing, you agree that your contributions will be licensed under the same MIT/Apache-2.0 dual license as the project.
