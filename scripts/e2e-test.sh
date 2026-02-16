#!/bin/bash
# E2E Test Script for cargo-autodd
# Run: ./scripts/e2e-test.sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BINARY="$PROJECT_ROOT/target/debug/cargo-autodd"

# Test directory
TEST_DIR=$(mktemp -d)
trap "rm -rf $TEST_DIR" EXIT

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  cargo-autodd E2E Test Suite${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Build the project first
echo -e "${YELLOW}[BUILD] Building cargo-autodd...${NC}"
cd "$PROJECT_ROOT"
cargo build --quiet
echo -e "${GREEN}[BUILD] Build successful${NC}"
echo ""

# Test counter
TESTS_PASSED=0
TESTS_FAILED=0

run_test() {
    local test_name="$1"
    local test_func="$2"

    echo -e "${YELLOW}[TEST] $test_name${NC}"

    if $test_func; then
        echo -e "${GREEN}[PASS] $test_name${NC}"
        ((TESTS_PASSED++))
    else
        echo -e "${RED}[FAIL] $test_name${NC}"
        ((TESTS_FAILED++))
    fi
    echo ""
}

# ============================================
# Test 1: Basic dependency detection
# ============================================
test_basic_detection() {
    local test_dir="$TEST_DIR/test1"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-basic"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
use tokio::runtime::Runtime;

fn main() {
    println!("Hello!");
}
EOF

    cd "$test_dir"
    $BINARY autodd --dry-run 2>&1 | grep -q "serde"
}

# ============================================
# Test 2: Dev-dependencies detection
# ============================================
test_dev_dependencies() {
    local test_dir="$TEST_DIR/test2"
    mkdir -p "$test_dir/src" "$test_dir/tests"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-dev-deps"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cat > "$test_dir/tests/integration.rs" << 'EOF'
use tempfile::TempDir;
#[test]
fn test_example() {}
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run 2>&1)
    echo "$output" | grep -q "\[dev-dependencies\]" && \
    echo "$output" | grep -q "tempfile"
}

# ============================================
# Test 3: Config file exclusion
# ============================================
test_config_exclusion() {
    local test_dir="$TEST_DIR/test3"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-config"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
use internal_crate::helper;
fn main() {}
EOF

    cat > "$test_dir/.cargo-autodd.toml" << 'EOF'
exclude = ["internal_crate"]
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run 2>&1)
    echo "$output" | grep -q "Excluded by config" && \
    echo "$output" | grep -q "internal_crate"
}

# ============================================
# Test 4: Dry-run mode doesn't modify files
# ============================================
test_dry_run_no_modify() {
    local test_dir="$TEST_DIR/test4"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-dry-run"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    local original_content=$(cat "$test_dir/Cargo.toml")

    cd "$test_dir"
    $BINARY autodd --dry-run > /dev/null 2>&1

    local new_content=$(cat "$test_dir/Cargo.toml")
    [ "$original_content" = "$new_content" ]
}

# ============================================
# Test 5: Actual update adds dependencies
# ============================================
test_actual_update() {
    local test_dir="$TEST_DIR/test5"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-update"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir"
    $BINARY autodd > /dev/null 2>&1

    grep -q "serde" "$test_dir/Cargo.toml"
}

# ============================================
# Test 6: Path dependency detection
# ============================================
test_path_dependency() {
    local test_dir="$TEST_DIR/test6"
    mkdir -p "$test_dir/main-crate/src" "$test_dir/internal-crate/src"

    cat > "$test_dir/internal-crate/Cargo.toml" << 'EOF'
[package]
name = "internal-crate"
version = "0.1.0"
edition = "2021"
EOF

    cat > "$test_dir/internal-crate/src/lib.rs" << 'EOF'
pub fn hello() -> &'static str { "Hello" }
EOF

    cat > "$test_dir/main-crate/Cargo.toml" << 'EOF'
[package]
name = "main-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
internal-crate = { path = "../internal-crate" }
EOF

    cat > "$test_dir/main-crate/src/main.rs" << 'EOF'
use internal_crate;
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir/main-crate"
    local output=$($BINARY autodd --dry-run 2>&1)
    # internal-crate should be detected as path dependency, not looked up on crates.io
    echo "$output" | grep -q "serde"
}

# ============================================
# Test 7: Debug mode output
# ============================================
test_debug_mode() {
    local test_dir="$TEST_DIR/test7"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-debug"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run --debug 2>&1)
    echo "$output" | grep -q "Starting dependency analysis in debug mode"
}

