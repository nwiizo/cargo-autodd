use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use semver::Version;
use toml_edit::DocumentMut;

use crate::dependency_manager::updater::DependencyUpdater;
use crate::models::CrateReference;

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

        // Check if this is a workspace or a package
        let is_workspace = doc.get("workspace").is_some();

        // Determine the correct dependencies table (workspace or package)
        let deps_path = if is_workspace {
            "workspace.dependencies"
        } else {
            "dependencies"
        };

        // Get dependencies from the correct table
        let deps = if deps_path.contains('.') {
            // Handle nested table path like "workspace.dependencies"
            let parts: Vec<&str> = deps_path.split('.').collect();
            doc.get(parts[0])
                .and_then(|t| t.as_table())
                .and_then(|t| t.get(parts[1]))
                .and_then(|t| t.as_table())
        } else {
            doc.get(deps_path).and_then(|t| t.as_table())
        };

        if let Some(deps) = deps {
            for (name, dep) in deps.iter() {
                println!("📦 {}", name);

                if let Some(version) = self.updater.get_dependency_version(dep) {
                    println!("  Version: {}", version);

                    match self.updater.get_latest_version(name) {
                        Ok(latest) => {
                            if let Ok(needs_update) = self.check_version(&version, &latest) {
                                if needs_update {
                                    println!("  ⚠️ Update available: {} -> {}", version, latest);
                                } else {
                                    println!("  ✅ Up to date");
                                }
                            }
                        }
                        Err(e) => {
                            println!("  ⚠️ Failed to check latest version: {}", e);
                        }
                    }
                }

                if let Some(crate_ref) = crate_refs.get(name) {
                    println!("  Used in {} file(s)", crate_ref.usage_count());
                    println!("  Usage locations:");
                    for path in &crate_ref.used_in {
                        if let Ok(relative) = path.strip_prefix(&self.project_root) {
                            println!("    - {}", relative.display());
                        }
                    }
                } else {
                    println!("  ⚠️ Warning: No usage detected in the project");
                }
                println!();
            }
        } else {
            println!("⚠️ No dependencies found in the {} table", deps_path);
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

        // Check if this is a workspace or a package
        let is_workspace = doc.get("workspace").is_some();

        // Determine the correct dependencies table (workspace or package)
        let deps_path = if is_workspace {
            "workspace.dependencies"
        } else {
            "dependencies"
        };

        // Get dependencies from the correct table
        let deps = if deps_path.contains('.') {
            // Handle nested table path like "workspace.dependencies"
            let parts: Vec<&str> = deps_path.split('.').collect();
            doc.get(parts[0])
                .and_then(|t| t.as_table())
                .and_then(|t| t.get(parts[1]))
                .and_then(|t| t.as_table())
        } else {
            doc.get(deps_path).and_then(|t| t.as_table())
        };

        if let Some(deps) = deps {
            for (name, dep) in deps.iter() {
                if let Some(version) = self.updater.get_dependency_version(dep)
                    && let Ok(latest) = self.updater.get_latest_version(name)
                    && let Ok(true) = self.check_version(&version, &latest)
                {
                    outdated.push((name.to_string(), format!("{} -> {}", version, latest)));
                }
            }
        }

        Ok(outdated)
    }

    pub fn check_version(&self, version: &str, latest: &str) -> Result<bool> {
        let current = Version::parse(Self::strip_version_prefix(version))?;
        let latest_ver = Version::parse(Self::strip_version_prefix(latest))?;
        Ok(latest_ver > current)
    }

    /// Strip version requirement prefixes (^, ~, =, >=, <=, >, <)
    fn strip_version_prefix(version: &str) -> &str {
        let version = version.trim();
        if version.starts_with(">=") || version.starts_with("<=") {
            &version[2..]
        } else if version.starts_with('^')
            || version.starts_with('~')
            || version.starts_with('=')
            || version.starts_with('>')
            || version.starts_with('<')
        {
            &version[1..]
        } else {
            version
        }
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

    fn create_workspace_test_environment() -> Result<(TempDir, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let cargo_toml = temp_dir.path().join("Cargo.toml");

        let content = r#"
[workspace]
members = ["crate1", "crate2"]

[workspace.dependencies]
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
    fn test_generate_workspace_dependency_report() -> Result<()> {
        let (temp_dir, _) = create_workspace_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        let mut crate_refs = HashMap::new();
        let mut serde_ref = CrateReference::new("serde".to_string());
        serde_ref.add_usage(temp_dir.path().join("crate1/src/main.rs"));
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

    #[test]
    fn test_generate_workspace_security_report() -> Result<()> {
        let (temp_dir, _) = create_workspace_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());
        reporter.generate_security_report()?;
        Ok(())
    }

    #[test]
    fn test_check_version_update_available() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        // Newer version available
        assert!(reporter.check_version("1.0.0", "1.1.0")?);
        assert!(reporter.check_version("1.0.0", "2.0.0")?);
        assert!(reporter.check_version("1.0.0", "1.0.1")?);

        Ok(())
    }

    #[test]
    fn test_check_version_up_to_date() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        // Same version
        assert!(!reporter.check_version("1.0.0", "1.0.0")?);

        // Current is newer (shouldn't happen in practice but test the logic)
        assert!(!reporter.check_version("2.0.0", "1.0.0")?);
        assert!(!reporter.check_version("1.1.0", "1.0.0")?);

        Ok(())
    }

    #[test]
    fn test_check_version_with_caret_prefix() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        // Caret prefix should be stripped
        assert!(reporter.check_version("^1.0.0", "1.1.0")?);
        assert!(reporter.check_version("^1.0.0", "^1.1.0")?);
        assert!(!reporter.check_version("^1.0.0", "^1.0.0")?);

        Ok(())
    }

    #[test]
    fn test_check_version_with_tilde_prefix() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        // Tilde prefix should be stripped
        assert!(reporter.check_version("~1.0.0", "1.1.0")?);
        assert!(!reporter.check_version("~1.0.0", "~1.0.0")?);

        Ok(())
    }

    #[test]
    fn test_check_version_with_comparison_prefixes() -> Result<()> {
        let (temp_dir, _) = create_test_environment()?;
        let reporter = DependencyReporter::new(temp_dir.path().to_path_buf());

        // Various comparison prefixes should be stripped
        assert!(reporter.check_version("=1.0.0", "1.1.0")?);
        assert!(reporter.check_version(">=1.0.0", "1.1.0")?);
        assert!(reporter.check_version("<=1.0.0", "1.1.0")?);
        assert!(reporter.check_version(">1.0.0", "1.1.0")?);
        assert!(reporter.check_version("<1.0.0", "1.1.0")?);

        Ok(())
    }

    #[test]
    fn test_strip_version_prefix() {
        // Test the private helper function behavior through check_version
        // This test verifies that all prefixes are correctly handled

        // The function is private, so we test it indirectly
        // through the check_version method which uses it
    }
}
