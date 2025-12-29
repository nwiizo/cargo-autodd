# CLAUDE.md

cargo-autodd: Automatically manages Rust dependencies by analyzing source code.

## Quick Start

```sh
cargo build                                    # Build
cargo test                                     # Run tests
cargo fmt && cargo clippy -- -D warnings       # Format and lint
cargo run -- autodd --dry-run                  # Preview changes
```

## Documentation

| Topic | Location |
|-------|----------|
| Commands | [.claude/rules/commands.md](.claude/rules/commands.md) |
| Architecture | [.claude/rules/architecture.md](.claude/rules/architecture.md) |
| CLI & Config | [.claude/rules/config.md](.claude/rules/config.md) |
| Limitations | [.claude/rules/limitations.md](.claude/rules/limitations.md) |
| Testing | [.claude/skills/testing/SKILL.md](.claude/skills/testing/SKILL.md) |

## Key Points

- **Analyzer**: Scans `use` statements, `extern crate`, direct refs (e.g., `serde_json::Value`)
- **Dev-deps**: Crates in `tests/` → `[dev-dependencies]`
- **Essential**: `serde`, `tokio`, `anyhow`, `thiserror`, `async-trait`, `futures` never removed
- **Path deps**: Auto-detected from Cargo.toml, skipped from crates.io
- **Config**: `.cargo-autodd.toml` for exclusions and customization

## Commands

| Command | Description |
|---------|-------------|
| `/e2e-test` | Run E2E test suite |
| `/similarity` | Detect code similarities |
| `/mutants` | Run mutation testing |
| `/coupling` | Analyze coupling (Khononov) |
