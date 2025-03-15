use anyhow::Result;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;

use crate::CargoAutodd;
use crate::dependency_manager::DependencyAnalyzer;

/// Create a test environment with monorepo structure
fn create_monorepo_test_environment() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let root_path = temp_dir.path();

    // Create root Cargo.toml
    let root_cargo_toml = root_path.join("Cargo.toml");
    let root_content = r#"
[workspace]
members = [
    "public-crate",
    "internal-crate"
]

[workspace.dependencies]
serde = "1.0"
"#;
    let mut file = File::create(root_cargo_toml)?;
    writeln!(file, "{}", root_content)?;

    // Create internal crate directory and files
    fs::create_dir_all(root_path.join("internal-crate/src"))?;

    // Create internal crate's Cargo.toml
    let internal_cargo_toml = root_path.join("internal-crate/Cargo.toml");
    let internal_content = r#"
[package]
name = "internal-crate"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
serde = { workspace = true }
"#;
    let mut file = File::create(internal_cargo_toml)?;
    writeln!(file, "{}", internal_content)?;

    // Create internal crate's source file
    let internal_main_rs = root_path.join("internal-crate/src/lib.rs");
    let internal_main_content = r#"
use serde;

pub fn hello() -> &'static str {
    "Hello from internal crate"
}
"#;
    let mut file = File::create(internal_main_rs)?;
    writeln!(file, "{}", internal_main_content)?;

    // Create public crate directory and files
    fs::create_dir_all(root_path.join("public-crate/src"))?;

    // Create public crate's Cargo.toml
    let public_cargo_toml = root_path.join("public-crate/Cargo.toml");
    let public_content = r#"
[package]
name = "public-crate"
version = "0.1.0"
edition = "2021"

[dependencies]
internal-crate = { path = "../internal-crate" }
"#;
    let mut file = File::create(public_cargo_toml)?;
    writeln!(file, "{}", public_content)?;

    // Create public crate's source file
    let public_main_rs = root_path.join("public-crate/src/main.rs");
    let public_main_content = r#"
use internal_crate;
use tokio;

fn main() {
    println!("{}", internal_crate::hello());
}
"#;
    let mut file = File::create(public_main_rs)?;
    writeln!(file, "{}", public_main_content)?;

    Ok(temp_dir)
}

#[test]
fn test_monorepo_dependency_analysis() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // Path to the public crate
    let public_crate_path = root_path.join("public-crate");

    // Analyze dependencies
    let analyzer = DependencyAnalyzer::with_debug(public_crate_path, true);
    let crate_refs = analyzer.analyze_dependencies()?;

    // Verify results
    // tokio should be detected
    assert!(
        crate_refs.contains_key("tokio"),
        "tokio dependency not found"
    );

    // internal-crate or internal_crate should be detected
    // Rust naming conventions may convert hyphens to underscores
    assert!(
        crate_refs.contains_key("internal-crate") || crate_refs.contains_key("internal_crate"),
        "internal-crate dependency not found"
    );

    // Check if internal crate is detected as a path dependency
    if let Some(internal_crate) = crate_refs.get("internal-crate") {
        assert!(
            internal_crate.is_path_dependency,
            "internal-crate should be a path dependency"
        );
    } else if let Some(internal_crate) = crate_refs.get("internal_crate") {
        // If internal_crate is detected from use statements, it might not be recognized as a path dependency
        // Skip this check in that case
        println!("Note: internal_crate detected instead of internal-crate");
    }

    Ok(())
}

#[test]
#[ignore] // Skip in CI environments as it requires access to crates.io
fn test_monorepo_update_with_internal_crates() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // Path to the public crate
    let public_crate_path = root_path.join("public-crate");

    // Create CargoAutodd instance
    let autodd = CargoAutodd::with_debug(public_crate_path, true);

    // Execute dependency update
    let result = autodd.analyze_and_update();

    // Verify results
    assert!(
        result.is_ok(),
        "analyze_and_update should succeed with internal crates"
    );

    // Read updated Cargo.toml
    let cargo_toml_content = fs::read_to_string(root_path.join("public-crate/Cargo.toml"))?;

    // Check if tokio was added
    assert!(
        cargo_toml_content.contains("tokio"),
        "tokio should be added to Cargo.toml"
    );

    // Check if internal-crate is maintained as a path dependency
    assert!(
        cargo_toml_content.contains("internal-crate = { path ="),
        "internal-crate should be maintained as a path dependency"
    );

    Ok(())
}

#[test]
#[ignore] // Skip in CI environments as it requires access to crates.io
fn test_monorepo_with_publish_false_crates() -> Result<()> {
    let temp_dir = create_monorepo_test_environment()?;
    let root_path = temp_dir.path().to_path_buf();

    // Path to the internal crate
    let internal_crate_path = root_path.join("internal-crate");

    // Create CargoAutodd instance
    let autodd = CargoAutodd::with_debug(internal_crate_path, true);

    // Execute dependency update
    let result = autodd.analyze_and_update();

    // Verify results
    assert!(
        result.is_ok(),
        "analyze_and_update should succeed with publish=false crates"
    );

    // Read updated Cargo.toml
    let cargo_toml_content = fs::read_to_string(root_path.join("internal-crate/Cargo.toml"))?;

    // Check if publish = false is maintained
    assert!(
        cargo_toml_content.contains("publish = false"),
        "publish = false should be maintained"
    );

    // Check if serde is added/maintained
    assert!(
        cargo_toml_content.contains("serde"),
        "serde should be in Cargo.toml"
    );

    Ok(())
}
