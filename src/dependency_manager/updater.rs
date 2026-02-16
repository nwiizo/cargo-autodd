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

pub(crate) trait CrateRegistry {
    fn get_latest_version(&self, crate_name: &str) -> Result<Option<String>>;
}

pub(crate) struct CratesIoRegistry;

impl CrateRegistry for CratesIoRegistry {
    fn get_latest_version(&self, crate_name: &str) -> Result<Option<String>> {
        let url = format!("https://crates.io/api/v1/crates/{}", crate_name);
        let response = ureq::get(&url).call();

        match response {
            Ok(res) => {
                let reader = BufReader::new(res.into_reader());
                let crates_io_data: CratesIoResponse = serde_json::from_reader(reader)?;

                let latest_version = crates_io_data
                    .versions
                    .iter()
                    .filter(|v| !v.yanked)
                    .map(|v| Version::parse(&v.num))
                    .filter_map(Result::ok)
                    .max();

                match latest_version {
                    Some(v) => Ok(Some(format!("{}.{}.{}", v.major, v.minor, v.patch))),
                    None => Ok(None),
                }
            }
            Err(ureq::Error::Status(404, _)) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Failed to fetch crate info: {}", e)),
        }
    }
}

pub struct DependencyUpdater {
    project_root: PathBuf,
    cargo_toml: PathBuf,
    debug: bool,
    registry: Box<dyn CrateRegistry>,
}

impl DependencyUpdater {
    pub fn new(project_root: PathBuf) -> Self {
        let cargo_toml = project_root.join("Cargo.toml");
        Self {
            project_root,
            cargo_toml,
            debug: false,
            registry: Box::new(CratesIoRegistry),
        }
    }

    pub fn with_debug(project_root: PathBuf, debug: bool) -> Self {
        let cargo_toml = project_root.join("Cargo.toml");
        Self {
            project_root,
            cargo_toml,
            debug,
            registry: Box::new(CratesIoRegistry),
        }
    }

    #[cfg(test)]
    fn with_registry(project_root: PathBuf, registry: Box<dyn CrateRegistry>) -> Self {
        let cargo_toml = project_root.join("Cargo.toml");
        Self {
            project_root,
            cargo_toml,
            debug: false,
            registry,
        }
    }

    pub fn update_cargo_toml(&self, crate_refs: &HashMap<String, CrateReference>) -> Result<()> {
        let content = fs::read_to_string(&self.cargo_toml)
            .with_context(|| format!("Failed to read {}", self.cargo_toml.display()))?;
        let mut doc = content
            .parse::<DocumentMut>()
            .with_context(|| format!("Failed to parse {} as TOML", self.cargo_toml.display()))?;

        // Check if this is a workspace or a package
        let is_workspace = doc.get("workspace").is_some();
        if is_workspace && doc.get("package").is_none() {
            if self.debug {
                println!("This is a workspace root without a package. Skipping dependency update.");
            }
            return Ok(());
        }

        // Separate regular dependencies and dev-dependencies
        let (regular_deps, dev_deps): (HashMap<_, _>, HashMap<_, _>) = crate_refs
            .iter()
            .partition(|(_, crate_ref)| !crate_ref.is_dev_dependency);

        // Get the dependencies path
        let deps_path = self.get_dependencies_path()?;
        let dev_deps_path = "dev-dependencies".to_string();

        // Update regular dependencies
        self.update_dependency_section(&mut doc, &regular_deps, &deps_path)?;

        // Update dev-dependencies (only if not a workspace with shared deps)
        if !is_workspace {
            self.update_dependency_section(&mut doc, &dev_deps, &dev_deps_path)?;
        }

        // Write back to Cargo.toml
        fs::write(&self.cargo_toml, doc.to_string())?;

        Ok(())
    }

