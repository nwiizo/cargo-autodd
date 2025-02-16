use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use regex::Regex;
use semver::Version;
use serde::Deserialize;
use toml_edit::{DocumentMut, Item, Table};

#[derive(Debug, Clone)]
struct CrateReference {
    name: String,
    features: HashSet<String>,
    used_in: HashSet<PathBuf>,
}

#[derive(Deserialize)]
struct CratesIoResponse {
    versions: Vec<CrateVersion>,
}

#[derive(Deserialize)]
struct CrateVersion {
    num: String,
    yanked: bool,
}

struct DependencyManager {
    project_root: PathBuf,
    cargo_toml: PathBuf,
}

impl DependencyManager {
    fn new(project_root: PathBuf) -> Result<Self> {
        let cargo_toml = project_root.join("Cargo.toml");
        Ok(Self {
            project_root,
            cargo_toml,
        })
    }

    fn analyze_dependencies(&self) -> Result<HashMap<String, CrateReference>> {
        let mut crate_refs = HashMap::new();
        let use_regex = Regex::new(r"use\s+([a-zA-Z_][a-zA-Z0-9_]*)(::|\s|;)")?;
        let extern_regex = Regex::new(r"extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)")?;

        // rust-analyzer CLIを使用してプロジェクトを解析
        let output = Command::new("rust-analyzer")
            .arg("analysis")
            .arg("--workspace")
            .current_dir(&self.project_root)
            .output()
            .context("Failed to run rust-analyzer. Is it installed?")?;

        if !output.status.success() {
            println!("Warning: rust-analyzer analysis returned non-zero status. Falling back to regex-based analysis.");
        }

        // プロジェクト内のすべてのRustファイルを走査
        for dir_entry in walkdir::WalkDir::new(&self.project_root)
            .into_iter()
            .filter_entry(|e| !is_hidden(e.path()))
        {
            let dir_entry = dir_entry?;
            if !dir_entry.path().to_string_lossy().ends_with(".rs") {
                continue;
            }

            let content = fs::read_to_string(dir_entry.path())?;
            let file_path = dir_entry.path().to_path_buf();

            // use文を解析
            for cap in use_regex.captures_iter(&content) {
                let crate_name = cap[1].to_string();
                if !is_std_crate(&crate_name) {
                    let crate_ref =
                        crate_refs
                            .entry(crate_name.clone())
                            .or_insert_with(|| CrateReference {
                                name: crate_name,
                                features: HashSet::new(),
                                used_in: HashSet::new(),
                            });
                    crate_ref.used_in.insert(file_path.clone());
                }
            }

            // extern crate文を解析
            for cap in extern_regex.captures_iter(&content) {
                let crate_name = cap[1].to_string();
                if !is_std_crate(&crate_name) {
                    let crate_ref =
                        crate_refs
                            .entry(crate_name.clone())
                            .or_insert_with(|| CrateReference {
                                name: crate_name,
                                features: HashSet::new(),
                                used_in: HashSet::new(),
                            });
                    crate_ref.used_in.insert(file_path.clone());
                }
            }
        }

        Ok(crate_refs)
    }

    fn update_cargo_toml(&self, crate_refs: &HashMap<String, CrateReference>) -> Result<()> {
        let content = fs::read_to_string(&self.cargo_toml)?;
        let mut doc = content.parse::<DocumentMut>()?;

        // 現在の依存関係を取得
        let mut current_deps = HashSet::new();
        if let Some(Item::Table(deps)) = doc.get("dependencies") {
            for (key, _) in deps.iter() {
                current_deps.insert(key.to_string());
            }
        }

        // 新しい依存関係を追加
        for (name, crate_ref) in crate_refs {
            if !current_deps.contains(name) && !is_std_crate(name) {
                self.add_dependency(&mut doc, crate_ref)?;
            }
        }

        // 未使用の依存関係を削除
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

        // Cargo.tomlを更新
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

        // フィーチャーフラグがある場合は追加
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

    fn get_latest_version(&self, crate_name: &str) -> Result<String> {
        let url = format!("https://crates.io/api/v1/crates/{}/versions", crate_name);
        let response = ureq::get(&url).call()?;
        let reader = BufReader::new(response.into_reader());
        let response: CratesIoResponse = serde_json::from_reader(reader)?;

        // 最新の非yankedバージョンを取得
        let latest_version = response
            .versions
            .iter()
            .find(|v| !v.yanked)
            .ok_or_else(|| anyhow::anyhow!("No valid version found"))?;

        let version = Version::parse(&latest_version.num)?;
        Ok(format!("^{}.{}.0", version.major, version.minor))
    }

    fn verify_dependencies(&self) -> Result<()> {
        Command::new("cargo")
            .current_dir(&self.project_root)
            .arg("check")
            .status()
            .context("Failed to run cargo check")?;
        Ok(())
    }
}

fn is_hidden(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

fn is_std_crate(name: &str) -> bool {
    let std_crates = [
        "std",
        "core",
        "alloc",
        "test",
        "proc_macro",
        "rand",
        "libc",
        "collections",
    ];
    std_crates.contains(&name)
}

fn is_essential_dep(name: &str) -> bool {
    let essential_deps = [
        "serde",
        "tokio",
        "anyhow",
        "thiserror",
        "async-trait",
        "futures",
    ];
    essential_deps.contains(&name)
}

fn main() -> Result<()> {
    let project_root = std::env::current_dir()?;
    let manager = DependencyManager::new(project_root)?;

    println!("Analyzing project dependencies...");
    let crate_refs = manager.analyze_dependencies()?;

    println!("Updating Cargo.toml...");
    manager.update_cargo_toml(&crate_refs)?;

    println!("Verifying dependencies...");
    manager.verify_dependencies()?;

    println!("Done!");
    Ok(())
}
