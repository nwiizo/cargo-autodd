# Dependency Analyzer Implementation

## Overview

The dependency analyzer in `cargo-autodd` is responsible for scanning Rust source files to detect which external crates are used in a project. This document explains the evolution from a regex-based approach to a proper AST-based implementation using the `syn` crate.

## Problems with Regex/Lexical Scanning

The original implementation used regular expressions and character-by-character scanning to extract crate names from Rust source code. This approach had several fundamental flaws:

### 1. **Fragile Comment Handling**

Regex-based scanning would extract crate names from commented code:

```rust
// use old_crate::Thing;  // This is commented out!
/*
use another_crate::Foo;   // But regex would still find it
*/
```

The analyzer would incorrectly detect `old_crate` and `another_crate` as dependencies.

### 2. **String Literal False Positives**

Code containing crate-like patterns in strings would be incorrectly detected:

```rust
let example = "You should use serde_json::Value for this";
// regex scanner would think serde_json is used here
```

### 3. **Complex Use Statement Parsing**

Multi-line and nested use statements were handled incorrectly:

```rust
use {
    serde::{
        Serialize,
        Deserialize
    },
    tokio::runtime::Runtime
};
```

The regex approach would miss crates or extract wrong segments.

### 4. **The Imported Name Problem (Critical Bug)**

The most serious flaw was that direct reference scanning couldn't distinguish between crate names and imported names:

```rust
use foo::bar;

fn main() {
    bar::thing();  // bar is NOT a crate, it's imported from foo!
}
```

The regex scanner would see `bar::` and incorrectly add `bar` as a dependency. This led to:
- False positives (non-existent crates in dependencies)
- Confusion when `bar` couldn't be found on crates.io
- Incorrect dependency lists

Similarly with renamed imports:

```rust
use serde::Deserialize;

fn process(d: Deserialize) {
    Deserialize::deserialize();  // Deserialize is not a crate!
}
```

### 5. **Path Segment Confusion**

When scanning for direct references like `std::fs::File`, the character scanner would extract each segment (`std`, `fs`, `File`) rather than just the first segment that represents the crate.

### 6. **Local Module Confusion**

Local module declarations were not tracked:

```rust
mod util;

use util::helper;

fn main() {
    util::something();  // util is a local module, not a crate!
}
```

The regex scanner would incorrectly treat `util` as an external crate dependency.

### 7. **Standard Library Trait References**

Common trait usage patterns would be misidentified:

```rust
let cm = Cm { address, path, ..Default::default() };
// Default is a std trait, not a crate!
```

The scanner would see `Default::default()` and add `Default` as a dependency.

## The Solution: AST-Based Parsing with `syn`

The refactored implementation uses the `syn` crate to properly parse Rust code into an Abstract Syntax Tree (AST), then uses the Visitor pattern to extract dependencies correctly.

### Architecture

The analyzer now uses three separate AST visitors:

#### 0. **ModuleCollector** - Pre-Pass to Track Local Modules

```rust
struct ModuleCollector<'a> {
    local_modules: &'a mut HashSet<String>,
    depth: usize,  // Track nesting depth
}
```

**Responsibilities:**
- Visit **top-level** `mod` declarations only (depth == 0)
- Ensures local modules are not confused with external crates
- Correctly handles scoping - nested modules don't conflict with external crates

**Example:**
```rust
mod util;  // Top-level: added to local_modules

mod parent {
    mod bar;  // Nested: NOT added (this is parent::bar, not top-level)
}

use bar::Thing;  // This "bar" is an external crate - not filtered
```

**Why only top-level?** Only top-level `mod` declarations can conflict with external crate names in `use` statements at the file scope. Nested modules have qualified names (`parent::bar`) and don't shadow external crates.

#### 1. **CrateVisitor** - Extract Crates and Track Imports

```rust
struct CrateVisitor<'a> {
    crate_refs: &'a mut HashMap<String, CrateReference>,
    file_path: &'a PathBuf,
    debug: bool,
    scope_stack: &'a mut Vec<HashSet<String>>,  // Stack of scopes, each containing imported names
    local_modules: &'a HashSet<String>,  // Reference to collected modules
}
```

**Responsibilities:**
- Visit `use` statements and extract the root crate name (excluding local modules)
- Visit `extern crate` declarations
- **Track imported names in a scope stack** (including renames)
- Push/pop scopes when entering/leaving functions, blocks, impls, and traits

**Scope Stack:**
Imported names are tracked using a stack of HashSets:
- `scope_stack[0]`: File-level scope (imports at the top level)
- `scope_stack[1]`: First nested scope (inside a function or block)
- `scope_stack[2]`: Second nested scope (nested block), etc.

When entering a scope (function/block/impl/trait), a new empty HashSet is pushed onto the stack. When leaving, it's popped. This ensures that names imported in one scope don't leak into sibling scopes.

**Examples:**
```rust
use serde::Serialize;
// Adds "serde" as crate, adds "Serialize" to imported_names

use foo::bar as baz;
// Adds "foo" as crate, adds "baz" to imported_names

use {tokio, regex};
// Adds both "tokio" and "regex" as crates and to imported_names

mod util;
// Does NOT add "util" as a crate (it's a local module)
```

#### 2. **PathVisitor** - Find Direct References

```rust
struct PathVisitor<'a> {
    crate_refs: &'a mut HashMap<String, CrateReference>,
    file_path: &'a PathBuf,
    debug: bool,
    scope_stack: &'a mut Vec<HashSet<String>>,  // Same stack from CrateVisitor
    local_modules: &'a HashSet<String>,
}
```

