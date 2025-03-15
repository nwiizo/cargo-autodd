use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufReader;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;
use serde_json;
use toml_edit::{DocumentMut, Item, Table};
use ureq;

use crate::models::CrateReference;
use crate::utils::is_essential_dep;

#[derive(Deserialize)]
struct CratesIoResponse {
    versions: Vec<CrateVersion>,
}

#[derive(Deserialize)]
struct CrateVersion {
    num: String,
    yanked: bool,
}

pub struct DependencyUpdater {
    project_root: PathBuf,
    cargo_toml: PathBuf,
    debug: bool,
}

impl DependencyUpdater {
    pub fn new(project_root: PathBuf) -> Self {
        let cargo_toml = project_root.join("Cargo.toml");
        Self {
            project_root,
            cargo_toml,
            debug: false,
        }
    }

    pub fn update_cargo_toml(&self, crate_refs: &HashMap<String, CrateReference>) -> Result<()> {
        let content = fs::read_to_string(&self.cargo_toml)?;
        let mut doc = content.parse::<DocumentMut>()?;

        // Check if this is a workspace or a package
        let is_workspace = doc.get("workspace").is_some();
        if is_workspace && doc.get("package").is_none() {
            if self.debug {
                println!("This is a workspace root without a package. Skipping dependency update.");
            }
            return Ok(());
        }

        // Get the dependencies path
        let deps_path = self.get_dependencies_path()?;

        // Get existing dependencies
        let existing_deps = if let Some(deps) = doc.get(&deps_path) {
            if let Some(table) = deps.as_table() {
                table
                    .iter()
                    .map(|(k, _)| k.to_string())
                    .collect::<HashSet<_>>()
            } else {
                HashSet::new()
            }
        } else {
            HashSet::new()
        };

        // Add new dependencies
        for crate_ref in crate_refs.values() {
            if !existing_deps.contains(&crate_ref.name) {
                self.add_dependency(&mut doc, crate_ref, &deps_path)?;
            }
        }

        // Remove unused dependencies
        let used_deps = crate_refs.keys().cloned().collect::<HashSet<_>>();
        let to_remove = existing_deps
            .iter()
            .filter(|dep| !used_deps.contains(*dep) && !is_essential_dep(dep))
            .cloned()
            .collect::<Vec<_>>();

        for dep in to_remove {
            self.remove_dependency(&mut doc, &dep, &deps_path)?;
        }

        // Write back to Cargo.toml
        fs::write(&self.cargo_toml, doc.to_string())?;

        Ok(())
    }

    fn add_dependency(
        &self,
        doc: &mut DocumentMut,
        crate_ref: &CrateReference,
        deps_path: &str,
    ) -> Result<()> {
        // For internal crates (path dependencies), add without searching on crates.io
        if crate_ref.is_path_dependency {
            if let Some(path) = &crate_ref.path {
                if self.debug {
                    println!(
                        "Adding path dependency: {} with path {}",
                        crate_ref.name, path
                    );
                }

                // Get or create the dependencies table
                let deps = doc
                    .entry(deps_path)
                    .or_insert(toml_edit::table())
                    .as_table_mut()
                    .ok_or_else(|| anyhow::anyhow!("Failed to get dependencies table"))?;

                // Add internal crate as path dependency
                let mut table = Table::new();
                table["path"] = toml_edit::value(path.clone());

                // Add publish setting if available
                if let Some(publish) = crate_ref.publish {
                    table["publish"] = toml_edit::value(publish);
                }

                deps[&crate_ref.name] = toml_edit::Item::Table(table);
                return Ok(());
            }
        }

        // For regular dependencies, get the latest version from crates.io
        let version = match self.get_latest_version(&crate_ref.name) {
            Ok(v) => v,
            Err(e) => {
                // If not found on crates.io, it might be an internal crate, so continue with a warning
                if self.debug {
                    println!(
                        "Warning: Failed to get version for {}: {}",
                        crate_ref.name, e
                    );
                    println!("This might be an internal crate not published on crates.io.");
                    println!("Skipping this dependency.");
                }
                return Ok(());
            }
        };

        if self.debug {
            println!("Adding dependency: {} = \"{}\"", crate_ref.name, version);
        }

        // Get or create the dependencies table
        let deps = doc
            .entry(deps_path)
            .or_insert(toml_edit::table())
            .as_table_mut()
            .ok_or_else(|| anyhow::anyhow!("Failed to get dependencies table"))?;

        // Add the dependency
        deps[&crate_ref.name] = toml_edit::value(version);

        Ok(())
    }

    fn remove_dependency(&self, doc: &mut DocumentMut, name: &str, deps_path: &str) -> Result<()> {
        if deps_path.contains('.') {
            // Handle nested table path like "workspace.dependencies"
            let parts: Vec<&str> = deps_path.split('.').collect();
            if let Some(Item::Table(parent)) = doc.get_mut(parts[0]) {
                if let Some(Item::Table(deps)) = parent.get_mut(parts[1]) {
                    deps.remove(name);
                }
            }
        } else if let Some(Item::Table(deps)) = doc.get_mut(deps_path) {
            deps.remove(name);
        }
        Ok(())
    }