    fn update_dependency_section(
        &self,
        doc: &mut DocumentMut,
        deps_map: &HashMap<&String, &CrateReference>,
        deps_path: &str,
    ) -> Result<()> {
        // Get existing dependencies
        let existing_deps = if let Some(deps) = doc.get(deps_path) {
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
        for crate_ref in deps_map.values() {
            // Check if dependency exists by comparing normalized names (hyphens → underscores)
            let exists = existing_deps
                .iter()
                .any(|dep| dep.replace('-', "_") == crate_ref.name);
            if !exists {
                self.add_dependency(doc, crate_ref, deps_path)?;
            }
        }

        // Remove unused dependencies
        let used_deps = deps_map
            .keys()
            .map(|k| (*k).clone())
            .collect::<HashSet<_>>();
        let to_remove = existing_deps
            .iter()
            .filter(|dep| {
                let normalized = dep.replace('-', "_");
                !used_deps.contains(&normalized) && !is_essential_dep(dep)
            })
            .cloned()
            .collect::<Vec<_>>();

        for dep in to_remove {
            self.remove_dependency(doc, &dep, deps_path)?;
        }

        Ok(())
    }

    fn add_dependency(
        &self,
        doc: &mut DocumentMut,
        crate_ref: &CrateReference,
        deps_path: &str,
    ) -> Result<()> {
        // For internal crates (path dependencies), add without searching on crates.io
        if crate_ref.is_path_dependency
            && let Some(path) = &crate_ref.path
        {
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
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "Failed to get {} table in {}",
                        deps_path,
                        self.cargo_toml.display()
                    )
                })?;

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
            if let Some(Item::Table(parent)) = doc.get_mut(parts[0])
                && let Some(Item::Table(deps)) = parent.get_mut(parts[1])
            {
                deps.remove(name);
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

        match self.registry.get_latest_version(crate_name)? {
            Some(v) => Ok(v),
            None => Err(anyhow::anyhow!(
                "No valid versions found for {}",
                crate_name
            )),
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
        let status = Command::new("cargo")
            .current_dir(&self.project_root)
            .arg("check")
            .status()
            .context("Failed to run cargo check")?;
        if !status.success() {
            anyhow::bail!(
                "cargo check failed with exit code: {}",
                status.code().unwrap_or(-1)
            );
        }
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
        let content = fs::read_to_string(&self.cargo_toml)
            .with_context(|| format!("Failed to read {}", self.cargo_toml.display()))?;
        let doc = content
            .parse::<DocumentMut>()
            .with_context(|| format!("Failed to parse {} as TOML", self.cargo_toml.display()))?;
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

    #[test]
    fn test_remove_unused_dependency() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create Cargo.toml with multiple dependencies
        let path = temp_dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
unused_crate = "0.1"
another_unused = "0.2"
"#;
        let mut file = File::create(&path)?;
        writeln!(file, "{}", content)?;

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let mut crate_refs = HashMap::new();

        // Only serde and tokio are used
        crate_refs.insert(
            "serde".to_string(),
            CrateReference::new("serde".to_string()),
        );
        crate_refs.insert(
            "tokio".to_string(),
            CrateReference::new("tokio".to_string()),
        );

        updater.update_cargo_toml(&crate_refs)?;

        // Verify unused dependencies are removed
        let result = fs::read_to_string(&path)?;
        assert!(result.contains("serde"), "serde should remain");
        assert!(result.contains("tokio"), "tokio should remain");
        assert!(
            !result.contains("unused_crate"),
            "unused_crate should be removed"
        );
        assert!(
            !result.contains("another_unused"),
            "another_unused should be removed"
        );

        Ok(())
    }

    #[test]
    fn test_preserve_essential_dependencies() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create Cargo.toml with essential dependencies
        let path = temp_dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
tokio = "1.0"
anyhow = "1.0"
thiserror = "1.0"
unused_crate = "0.1"
"#;
        let mut file = File::create(&path)?;
        writeln!(file, "{}", content)?;

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());

