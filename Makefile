.PHONY: build test bench clean check demo walkthrough verify doc install

# Build
build:
	cargo build --release

# Run all tests
test:
	cargo test

# Run benchmarks
bench:
	cargo bench

# Clean build artifacts
clean:
	cargo clean

# Run lints and format check
check:
	cargo clippy --all-targets -- -D warnings
	cargo fmt --check

# Format code
fmt:
	cargo fmt

# Generate documentation
doc:
	cargo doc --no-deps --all-features

# Generate demo WAV files (5 signals + 18 effect demos)
demo:
	./scripts/generate_demos.sh

# Full guided walkthrough (build, effects, analysis, presets)
walkthrough:
	./scripts/walkthrough.sh

# Verify existing demos work
verify:
	./scripts/walkthrough.sh --verify

# Install CLI globally
install:
	cargo install --path crates/sonido-cli

# Test no_std compatibility
test-nostd:
	cargo test --no-default-features -p sonido-core
	cargo test --no-default-features -p sonido-effects

# Full CI check
ci: check test test-nostd doc
	@echo "All CI checks passed"