    pub fn get_latest_version(&self, crate_name: &str) -> Result<String> {
        // Return an error for internal crates
        if crate_name.contains('-') && crate_name.replace('-', "_") != crate_name {
            let normalized_name = crate_name.replace('-', "_");
            if self.debug {
                println!(
                    "Checking if {} is an internal crate (normalized: {})",
                    crate_name, normalized_name
                );
            }

            // Check if it's an internal crate by reading Cargo.toml
            let workspace_root = self.find_workspace_root()?;
            let workspace_cargo_toml = workspace_root.join("Cargo.toml");

            if workspace_cargo_toml.exists() {
                let content = fs::read_to_string(&workspace_cargo_toml)?;
                if content.contains(&format!("name = \"{}\"", crate_name))
                    || content.contains(&format!("name = \"{}\"", normalized_name))
                {
                    if self.debug {
                        println!(
                            "{} appears to be an internal crate in the workspace",
                            crate_name
                        );
                    }
                    return Err(anyhow::anyhow!("Internal crate not published on crates.io"));
                }
            }
        }

        // Get the latest version from crates.io
        let url = format!("https://crates.io/api/v1/crates/{}", crate_name);
        let response = ureq::get(&url).call();

        match response {
            Ok(res) => {
                let reader = BufReader::new(res.into_reader());
                let crates_io_data: CratesIoResponse = serde_json::from_reader(reader)?;

                // Find the latest non-yanked version
                let latest_version = crates_io_data
                    .versions
                    .iter()
                    .filter(|v| !v.yanked)
                    .map(|v| Version::parse(&v.num))
                    .filter_map(Result::ok)
                    .max();

                match latest_version {
                    Some(v) => {
                        // Include patch version for more accurate updates
                        Ok(format!("{}.{}.{}", v.major, v.minor, v.patch))
                    }
                    None => Err(anyhow::anyhow!(
                        "No valid versions found for {}",
                        crate_name
                    )),
                }
            }
            Err(e) => Err(anyhow::anyhow!("Failed to fetch crate info: {}", e)),
        }
    }

    /// Find the workspace root directory
    fn find_workspace_root(&self) -> Result<PathBuf> {
        let mut current_dir = self.project_root.clone();

        loop {
            let cargo_toml = current_dir.join("Cargo.toml");
            if cargo_toml.exists() {
                let content = fs::read_to_string(&cargo_toml)?;
                if content.contains("[workspace]") {
                    return Ok(current_dir);
                }
            }

            if !current_dir.pop() {
                // If we've reached the root directory, return the current project root
                return Ok(self.project_root.clone());
            }
        }
    }

    pub fn verify_dependencies(&self) -> Result<()> {
        Command::new("cargo")
            .current_dir(&self.project_root)
            .arg("check")
            .status()
            .context("Failed to run cargo check")?;
        Ok(())
    }

    pub fn get_dependency_version(&self, dep: &Item) -> Option<String> {
        match dep {
            Item::Value(v) => Some(v.as_str()?.to_string()),
            Item::Table(t) => t
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }
    }

    // New method to detect if the current Cargo.toml is a workspace
    pub fn is_workspace(&self) -> Result<bool> {
        let content = fs::read_to_string(&self.cargo_toml)?;
        let doc = content.parse::<DocumentMut>()?;
        Ok(doc.get("workspace").is_some())
    }

    // New method to get dependencies path
    pub fn get_dependencies_path(&self) -> Result<String> {
        if self.is_workspace()? {
            Ok("workspace.dependencies".to_string())
        } else {
            Ok("dependencies".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_cargo_toml(dir: &TempDir) -> PathBuf {
        let path = dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
"#;
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", content).unwrap();
        path
    }

    fn create_workspace_cargo_toml(dir: &TempDir) -> PathBuf {
        let path = dir.path().join("Cargo.toml");
        let content = r#"
[workspace]
members = ["crate1", "crate2"]

[package]
name = "workspace-root"
version = "0.1.0"
edition = "2021"

[workspace.dependencies]
serde = "1.0"
tokio = "1.0"
"#;
        let mut file = File::create(&path).unwrap();
        writeln!(file, "{}", content).unwrap();
        path
    }

    #[test]
    fn test_update_cargo_toml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_cargo_toml(&temp_dir);

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let mut crate_refs = HashMap::new();

        // Add a new dependency
        let mut new_crate = CrateReference::new("regex".to_string());
        new_crate.add_feature("unicode".to_string());
        crate_refs.insert("regex".to_string(), new_crate);

        // Add an existing dependency
        let serde_crate = CrateReference::new("serde".to_string());
        crate_refs.insert("serde".to_string(), serde_crate);

        updater.update_cargo_toml(&crate_refs)?;

        // Verify the changes
        let content = fs::read_to_string(updater.cargo_toml)?;
        assert!(content.contains("regex"));
        assert!(content.contains("serde"));
        assert!(!content.contains("unused-dep"));

        Ok(())
    }

    #[test]
    fn test_update_workspace_cargo_toml() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_workspace_cargo_toml(&temp_dir);

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let mut crate_refs = HashMap::new();

        // Add a new dependency
        let mut new_crate = CrateReference::new("regex".to_string());
        new_crate.add_feature("unicode".to_string());
        crate_refs.insert("regex".to_string(), new_crate);

        // Add an existing dependency
        let serde_crate = CrateReference::new("serde".to_string());
        crate_refs.insert("serde".to_string(), serde_crate);

        updater.update_cargo_toml(&crate_refs)?;

        // Verify the changes
        let content = fs::read_to_string(updater.cargo_toml)?;
        assert!(content.contains("regex"));
        assert!(content.contains("serde"));
        assert!(content.contains("[workspace.dependencies]"));

        Ok(())
    }

    #[test]
    fn test_is_workspace() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Test regular package
        create_cargo_toml(&temp_dir);
        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        assert!(!updater.is_workspace()?);

        // Test workspace
        create_workspace_cargo_toml(&temp_dir);
        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        assert!(updater.is_workspace()?);

        Ok(())
    }
}
