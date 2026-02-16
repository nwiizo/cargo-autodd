pub mod config;
pub mod dependency_manager;
pub mod models;
pub mod utils;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;
pub use config::Config;
use toml_edit::DocumentMut;

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

        // Read existing dependency names from Cargo.toml
        let mut existing_deps = HashSet::new();
        let mut existing_dev_deps = HashSet::new();
        let cargo_toml_path = self.project_root.join("Cargo.toml");
        if let Ok(content) = std::fs::read_to_string(&cargo_toml_path)
            && let Ok(doc) = content.parse::<DocumentMut>()
        {
            if let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) {
                for (name, _) in deps.iter() {
                    existing_deps.insert(name.to_string());
                }
            }
            if let Some(deps) = doc.get("dev-dependencies").and_then(|d| d.as_table()) {
                for (name, _) in deps.iter() {
                    existing_dev_deps.insert(name.to_string());
                }
            }
        }

        let (regular, dev): (Vec<_>, Vec<_>) = crate_refs
            .iter()
            .partition(|(_, crate_ref)| !crate_ref.is_dev_dependency);

        // All existing deps (both regular and dev) for dedup
        let all_existing: HashSet<_> = existing_deps.union(&existing_dev_deps).collect();

        // Categorize regular dependencies
        let (new_regular, tracked_regular): (Vec<_>, Vec<_>) =
            regular.into_iter().partition(|(name, _)| {
                let hyphenated = name.replace('_', "-");
                !all_existing.contains(name) && !all_existing.contains(&hyphenated)
            });

        // Categorize dev dependencies
        let (new_dev, tracked_dev): (Vec<_>, Vec<_>) = dev.into_iter().partition(|(name, _)| {
            let hyphenated = name.replace('_', "-");
            !all_existing.contains(name) && !all_existing.contains(&hyphenated)
        });

        // Would add (new deps only)
        if !new_regular.is_empty() {
            println!("\n[dependencies] would add:");
            for (name, crate_ref) in &new_regular {
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

        if !new_dev.is_empty() {
            println!("\n[dev-dependencies] would add:");
            for (name, crate_ref) in &new_dev {
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

        // Would remove (existing deps not found in analysis, excluding essential)
        let analyzed_names: HashSet<&str> = crate_refs.keys().map(|s| s.as_str()).collect();
        let would_remove: Vec<_> = existing_deps
            .iter()
            .filter(|name| {
                let normalized = name.replace('-', "_");
                !analyzed_names.contains(name.as_str())
                    && !analyzed_names.contains(normalized.as_str())
                    && !utils::is_essential_dep(name)
                    && !self.config.is_essential(name)
            })
            .collect();

        if !would_remove.is_empty() {
            println!("\n[dependencies] would remove:");
            for name in &would_remove {
                println!("  - {}", name);
            }
        }

        // Already tracked
        if !tracked_regular.is_empty() || !tracked_dev.is_empty() {
            println!("\nAlready tracked:");
            for (name, _) in &tracked_regular {
                println!("  ✓ {}", name);
            }
            for (name, _) in &tracked_dev {
                println!("  ✓ {} (dev)", name);
            }
        }

        // Show config exclusions
        if !self.config.exclude.is_empty() {
            println!("\nExcluded by config:");
            for name in &self.config.exclude {
                println!("  - {}", name);
            }
        }

        if new_regular.is_empty() && new_dev.is_empty() && would_remove.is_empty() {
            println!("\n✅ No changes needed (all dependencies are in sync)");
        } else {
            println!("\n✅ No changes were made (dry-run mode)");
        }
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
