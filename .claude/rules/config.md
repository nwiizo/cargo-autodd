# CLI Options and Configuration

## CLI Usage

```
cargo autodd [OPTIONS] [SUBCOMMAND]

Options:
    -d, --debug              Enable debug output
        --dry-run            Preview changes without modifying files
    -c, --config <FILE>      Path to config file (default: .cargo-autodd.toml)

Subcommands:
    update      Update dependencies to latest versions
    report      Generate dependency usage report
    security    Check for security vulnerabilities
```

## Config File Format

Create `.cargo-autodd.toml` in your project root:

```toml
# Crates to exclude from analysis (e.g., internal crates)
exclude = ["internal_crate", "another_internal"]

# Additional essential dependencies (never removed)
essential = ["custom_essential_lib"]

# Crates to always treat as dev-dependencies
dev_only = ["proptest", "criterion"]

# Skip tests/ directory analysis entirely
skip_tests = false
```
