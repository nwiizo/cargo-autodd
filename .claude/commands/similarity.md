# Similarity Detection

Run semantic code similarity analysis using similarity-rs.

## Usage

```sh
similarity-rs .
```

## Options

```sh
# Analyze specific directory
similarity-rs ./src

# With threshold (0.0-1.0, default: 0.8)
similarity-rs . --threshold 0.7

# Output format
similarity-rs . --format json
similarity-rs . --format table

# Ignore patterns
similarity-rs . --ignore "tests/*" --ignore "target/*"
```

## Interpretation

- **High similarity (>0.9)**: Likely duplicate or copy-pasted code
- **Medium similarity (0.7-0.9)**: Similar patterns, consider abstraction
- **Low similarity (<0.7)**: Different implementations

## Use Cases

- Detect duplicate code for refactoring
- Find similar patterns across modules
- Identify candidates for shared utilities
