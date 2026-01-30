.PHONY: build test bench clean check demo doc install

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

# Run demo script
demo:
	./scripts/demo.sh

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
