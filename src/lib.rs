pub mod dependency_manager;
pub mod models;
pub mod utils;

use std::path::PathBuf;

use anyhow::Result;

pub struct CargoAutodd {
    analyzer: dependency_manager::DependencyAnalyzer,
    updater: dependency_manager::DependencyUpdater,
    reporter: dependency_manager::DependencyReporter,
    debug: bool,
}

impl CargoAutodd {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            analyzer: dependency_manager::DependencyAnalyzer::new(project_root.clone()),
            updater: dependency_manager::DependencyUpdater::new(project_root.clone()),
            reporter: dependency_manager::DependencyReporter::new(project_root),
            debug: false,
        }
    }

    pub fn with_debug(project_root: PathBuf, debug: bool) -> Self {
        Self {
            analyzer: dependency_manager::DependencyAnalyzer::with_debug(project_root.clone(), debug),
            updater: dependency_manager::DependencyUpdater::new(project_root.clone()),
            reporter: dependency_manager::DependencyReporter::new(project_root),
            debug,
        }
    }

    pub fn analyze_and_update(&self) -> Result<()> {
        if self.debug {
            println!("🔍 Starting dependency analysis in debug mode...");
        }
        println!("🔍 Analyzing project dependencies...");
        let crate_refs = self.analyzer.analyze_dependencies()?;
        
        if self.debug {
            println!("\n📝 Updating Cargo.toml with found dependencies...");
        }
        println!("📝 Updating Cargo.toml...");
        self.updater.update_cargo_toml(&crate_refs)?;
        
        println!("✅ Dependencies updated successfully!");
        Ok(())
    }

    pub fn update_dependencies(&self) -> Result<()> {
        println!("🔍 Checking for dependency updates...");
        let crate_refs = self.analyzer.analyze_dependencies()?;
        self.updater.update_cargo_toml(&crate_refs)?;
        println!("\n🔍 Verifying dependencies...");
        self.updater.verify_dependencies()?;
        println!("✅ Dependencies updated successfully!");
        Ok(())
    }

    pub fn generate_report(&self) -> Result<()> {
        println!("📊 Analyzing dependency usage...");
        let crate_refs = self.analyzer.analyze_dependencies()?;
        self.reporter.generate_dependency_report(&crate_refs)
    }

    pub fn check_security(&self) -> Result<()> {
        println!("🔒 Running security check...");
        self.reporter.generate_security_report()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_environment() -> Result<TempDir> {
        let temp_dir = TempDir::new()?;
        
        // Create Cargo.toml
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
"#;
        let mut file = File::create(&cargo_toml)?;
        writeln!(file, "{}", content)?;

        // Create src directory and main.rs
        std::fs::create_dir(temp_dir.path().join("src"))?;
        let main_rs = temp_dir.path().join("src/main.rs");
        let content = r#"
use serde;
use tokio;
"#;
        let mut file = File::create(main_rs)?;
        writeln!(file, "{}", content)?;

        Ok(temp_dir)
    }

    #[test]
    fn test_analyze_and_update() -> Result<()> {
        let temp_dir = create_test_environment()?;
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());
        autodd.analyze_and_update()?;
        Ok(())
    }

    #[test]
    fn test_generate_report() -> Result<()> {
        let temp_dir = create_test_environment()?;
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());
        autodd.generate_report()?;
        Ok(())
    }

    #[test]
    fn test_check_security() -> Result<()> {
        let temp_dir = create_test_environment()?;
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf());
        autodd.check_security()?;
        Ok(())
    }
} 