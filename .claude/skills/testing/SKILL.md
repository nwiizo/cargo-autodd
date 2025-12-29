# Testing Skill

Comprehensive testing workflow for cargo-autodd.

## Test Organization

| Type | Location | Description |
|------|----------|-------------|
| Unit tests | `#[cfg(test)] mod tests` | Inline in each module |
| Monorepo tests | `src/dependency_manager/tests/` | Workspace and path dependency |
| Integration tests | `tests/integration_tests.rs` | E2E workflow tests |
| Config tests | `src/config.rs` | Config file loading |
| E2E script | `scripts/e2e-test.sh` | 10 comprehensive tests |

## Quick Commands

```sh
# All tests
cargo test

# Single test
cargo test <test_name>

# Ignored tests (require network)
cargo test -- --ignored

# E2E suite
./scripts/e2e-test.sh

# Mutation testing
cargo mutants --timeout 60
```

## E2E Test Suite (10 tests)

1. Basic dependency detection
2. Dev-dependencies detection
3. Config file exclusion
4. Dry-run mode
5. Actual update
6. Path dependency detection
7. Debug mode output
8. Report generation
9. Security check
10. Workspace detection

## Ignored Tests

Require network access to crates.io:
- `test_monorepo_update_with_internal_crates`
- `test_monorepo_with_publish_false_crates`

## Test Patterns

Tests create temporary Cargo.toml and .rs files to verify:
- Use statement parsing (simple, nested, multi-line, with comments)
- Path dependency detection
- Workspace handling
- `publish = false` crate handling
- Dev-dependency detection from tests/
- Version comparison with various prefixes
- Config file loading and exclusions

## Mutation Testing

```sh
cargo mutants --timeout 60
```

Key areas for additional tests:
- Filtering logic in `analyzer.rs` (&&/|| replacements)
- Version comparison in `reporter.rs`
- Dependency removal in `updater.rs`
