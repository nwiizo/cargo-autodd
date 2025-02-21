use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use cargo_autodd::CargoAutodd;
use tempfile::TempDir;

fn create_test_project() -> Result<TempDir> {
    let temp_dir = TempDir::new()?;

    // Create project structure
    fs::create_dir(temp_dir.path().join("src"))?;

    // Create Cargo.toml
    let cargo_toml = temp_dir.path().join("Cargo.toml");
    let content = r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#;
    let mut file = File::create(cargo_toml)?;
    writeln!(file, "{}", content)?;

    // Create source files
    create_test_file(
        &temp_dir.path().join("src/main.rs"),
        r#"
use serde;
use regex;
use tokio;
"#,
    )?;

    create_test_file(
        &temp_dir.path().join("src/lib.rs"),
        r#"
use serde;
use anyhow;
"#,
    )?;

    Ok(temp_dir)
}

fn create_test_file(path: &Path, content: &str) -> Result<()> {
    let mut file = File::create(path)?;
    writeln!(file, "{}", content)?;
    Ok(())
}

#[test]
fn test_analyze_and_update_adds_new_dependencies() -> Result<()> {
    let temp_dir = create_test_project()?;
    let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());

    // Run the analysis
    autodd.analyze_and_update()?;

    // Check if Cargo.toml was updated
    let cargo_toml = fs::read_to_string(temp_dir.path().join("Cargo.toml"))?;
    assert!(cargo_toml.contains("regex"));
    assert!(cargo_toml.contains("anyhow"));

    Ok(())
}

#[test]
fn test_report_generation() -> Result<()> {
    let temp_dir = create_test_project()?;
    let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());

    // Generate report
    autodd.generate_report()?;
    Ok(())
}

#[test]
fn test_security_check() -> Result<()> {
    let temp_dir = create_test_project()?;
    let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());

    // Run security check
    autodd.check_security()?;
    Ok(())
}

#[test]
fn test_update_dependencies() -> Result<()> {
    let temp_dir = create_test_project()?;
    let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());

    // Update dependencies
    autodd.update_dependencies()?;
    Ok(())
}