        // Empty crate_refs - nothing is used
        let crate_refs = HashMap::new();

        updater.update_cargo_toml(&crate_refs)?;

        // Verify essential dependencies are preserved even if not used
        let result = fs::read_to_string(&path)?;
        assert!(
            result.contains("serde"),
            "serde (essential) should be preserved"
        );
        assert!(
            result.contains("tokio"),
            "tokio (essential) should be preserved"
        );
        assert!(
            result.contains("anyhow"),
            "anyhow (essential) should be preserved"
        );
        assert!(
            result.contains("thiserror"),
            "thiserror (essential) should be preserved"
        );
        assert!(
            !result.contains("unused_crate"),
            "non-essential unused_crate should be removed"
        );

        Ok(())
    }

    #[test]
    fn test_get_dependency_version() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_cargo_toml(&temp_dir);

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());

        // Test simple version string
        let simple_version = toml_edit::value("1.0.0");
        assert_eq!(
            updater.get_dependency_version(&simple_version),
            Some("1.0.0".to_string())
        );

        // Test table with version
        let mut table = toml_edit::Table::new();
        table["version"] = toml_edit::value("2.0.0");
        let table_version = toml_edit::Item::Table(table);
        assert_eq!(
            updater.get_dependency_version(&table_version),
            Some("2.0.0".to_string())
        );

        Ok(())
    }

    #[test]
    fn test_verify_dependencies_missing_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let result = updater.verify_dependencies();
        assert!(
            result.is_err(),
            "verify_dependencies should fail without Cargo.toml"
        );
    }

    #[test]
    fn test_verify_dependencies_valid_project() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create a minimal valid Rust project
        let cargo_toml = temp_dir.path().join("Cargo.toml");
        let content = r#"[package]
name = "test-verify"
version = "0.1.0"
edition = "2021"
"#;
        let mut file = File::create(&cargo_toml)?;
        write!(file, "{}", content)?;

        std::fs::create_dir(temp_dir.path().join("src"))?;
        let main_rs = temp_dir.path().join("src/main.rs");
        let mut file = File::create(main_rs)?;
        write!(file, "fn main() {{}}")?;

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        updater.verify_dependencies()?;
        Ok(())
    }

    #[test]
    fn test_find_workspace_root_no_workspace() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_cargo_toml(&temp_dir);

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let root = updater.find_workspace_root()?;
        assert_eq!(root, temp_dir.path().to_path_buf());
        Ok(())
    }

    #[test]
    fn test_find_workspace_root_with_workspace() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_workspace_cargo_toml(&temp_dir);

        // Create a sub-crate directory
        let sub_crate_dir = temp_dir.path().join("crate1");
        std::fs::create_dir(&sub_crate_dir)?;
        let sub_cargo_toml = sub_crate_dir.join("Cargo.toml");
        let content = r#"[package]
name = "crate1"
version = "0.1.0"
edition = "2021"
"#;
        let mut file = File::create(sub_cargo_toml)?;
        write!(file, "{}", content)?;

        let updater = DependencyUpdater::new(sub_crate_dir.clone());
        let root = updater.find_workspace_root()?;
        assert_eq!(root, temp_dir.path().to_path_buf());
        Ok(())
    }

    #[test]
    fn test_hyphenated_crate_not_duplicated() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Cargo.toml has hyphenated name "serde-json"
        let path = temp_dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
serde-json = "1.0"
"#;
        let mut file = File::create(&path)?;
        writeln!(file, "{}", content)?;

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let mut crate_refs = HashMap::new();

        // Analyzer detects "serde_json" (underscore form from Rust code)
        crate_refs.insert(
            "serde".to_string(),
            CrateReference::new("serde".to_string()),
        );
        crate_refs.insert(
            "serde_json".to_string(),
            CrateReference::new("serde_json".to_string()),
        );

        updater.update_cargo_toml(&crate_refs)?;

        let result = fs::read_to_string(&path)?;
        // serde-json should NOT be duplicated as serde_json
        let serde_json_count =
            result.matches("serde-json").count() + result.matches("serde_json").count();
        assert_eq!(
            serde_json_count, 1,
            "serde-json should appear exactly once, not be duplicated: {}",
            result
        );
        Ok(())
    }

    #[test]
    fn test_hyphenated_crate_not_removed_when_used() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Cargo.toml has hyphenated name "serde-json"
        let path = temp_dir.path().join("Cargo.toml");
        let content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"

