pub mod config;
pub mod dependency_manager;
pub mod models;
pub mod utils;

use std::path::PathBuf;

use anyhow::Result;
pub use config::Config;

pub struct CargoAutodd {
    #[allow(dead_code)]
    project_root: PathBuf,
    analyzer: dependency_manager::DependencyAnalyzer,
    updater: dependency_manager::DependencyUpdater,
    reporter: dependency_manager::DependencyReporter,
    config: Config,
    debug: bool,
    dry_run: bool,
}

impl CargoAutodd {
    pub fn new(project_root: PathBuf) -> Result<Self> {
        let config = Config::load_default(&project_root)?;
        Ok(Self {
            project_root: project_root.clone(),
            analyzer: dependency_manager::DependencyAnalyzer::new(project_root.clone()),
            updater: dependency_manager::DependencyUpdater::new(project_root.clone()),
            reporter: dependency_manager::DependencyReporter::new(project_root),
            config,
            debug: false,
            dry_run: false,
        })
    }

    pub fn with_debug(project_root: PathBuf, debug: bool) -> Result<Self> {
        let config = Config::load_default(&project_root)?;
        Ok(Self {
            project_root: project_root.clone(),
            analyzer: dependency_manager::DependencyAnalyzer::with_debug(
                project_root.clone(),
                debug,
            ),
            updater: dependency_manager::DependencyUpdater::new(project_root.clone()),
            reporter: dependency_manager::DependencyReporter::new(project_root),
            config,
            debug,
            dry_run: false,
        })
    }

    pub fn with_options(project_root: PathBuf, debug: bool, dry_run: bool, config: Config) -> Self {
        Self {
            project_root: project_root.clone(),
            analyzer: dependency_manager::DependencyAnalyzer::with_debug(
                project_root.clone(),
                debug,
            ),
            updater: dependency_manager::DependencyUpdater::new(project_root.clone()),
            reporter: dependency_manager::DependencyReporter::new(project_root),
            config,
            debug,
            dry_run,
        }
    }

    pub fn analyze_and_update(&self) -> Result<()> {
        if self.debug {
            println!("🔍 Starting dependency analysis in debug mode...");
        }
        if self.dry_run {
            println!("🔍 Running in dry-run mode (no changes will be made)...");
        }

        println!("🔍 Analyzing project dependencies...");
        let mut crate_refs = self.analyzer.analyze_dependencies()?;

        // Apply config exclusions
        crate_refs.retain(|name, _| !self.config.should_exclude(name));

        if self.dry_run {
            self.print_dry_run_summary(&crate_refs)?;
            return Ok(());
        }

        if self.debug {
            println!("\n📝 Updating Cargo.toml with found dependencies...");
        }
        println!("📝 Updating Cargo.toml...");
        self.updater.update_cargo_toml(&crate_refs)?;

        println!("✅ Dependencies updated successfully!");
        Ok(())
    }

    fn print_dry_run_summary(
        &self,
        crate_refs: &std::collections::HashMap<String, models::CrateReference>,
    ) -> Result<()> {
        println!("\n📋 Dry-run summary:");
        println!("==================");

        let (regular, dev): (Vec<_>, Vec<_>) = crate_refs
            .iter()
            .partition(|(_, crate_ref)| !crate_ref.is_dev_dependency);

        if !regular.is_empty() {
            println!("\n[dependencies] would add:");
            for (name, crate_ref) in regular {
                if crate_ref.is_path_dependency {
                    println!(
                        "  {} = {{ path = \"{}\" }}",
                        name,
                        crate_ref.path.as_ref().unwrap_or(&"?".to_string())
                    );
                } else {
                    println!("  {} = \"<latest>\"", name);
                }
            }
        }

        if !dev.is_empty() {
            println!("\n[dev-dependencies] would add:");
            for (name, crate_ref) in dev {
                if crate_ref.is_path_dependency {
                    println!(
                        "  {} = {{ path = \"{}\" }}",
                        name,
                        crate_ref.path.as_ref().unwrap_or(&"?".to_string())
                    );
                } else {
                    println!("  {} = \"<latest>\"", name);
                }
            }
        }

        // Show config exclusions
        if !self.config.exclude.is_empty() {
            println!("\nExcluded by config:");
            for name in &self.config.exclude {
                println!("  - {}", name);
            }
        }

        println!("\n✅ No changes were made (dry-run mode)");
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
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf())?;
        autodd.analyze_and_update()?;
        Ok(())
    }

    #[test]
    fn test_generate_report() -> Result<()> {
        let temp_dir = create_test_environment()?;
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf())?;
        autodd.generate_report()?;
        Ok(())
    }

    #[test]
    fn test_check_security() -> Result<()> {
        let temp_dir = create_test_environment()?;
        let autodd = CargoAutodd::new(temp_dir.path().to_path_buf())?;
        autodd.check_security()?;
        Ok(())
    }

    #[test]
    fn test_dry_run_does_not_modify_cargo_toml() -> Result<()> {
        let temp_dir = create_test_environment()?;

        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let before = std::fs::read_to_string(&cargo_toml_path)?;

        let config = Config::default();
        let autodd = CargoAutodd::with_options(temp_dir.path().to_path_buf(), false, true, config);
        autodd.analyze_and_update()?;

        let after = std::fs::read_to_string(&cargo_toml_path)?;
        assert_eq!(
            before, after,
            "Cargo.toml should not be modified in dry-run mode"
        );
        Ok(())
    }
}
