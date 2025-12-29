# Coupling Analysis

Analyze coupling in Rust projects using cargo-coupling (Khononov's Balance framework).

## Usage

```sh
cargo coupling ./src
```

## Quick Commands

```sh
# Summary only
cargo coupling --summary ./src

# Japanese output
cargo coupling --summary --jp ./src

# Show all issues (including Low severity)
cargo coupling --summary --all ./src

# AI-friendly output for refactoring
cargo coupling --ai ./src
```

## Job-Focused Commands

```sh
# Top refactoring targets
cargo coupling --hotspots ./src
cargo coupling --hotspots=10 ./src

# With explanations
cargo coupling --hotspots --verbose ./src

# Change impact analysis
cargo coupling --impact main ./src

# Trace dependencies
cargo coupling --trace analyze_file ./src

# CI/CD quality gate
cargo coupling --check ./src
cargo coupling --check --min-grade=B ./src
cargo coupling --check --max-critical=0 --max-circular=0 ./src

# JSON output
cargo coupling --json ./src
```

## Web UI

```sh
cargo coupling --web ./src
cargo coupling --web --port 8080 ./src
```

## Interpretation

| Grade | Meaning |
|-------|---------|
| S | Over-optimized (warning) |
| A | Well-balanced (ideal) |
| B | Healthy |
| C | Room for improvement |
| D | Attention needed |
| F | Immediate action required |

## Balance Law

```
Strong coupling is only acceptable when:
- Distance is close (same module), OR
- Volatility is low (stable code)
```
