# Known Limitations

## Type names as crates
Direct references like `serde_json::Value` work correctly, but `use crate::TempDir as Foo` may incorrectly detect `TempDir` as a crate.

## Deeply nested imports
Complex nested use statements like `use crate::{a::{b::c, d}}` may not be fully parsed.

## Build dependencies
`build.rs` dependencies are not yet supported. Files are skipped during analysis.

## Workarounds

Use `.cargo-autodd.toml` to exclude false positives:

```toml
exclude = ["TempDir", "false_positive_crate"]
```