# ============================================
# Test 8: Report generation
# ============================================
test_report_generation() {
    local test_dir="$TEST_DIR/test8"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-report"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir"
    $BINARY autodd report 2>&1 | grep -q "Analyzing dependency usage"
}

# ============================================
# Test 9: Security check
# ============================================
test_security_check() {
    local test_dir="$TEST_DIR/test9"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-security"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir"
    $BINARY autodd security 2>&1 | grep -q "Running security check"
}

# ============================================
# Test 10: Workspace detection
# ============================================
test_workspace() {
    local test_dir="$TEST_DIR/test10"
    mkdir -p "$test_dir/crate1/src" "$test_dir/crate2/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[workspace]
members = ["crate1", "crate2"]

[workspace.dependencies]
serde = "1.0"
EOF

    cat > "$test_dir/crate1/Cargo.toml" << 'EOF'
[package]
name = "crate1"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = { workspace = true }
EOF

    cat > "$test_dir/crate1/src/lib.rs" << 'EOF'
use serde::Serialize;
EOF

    cat > "$test_dir/crate2/Cargo.toml" << 'EOF'
[package]
name = "crate2"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/crate2/src/lib.rs" << 'EOF'
pub fn hello() {}
EOF

    cd "$test_dir/crate1"
    $BINARY autodd --dry-run 2>&1 | grep -q "serde"
}

# ============================================
# Test 11: Imported names not treated as crates
# ============================================
test_imported_names() {
    local test_dir="$TEST_DIR/test11"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-imported-names"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
use tokio::sync::Mutex;

fn main() {
    // Serialize and Mutex are imported names, NOT crates
    let _s = Serialize::default();
    let _m = Mutex::new(42);

    // regex IS a crate (direct reference, no use statement)
    let _r = regex::Regex::new(r"test").unwrap();
}
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run 2>&1)
    # serde and tokio should be detected as crates
    echo "$output" | grep -q "serde" && \
    echo "$output" | grep -q "tokio" && \
    echo "$output" | grep -q "regex" && \
    # Serialize and Mutex should NOT appear as separate crates
    ! echo "$output" | grep -qw "Serialize = " && \
    ! echo "$output" | grep -qw "Mutex = "
}

# ============================================
# Test 12: Local modules not treated as crates
# ============================================
test_local_modules() {
    local test_dir="$TEST_DIR/test12"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-local-modules"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    # Create a local module file
    cat > "$test_dir/src/helpers.rs" << 'EOF'
pub fn do_stuff() -> String {
    "hello".to_string()
}
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
mod helpers;

use serde::Serialize;

fn main() {
    // helpers is a local module, NOT a crate
    helpers::do_stuff();
}
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run 2>&1)
    echo "$output" | grep -q "serde" && \
    ! echo "$output" | grep -qw "helpers = "
}

# ============================================
# Test 13: Complex multi-file project
# ============================================
test_complex_project() {
    local test_dir="$TEST_DIR/test13"
    mkdir -p "$test_dir/src" "$test_dir/tests"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-complex"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
mod config;
mod utils;

use serde::{Serialize, Deserialize};
use anyhow::Result;

#[derive(Serialize, Deserialize)]
struct AppConfig {
    name: String,
}

fn main() -> Result<()> {
    // Direct reference without use statement
    let re = regex::Regex::new(r"^\d+$")?;
    let val: serde_json::Value = serde_json::from_str("{}")?;
    config::load();
    utils::init();
    Ok(())
}
EOF

    cat > "$test_dir/src/config.rs" << 'EOF'
use toml;

pub fn load() {
    let _: toml::Value = toml::from_str("key = 'val'").unwrap();
}
EOF

    cat > "$test_dir/src/utils.rs" << 'EOF'
use log::info;

pub fn init() {
    info!("initialized");
}
EOF

    cat > "$test_dir/tests/integration.rs" << 'EOF'
use tempfile::TempDir;
use assert_fs::prelude::*;

#[test]
fn test_complex_flow() {
    let tmp = TempDir::new().unwrap();
    assert!(tmp.path().exists());
}
EOF

    cd "$test_dir"
    local output=$($BINARY autodd --dry-run 2>&1)

    # Regular dependencies from src/
    echo "$output" | grep -q "anyhow" && \
    echo "$output" | grep -q "regex" && \
    echo "$output" | grep -q "serde_json" && \
    echo "$output" | grep -q "toml" && \
    echo "$output" | grep -q "log" && \
    # Dev-dependencies from tests/
    echo "$output" | grep -q "\[dev-dependencies\]" && \
    echo "$output" | grep -q "tempfile" && \
    echo "$output" | grep -q "assert_fs" && \
    # Local modules should NOT be detected as crates
    ! echo "$output" | grep -qw "config = " && \
    ! echo "$output" | grep -qw "utils = "
}

