# Build and Development Commands

```sh
# Build
cargo build

# Run tests
cargo test

# Run a single test
cargo test <test_name>

# Run ignored tests (requires network access to crates.io)
cargo test -- --ignored

# Format, lint, and test (run before committing)
cargo fmt && cargo clippy -- -D warnings && cargo test

# Run mutation testing
cargo mutants --timeout 60

# Run E2E tests
./scripts/e2e-test.sh

# Run the tool locally
cargo run -- autodd

# Run with debug mode
cargo run -- autodd --debug

# Run with dry-run mode
cargo run -- autodd --dry-run

# Run with custom config file
cargo run -- autodd --config /path/to/.cargo-autodd.toml
```
