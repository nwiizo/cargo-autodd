# Mutation Testing

Run mutation testing using cargo-mutants to analyze test quality.

## Usage

```sh
cargo mutants --timeout 60
```

## Options

```sh
# Run on specific files
cargo mutants --timeout 60 -- src/dependency_manager/analyzer.rs

# Run specific mutants
cargo mutants --timeout 60 --re "filter"

# Show all mutants without running
cargo mutants --list

# Run in parallel (faster)
cargo mutants --timeout 60 -j 4

# Output JSON report
cargo mutants --timeout 60 --json > mutants-report.json
```

## Interpretation

| Result | Meaning |
|--------|---------|
| **caught** | Test detected the mutation (good) |
| **missed** | Test did not detect mutation (needs more tests) |
| **timeout** | Mutation caused infinite loop |
| **unviable** | Mutation broke compilation |

## Key Areas to Watch

- `analyzer.rs`: Filtering logic (&&/|| replacements)
- `reporter.rs`: Version comparison
- `updater.rs`: Dependency removal logic
- `crate_utils.rs`: Std crate filtering

## After Running

1. Review MISSED mutants
2. Add tests to catch missed mutations
3. Focus on critical business logic
