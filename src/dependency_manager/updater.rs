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
use crate::utils::is_std_crate;

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

        // Get current dependencies
        let mut current_deps = HashSet::new();
        if let Some(Item::Table(deps)) = doc.get("dependencies") {
            for (key, _) in deps.iter() {
                current_deps.insert(key.to_string());
            }
        }

        // Add new dependencies
        for (name, crate_ref) in crate_refs {
            if !current_deps.contains(name) {
                self.add_dependency(&mut doc, crate_ref)?;
            }
        }

        // Remove unused dependencies
        let used_crates: HashSet<_> = crate_refs.keys().cloned().collect();
        let unused_deps: Vec<_> = current_deps
            .difference(&used_crates)
            .filter(|name| !is_essential_dep(name))
            .cloned()
            .collect();

        for name in unused_deps {
            self.remove_dependency(&mut doc, &name)?;
            println!("Removing unused dependency: {}", name);
        }

        fs::write(&self.cargo_toml, doc.to_string())?;
        Ok(())
    }

    fn add_dependency(&self, doc: &mut DocumentMut, crate_ref: &CrateReference) -> Result<()> {
        let version = self.get_latest_version(&crate_ref.name)?;

        let deps = doc
            .get_mut("dependencies")
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("Could not find dependencies table"))?;

        let mut dep_table = Table::new();
        dep_table.insert("version", toml_edit::value(version));

        if !crate_ref.features.is_empty() {
            let mut array = toml_edit::Array::new();
            for feature in &crate_ref.features {
                array.push(feature.as_str());
            }
            dep_table.insert(
                "features",
                toml_edit::Item::Value(toml_edit::Value::Array(array)),
            );
        }

        deps.insert(&crate_ref.name, Item::Table(dep_table));
        println!(
            "Added dependency: {} with features: {:?}",
            crate_ref.name, crate_ref.features
        );

        Ok(())
    }

    fn remove_dependency(&self, doc: &mut DocumentMut, name: &str) -> Result<()> {
        if let Some(Item::Table(deps)) = doc.get_mut("dependencies") {
            deps.remove(name);
        }
        Ok(())
    }

    pub fn get_latest_version(&self, crate_name: &str) -> Result<String> {
        // Skip standard library types and modules
        if is_std_crate(crate_name) {
            return Err(anyhow::anyhow!("Standard library type: {}", crate_name));
        }

        let url = format!("https://crates.io/api/v1/crates/{}/versions", crate_name);

        match ureq::get(&url).call() {
            Ok(response) => {
                let reader = BufReader::new(response.into_reader());
                let response: CratesIoResponse = serde_json::from_reader(reader)?;

                let latest_version =
                    response
                        .versions
                        .iter()
                        .find(|v| !v.yanked)
                        .ok_or_else(|| {
                            anyhow::anyhow!("No non-yanked version found for {}", crate_name)
                        })?;

                let version = Version::parse(&latest_version.num)?;
                Ok(format!("^{}.{}.0", version.major, version.minor))
            }
            Err(ureq::Error::Status(404, _)) => {
                if self.debug {
                    println!("Warning: Crate '{}' not found on crates.io", crate_name);
                }
                Err(anyhow::anyhow!(
                    "Crate '{}' not found on crates.io",
                    crate_name
                ))
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to fetch version for {}: {}",
                crate_name,
                e
            )),
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
}