[dependencies]
serde = "1.0"
serde-json = "1.0"
my-utils = "0.5"
"#;
        let mut file = File::create(&path)?;
        writeln!(file, "{}", content)?;

        let updater = DependencyUpdater::new(temp_dir.path().to_path_buf());
        let mut crate_refs = HashMap::new();

        // Analyzer detects underscore forms from Rust code
        crate_refs.insert(
            "serde".to_string(),
            CrateReference::new("serde".to_string()),
        );
        crate_refs.insert(
            "serde_json".to_string(),
            CrateReference::new("serde_json".to_string()),
        );
        // my-utils is not used

        updater.update_cargo_toml(&crate_refs)?;

        let result = fs::read_to_string(&path)?;
        assert!(
            result.contains("serde-json"),
            "serde-json should be preserved (used as serde_json)"
        );
        assert!(
            !result.contains("my-utils") && !result.contains("my_utils"),
            "my-utils should be removed (unused)"
        );
        Ok(())
    }

    struct MockRegistry {
        versions: HashMap<String, String>,
    }

    impl CrateRegistry for MockRegistry {
        fn get_latest_version(&self, crate_name: &str) -> Result<Option<String>> {
            Ok(self.versions.get(crate_name).cloned())
        }
    }

    #[test]
    fn test_mock_registry_get_latest_version_success() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_cargo_toml(&temp_dir);

        let mut versions = HashMap::new();
        versions.insert("regex".to_string(), "1.10.0".to_string());
        let registry = Box::new(MockRegistry { versions });

        let updater = DependencyUpdater::with_registry(temp_dir.path().to_path_buf(), registry);
        let version = updater.get_latest_version("regex")?;
        assert_eq!(version, "1.10.0");
        Ok(())
    }

    #[test]
    fn test_mock_registry_get_latest_version_not_found() {
        let temp_dir = TempDir::new().unwrap();
        create_cargo_toml(&temp_dir);

        let versions = HashMap::new();
        let registry = Box::new(MockRegistry { versions });

        let updater = DependencyUpdater::with_registry(temp_dir.path().to_path_buf(), registry);
        let result = updater.get_latest_version("nonexistent_crate");
        assert!(
            result.is_err(),
            "Should return error for crate not found in registry"
        );
    }

    #[test]
    fn test_mock_registry_update_cargo_toml_adds_dep() -> Result<()> {
        let temp_dir = TempDir::new()?;
        create_cargo_toml(&temp_dir);

        let mut versions = HashMap::new();
        versions.insert("regex".to_string(), "1.10.0".to_string());
        let registry = Box::new(MockRegistry { versions });

        let updater = DependencyUpdater::with_registry(temp_dir.path().to_path_buf(), registry);

        let mut crate_refs = HashMap::new();
        crate_refs.insert(
            "serde".to_string(),
            CrateReference::new("serde".to_string()),
        );
        let mut regex_ref = CrateReference::new("regex".to_string());
        regex_ref.add_feature("unicode".to_string());
        crate_refs.insert("regex".to_string(), regex_ref);

        updater.update_cargo_toml(&crate_refs)?;

        let content = fs::read_to_string(updater.cargo_toml)?;
        assert!(content.contains("regex"), "regex should be added");
        assert!(
            content.contains("1.10.0"),
            "regex version should be 1.10.0 from mock"
        );
        Ok(())
    }
}
