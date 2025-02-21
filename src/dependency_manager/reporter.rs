use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use semver::Version;
use toml_edit::{DocumentMut, Item};

use crate::models::CrateReference;
use crate::dependency_manager::updater::DependencyUpdater;

pub struct DependencyReporter {
    project_root: PathBuf,
    cargo_toml: PathBuf,
    updater: DependencyUpdater,
}

impl DependencyReporter {
    pub fn new(project_root: PathBuf) -> Self {
        let cargo_toml = project_root.join("Cargo.toml");
        let updater = DependencyUpdater::new(project_root.clone());
        Self {
            project_root,
            cargo_toml,
            updater,
        }
    }

    pub fn generate_dependency_report(
        &self,
        crate_refs: &HashMap<String, CrateReference>,
    ) -> Result<()> {
        let content = fs::read_to_string(&self.cargo_toml)?;
        let doc = content.parse::<DocumentMut>()?;

        println!("\nDependency Usage Report");
        println!("=====================\n");

        if let Some(Item::Table(deps)) = doc.get("dependencies") {
            for (name, dep) in deps.iter() {
                println!("📦 {}", name);

                if let Some(version) = self.updater.get_dependency_version(dep) {
                    println!("  Version: {}", version);

                    if let Ok(latest) = self.updater.get_latest_version(name) {
                        if let (Ok(current), Ok(latest_ver)) = (
                            Version::parse(&version.trim_start_matches('^')),
                            Version::parse(&latest.trim_start_matches('^')),
                        ) {
                            if latest_ver > current {
                                println!("  Update available: {} -> {}", version, latest);
                            }
                        }
                    }
                }

                if let Some(crate_ref) = crate_refs.get(name) {
                    println!("  Used in {} file(s)", crate_ref.usage_count());
                    println!("  Usage locations:");
                    for path in &crate_ref.used_in {
                        if let Some(relative) = path.strip_prefix(&self.project_root).ok() {
                            println!("    - {}", relative.display());
                        }
                    }
                } else {
                    println!("  ⚠️ Warning: No usage detected in the project");
                }
                println!();
            }
        }

        Ok(())
    }

    pub fn generate_security_report(&self) -> Result<()> {
        println!("\nDependency Security Report");
        println!("========================\n");

        let outdated = self.check_security()?;
        
        if outdated.is_empty() {
            println!("✅ All dependencies are up to date.");
            return Ok(());
        }

        println!("⚠️ The following dependencies have updates available:\n");

        for (name, version_info) in outdated {
            println!("📦 {}", name);
            println!("  Version update available: {}", version_info);
            println!("  Please check the changelog for security fixes.");
            println!();
        }

        println!("Note: For a complete security audit, please use:");
        println!("  cargo audit");
        println!("  https://github.com/rustsec/rustsec\n");

        Ok(())
    }

    fn check_security(&self) -> Result<Vec<(String, String)>> {
        let content = fs::read_to_string(&self.cargo_toml)?;
        let doc = content.parse::<DocumentMut>()?;
        let mut outdated = Vec::new();

        if let Some(Item::Table(deps)) = doc.get("dependencies") {
            for (name, dep) in deps.iter() {
                if let Some(version) = self.updater.get_dependency_version(dep) {
                    if let Ok(latest) = self.updater.get_latest_version(name) {
                        let current = Version::parse(&version.trim_start_matches('^'))
                            .unwrap_or_else(|_| Version::new(0, 0, 0));
                        let latest = Version::parse(&latest.trim_start_matches('^'))
                            .unwrap_or_else(|_| Version::new(0, 0, 0));

                        if latest > current {
                            outdated.push((name.to_string(), format!("{} -> {}", version, latest)));
                        }
                    }
                }
            }
        }

        Ok(outdated)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_environment() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#;
        let mut file = File::create(&cargo_toml)?;
        writeln!(file, "{}", content)?;
        
        Ok((temp_dir, cargo_toml))
    }

    #[test]
    fn test_generate_dependency_report() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());
        
        let mut crate_refs = HashMap::new();
        let mut serde_ref = CrateReference::new("serde".to_string());
        serde_ref.add_usage(temp_dir.path().join("src/main.rs"));
        crate_refs.insert("serde".to_string(), serde_ref);

        reporter.generate_dependency_report(&crate_refs)?;
        Ok(())
    }

    #[test]
    fn test_generate_security_report() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());
        reporter.generate_security_report()?;
        Ok(())
    }
} 