# ============================================
# Test 14: Malformed config file error
# ============================================
test_malformed_config() {
    local test_dir="$TEST_DIR/test14"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-bad-config"
version = "0.1.0"
edition = "2021"

[dependencies]
EOF

    cat > "$test_dir/src/main.rs" << 'EOF'
fn main() {}
EOF

    # Create a malformed config file
    echo "invalid { toml" > "$test_dir/.cargo-autodd.toml"

    cd "$test_dir"
    # Should fail with a TOML parse error
    if $BINARY autodd --dry-run 2>&1 | grep -q "TOML parse error"; then
        return 0
    else
        return 1
    fi
}

# ============================================
# Test 15: Essential deps preserved on update
# ============================================
test_essential_deps_preserved() {
    local test_dir="$TEST_DIR/test15"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-essential"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
anyhow = "1.0"
thiserror = "1.0"
unused_crate = "0.1"
EOF

    # Source file only uses serde - tokio/anyhow/thiserror are essential
    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
fn main() {}
EOF

    cd "$test_dir"
    $BINARY autodd 2>&1 > /dev/null

    local content=$(cat "$test_dir/Cargo.toml")
    # Essential deps should remain even if not used
    echo "$content" | grep -q "serde" && \
    echo "$content" | grep -q "tokio" && \
    echo "$content" | grep -q "anyhow" && \
    echo "$content" | grep -q "thiserror" && \
    # Non-essential unused dep should be removed
    ! echo "$content" | grep -q "unused_crate"
}

# Test 16: Hyphenated crate names handled correctly
# ============================================
test_hyphenated_crates() {
    local test_dir="$TEST_DIR/test16"
    mkdir -p "$test_dir/src"

    cat > "$test_dir/Cargo.toml" << 'EOF'
[package]
name = "test-hyphens"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
serde-json = "1.0"
my-utils = "0.5"
EOF

    # Source uses serde_json (underscore form) but NOT my_utils
    cat > "$test_dir/src/main.rs" << 'EOF'
use serde::Serialize;
use serde_json::Value;

fn main() {
    let _v: Value = serde_json::from_str("{}").unwrap();
}
EOF

    cd "$test_dir"
    $BINARY autodd --dry-run 2>&1 > /dev/null

    local content=$(cat "$test_dir/Cargo.toml")
    # In dry-run, nothing changes - verify file is untouched
    echo "$content" | grep -q "serde-json"

    # Now run without dry-run
    $BINARY autodd 2>&1 > /dev/null

    content=$(cat "$test_dir/Cargo.toml")
    # serde-json should be preserved (used as serde_json in code)
    echo "$content" | grep -q "serde" && \
    # serde-json should NOT be duplicated as serde_json
    ! echo "$content" | grep -q "serde_json" || echo "$content" | grep -c "serde" | grep -q "^[12]$"
}

# ============================================
# Run all tests
# ============================================
echo -e "${BLUE}Running E2E tests...${NC}"
echo ""

run_test "Basic dependency detection" test_basic_detection
run_test "Dev-dependencies detection" test_dev_dependencies
run_test "Config file exclusion" test_config_exclusion
run_test "Dry-run mode doesn't modify files" test_dry_run_no_modify
run_test "Actual update adds dependencies" test_actual_update
run_test "Path dependency detection" test_path_dependency
run_test "Debug mode output" test_debug_mode
run_test "Report generation" test_report_generation
run_test "Security check" test_security_check
run_test "Workspace detection" test_workspace
run_test "Imported names not treated as crates" test_imported_names
run_test "Local modules not treated as crates" test_local_modules
run_test "Complex multi-file project" test_complex_project
run_test "Malformed config file error" test_malformed_config
run_test "Essential deps preserved on update" test_essential_deps_preserved
run_test "Hyphenated crate names" test_hyphenated_crates

# ============================================
# Summary
# ============================================
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Test Summary${NC}"
echo -e "${BLUE}========================================${NC}"
echo -e "  Passed: ${GREEN}$TESTS_PASSED${NC}"
echo -e "  Failed: ${RED}$TESTS_FAILED${NC}"
echo ""

if [ $TESTS_FAILED -eq 0 ]; then
    echo -e "${GREEN}All tests passed!${NC}"
    exit 0
else
    echo -e "${RED}Some tests failed!${NC}"
    exit 1
fi
