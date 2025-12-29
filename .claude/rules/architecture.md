# Architecture

cargo-autodd is a Cargo subcommand that automatically manages Rust dependencies by analyzing source code.

## Core Components

| Component | File | Description |
|-----------|------|-------------|
| `CargoAutodd` | `src/lib.rs` | Main facade orchestrating analyzer, updater, reporter |
| `Config` | `src/config.rs` | Configuration file support (`.cargo-autodd.toml`) |
| `DependencyAnalyzer` | `src/dependency_manager/analyzer.rs` | Scans Rust files for `use` statements, `extern crate`, direct references |
| `DependencyUpdater` | `src/dependency_manager/updater.rs` | Updates Cargo.toml, fetches versions from crates.io |
| `DependencyReporter` | `src/dependency_manager/reporter.rs` | Generates usage reports and security checks |
| `CrateReference` | `src/models/crate_reference.rs` | Represents crate with features, usage, path/dev flags |
| `crate_utils` | `src/utils/crate_utils.rs` | Filters std crates and essential dependencies |

## Key Behaviors

- **Path dependencies**: Detected from Cargo.toml, skipped from crates.io lookups
- **Std filtering**: `std`, `core`, `alloc` filtered via `is_std_crate()`
- **Essential deps**: `serde`, `tokio`, `anyhow`, `thiserror`, `async-trait`, `futures` never removed
- **Dev-dependencies**: Crates in `tests/` added to `[dev-dependencies]`
- **Dry-run mode**: `--dry-run` previews changes without modifying files
- **Config file**: `.cargo-autodd.toml` customizes exclusions and essential deps
- **Workspace support**: Detects `[workspace]` and uses `workspace.dependencies`
- **Version prefixes**: `^`, `~`, `=`, `>=`, `<=`, `>`, `<` properly stripped