**Responsibilities:**
- Visit all path expressions (`Expr::Path`) in the code
- Visit all type paths (`Type::Path`) in type annotations
- Extract the first segment of multi-segment paths
- **Exclude any name in any scope of the stack** (imported names are not crates)
- **Exclude any name that appears in `local_modules`**
- Push/pop scopes when entering/leaving functions, blocks, impls, and traits (same as CrateVisitor)

**Scope-Aware Filtering:**
When checking if a name should be filtered, it checks if the name exists in **any** scope in the current stack. This is correct because Rust's scoping rules make parent scopes visible to child scopes.

**Examples:**
```rust
use foo::bar;  // Added to scope_stack[0]
mod util;

fn test() {    // Pushes scope_stack[1]
    use inner::Data;  // Added to scope_stack[1]

    bar::thing();          // ❌ NOT added (bar found in scope_stack[0])
    util::helper();        // ❌ NOT added (util is in local_modules)
    Data::method();        // ❌ NOT added (Data found in scope_stack[1])
    baz::something();      // ✅ Added (baz not in any scope)
}  // Pops scope_stack[1]

fn other() {   // Pushes a new scope_stack[1]
    Data::method();        // ✅ Added! (Data not in scope_stack - was popped)
    bar::thing();          // ❌ NOT added (bar still in scope_stack[0])
}
```

### Three-Pass Analysis

The `analyze_file` method performs a three-pass analysis:

```rust
fn analyze_file(&self, content: &str, ...) -> Result<()> {
    let syntax = syn::parse_file(content)?;

    // PRE-PASS: Collect local module declarations
    let mut local_modules = HashSet::new();
    let mut module_collector = ModuleCollector { local_modules: &mut local_modules, depth: 0 };
    module_collector.visit_file(&syntax);

    // PASS 1: Extract crates from use statements and track imports with scope stack
    let mut scope_stack = vec![HashSet::new()];  // Start with file-level scope
    let mut first_pass_visitor = CrateVisitor {
        scope_stack: &mut scope_stack,
        local_modules: &local_modules,
        ...
    };
    first_pass_visitor.visit_file(&syntax);

    // PASS 2: Find direct references, excluding imported names and local modules
    // Reuses the SAME scope_stack so PathVisitor sees what was imported
    let mut path_visitor = PathVisitor {
        scope_stack: &mut scope_stack,  // Reused from first pass!
        local_modules: &local_modules,
        ...
    };
    path_visitor.visit_file(&syntax);

    Ok(())
}
```

**Key Design Decision:** Both visitors traverse the AST in the same order and push/pop scopes at the same points. After the first pass completes, all nested scopes have been popped, leaving only `scope_stack[0]` with file-level imports. The second pass then reuses this stack, maintaining the same push/pop behavior, so both visitors see the same scope structure.

### Handling Edge Cases

#### Scoped Use Statements

The implementation correctly handles scoped `use` statements:

```rust
fn foo() {
    use serde::Serialize;  // imported at depth 1
    Serialize::method();   // ❌ NOT added (Serialize visible at depth 1)
}

fn bar() {
    use Serialize::Thing;  // different scope, depth 1
    Serialize::method();   // ✅ Added! (different Serialize, a crate)
}
```

Each scope tracks its own imports. A name imported at depth D is only visible at depth D and deeper (D+1, D+2, etc.). This prevents false negatives when the same name refers to different things in different scopes.

#### Statement Order

The three-pass approach correctly handles any statement order:

```rust
use util::helper;  // use comes first
mod util;          // mod comes after - still works!
```

The pre-pass visits the entire file before processing `use` statements, so declaration order doesn't matter.

#### Root-Level Groups

```rust
use {
    serde::{Serialize, Deserialize},
    tokio::runtime::Runtime
};
```

Each item in a root-level group is treated as root-level, ensuring both `serde` and `tokio` are detected.

#### Simple Bare Imports

```rust
use serde;
use regex as re;
```

Both patterns are handled: `serde` is both a crate and an imported name; `regex` is the crate but only `re` is the imported name.

#### Extern Crate with Rename

```rust
extern crate serde as serde_crate;
```

Correctly identifies `serde` as the crate dependency and tracks `serde_crate` as the imported name.

#### Local Modules

```rust
mod util;
use util::helper;
```

The pre-pass identifies `util` as a local module, so it's never treated as an external crate dependency.

#### Standard Library Traits

```rust
let cm = Cm { ..Default::default() };
```

`Default` is recognized as a standard library trait and filtered out, along with other common traits like `Clone`, `Copy`, `Debug`, and `Display`.

## Benefits of AST-Based Approach

1. **Correctness**: Uses the Rust compiler's own parser, so handles all valid Rust syntax
2. **No false positives from comments**: AST doesn't include comments
3. **No false positives from strings**: String literals are separate AST nodes
4. **Proper import tracking**: Solves the imported name problem completely
5. **Local module awareness**: Distinguishes between external crates and local modules
6. **Standard library filtering**: Filters out common std traits and types
7. **Maintainable**: Easier to understand and modify than regex patterns
8. **Future-proof**: Will handle new Rust syntax as `syn` is updated

## Performance Considerations

While AST parsing is more expensive than regex, the benefits far outweigh the cost:
- Parsing happens once per file during analysis
- The cost is negligible compared to network requests to crates.io
- Correctness is more important than micro-optimizations in this tool

## Testing

The implementation includes comprehensive tests covering:
- Simple use statements
- Complex nested use statements
- Group imports at root level
- Direct path references
- **The critical imported names bug** (verified fixed)
- **Local modules not treated as crates** (verified fixed)
- Standard library trait filtering (Default, Clone, etc.)
- Dev dependencies from test directories
- Filtering of test-only crates
- Build script exclusion

All tests pass, confirming the robustness of the AST-based approach.
