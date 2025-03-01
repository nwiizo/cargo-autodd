use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use regex::Regex;
use toml_edit::{DocumentMut, Item};
use walkdir::WalkDir;

use crate::models::CrateReference;
use crate::utils::is_std_crate;

pub struct DependencyAnalyzer {
    project_root: PathBuf,
    debug: bool,
}

impl DependencyAnalyzer {
    pub fn new(project_root: PathBuf) -> Self {
        Self {
            project_root,
            debug: false,
        }
    }

    pub fn with_debug(project_root: PathBuf, debug: bool) -> Self {
        Self {
            project_root,
            debug,
        }
    }

    pub fn analyze_dependencies(&self) -> Result<HashMap<String, CrateReference>> {
        let mut crate_refs = HashMap::new();
        let use_regex = Regex::new(r"^\s*use\s+([a-zA-Z_][a-zA-Z0-9_]*(?:::[a-zA-Z0-9_]*)*)")?;
        let extern_regex = Regex::new(r"^\s*extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)")?;

        // 既存のCargo.tomlから内部クレート情報を読み取る
        self.load_existing_dependencies(&mut crate_refs)?;

        // Walk through all Rust files in the project
        for entry in WalkDir::new(&self.project_root) {
            let entry = entry?;
            let path = entry.path();

            // Skip test files and build scripts
            if path.to_string_lossy().contains("tests/")
                || path.file_name().is_some_and(|f| f == "build.rs")
            {
                continue;
            }

            if path.extension().is_some_and(|ext| ext == "rs") {
                let content = fs::read_to_string(path)?;
                let file_path = path.to_path_buf();

                self.analyze_file(FileAnalysisContext {
                    content: &content,
                    file_path: &file_path,
                    use_regex: &use_regex,
                    extern_regex: &extern_regex,
                    crate_refs: &mut crate_refs,
                })?;
            }
        }

        // Filter out dev-dependencies and test-only crates
        crate_refs.retain(|name, _| {
            !name.ends_with("_test")
                && !name.ends_with("_tests")
                && name != "test"
                && name != "tempfile"
                && !name.starts_with("crate")
        });

        if self.debug {
            println!("\nFinal crate references:");
            for (name, crate_ref) in &crate_refs {
                println!("- {} (used in {} files)", name, crate_ref.usage_count());
                if crate_ref.is_path_dependency {
                    println!("  Path dependency: {}", crate_ref.path.as_ref().unwrap_or(&"unknown".to_string()));
                }
                if let Some(publish) = crate_ref.publish {
                    println!("  Publish: {}", publish);
                }
                println!("  Used in:");
                for path in &crate_ref.used_in {
                    println!("    - {:?}", path);
                }
            }
        }

        Ok(crate_refs)
    }

    /// Cargo.tomlから既存の依存関係情報を読み込む
    fn load_existing_dependencies(&self, crate_refs: &mut HashMap<String, CrateReference>) -> Result<()> {
        let cargo_toml_path = self.project_root.join("Cargo.toml");
        if !cargo_toml_path.exists() {
            return Ok(());
        }

        if self.debug {
            println!("Loading dependencies from {:?}", cargo_toml_path);
        }

        let content = fs::read_to_string(&cargo_toml_path)
            .with_context(|| format!("Failed to read Cargo.toml at {:?}", cargo_toml_path))?;
        let doc = content.parse::<DocumentMut>()
            .with_context(|| format!("Failed to parse Cargo.toml at {:?}", cargo_toml_path))?;

        // パッケージの公開設定を確認
        let publish = if let Some(package) = doc.get("package") {
            if let Some(publish_value) = package.get("publish") {
                match publish_value.as_bool() {
                    Some(value) => Some(value),
                    None => None,
                }
            } else {
                None
            }
        } else {
            None
        };

        if self.debug {
            println!("Package publish setting: {:?}", publish);
        }

        // 依存関係を読み込む
        if let Some(dependencies) = doc.get("dependencies").and_then(|d| d.as_table()) {
            for (name, value) in dependencies.iter() {
                let crate_name = name.to_string();
                
                if self.debug {
                    println!("Found dependency: {}", crate_name);
                    println!("Dependency value type: {:?}", value);
                }
                
                // 既に存在する場合はスキップ
                if crate_refs.contains_key(&crate_name) {
                    continue;
                }

                match value {
                    // パス依存の場合（通常のテーブル形式）
                    Item::Table(table) => {
                        if self.debug {
                            println!("Dependency {} is a table: {:?}", crate_name, table);
                        }
                        if let Some(path_value) = table.get("path") {
                            if self.debug {
                                println!("Path value for {}: {:?}", crate_name, path_value);
                            }
                            if let Some(path_str) = path_value.as_str() {
                                let mut crate_ref = CrateReference::with_path(crate_name.clone(), path_str.to_string());
                                if let Some(publish_value) = publish {
                                    crate_ref.set_publish(publish_value);
                                }
                                
                                if self.debug {
                                    println!("Adding path dependency: {} at {}", crate_name, path_str);
                                    println!("With publish setting: {:?}", crate_ref.publish);
                                }
                                
                                crate_refs.insert(crate_name, crate_ref);
                            }
                        }
                    },
                    // パス依存の場合（インラインテーブル形式）
                    Item::Value(val) if val.is_inline_table() => {
                        if self.debug {
                            println!("Dependency {} is an inline table: {:?}", crate_name, val);
                        }
                        if let Some(inline_table) = val.as_inline_table() {
                            if let Some(path_value) = inline_table.get("path") {
                                if self.debug {
                                    println!("Path value for {}: {:?}", crate_name, path_value);
                                }
                                if let Some(path_str) = path_value.as_str() {
                                    let mut crate_ref = CrateReference::with_path(crate_name.clone(), path_str.to_string());
                                    if let Some(publish_value) = publish {
                                        crate_ref.set_publish(publish_value);
                                    }
                                    
                                    if self.debug {
                                        println!("Adding path dependency (inline): {} at {}", crate_name, path_str);
                                        println!("With publish setting: {:?}", crate_ref.publish);
                                    }
                                    
                                    crate_refs.insert(crate_name, crate_ref);
                                }
                            }
                        }
                    },
                    // 通常の依存関係の場合
                    _ => {
                        // 通常の依存関係は分析時に検出されるので、ここでは何もしない
                        if self.debug {
                            println!("Skipping regular dependency: {}", crate_name);
                        }
                    }
                }
            }
        } else if self.debug {
            println!("No dependencies section found in Cargo.toml");
        }

        Ok(())
    }

    fn analyze_file(&self, ctx: FileAnalysisContext) -> Result<()> {
        let FileAnalysisContext {
            content,
            file_path,
            use_regex,
            extern_regex,
            crate_refs,
        } = ctx;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            // Handle use statements
            if let Some(cap) = use_regex.captures(line) {
                let full_path = cap[1].to_string();
                let parts: Vec<&str> = full_path.split("::").collect();

                // Skip empty parts and special keywords
                if parts.is_empty()
                    || parts[0] == "self"
                    || parts[0] == "super"
                    || parts[0] == "crate"
                {
                    continue;
                }

                let base_crate = parts[0].to_string();

                // Skip standard library types and modules
                if !is_std_crate(&base_crate)
                    && !base_crate.starts_with("std::")
                    && !base_crate.starts_with("core::")
                    && !base_crate.starts_with("alloc::")
                {
                    // 既に内部クレートとして登録されている場合は、使用情報のみ追加
                    if let Some(crate_ref) = crate_refs.get_mut(&base_crate) {
                        crate_ref.add_usage(file_path.to_path_buf());
                    } else {
                        let crate_ref = crate_refs
                            .entry(base_crate.clone())
                            .or_insert_with(|| CrateReference::new(base_crate));
                        crate_ref.add_usage(file_path.to_path_buf());
                    }
                }
            }

            // Handle extern crate statements
            if let Some(cap) = extern_regex.captures(line) {
                let crate_name = cap[1].to_string();
                if !is_std_crate(&crate_name) {
                    // 既に内部クレートとして登録されている場合は、使用情報のみ追加
                    if let Some(crate_ref) = crate_refs.get_mut(&crate_name) {
                        crate_ref.add_usage(file_path.to_path_buf());
                    } else {
                        let crate_ref = crate_refs
                            .entry(crate_name.clone())
                            .or_insert_with(|| CrateReference::new(crate_name));
                        crate_ref.add_usage(file_path.to_path_buf());
                    }
                }
            }
        }

        Ok(())
    }
}

struct FileAnalysisContext<'a> {
    content: &'a str,
    file_path: &'a PathBuf,
    use_regex: &'a Regex,
    extern_regex: &'a Regex,
    crate_refs: &'a mut HashMap<String, CrateReference>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_file(dir: &TempDir, name: &str, content: &str) -> Result<PathBuf> {
        let path = dir.path().join(name);
        let mut file = File::create(&path)?;
        writeln!(file, "{}", content.trim())?;
        Ok(path)
    }

    #[test]
    fn test_analyze_dependencies() -> Result<()> {
        let temp_dir = TempDir::new()?;

        // Create test files with various import styles
        let main_rs = create_test_file(
            &temp_dir,
            "main.rs",
            r#"use serde::Serialize;
               use tokio::runtime::Runtime;
               use anyhow::Result;
               use std::fs;"#,
        )?;

        let lib_rs = create_test_file(
            &temp_dir,
            "lib.rs",
            r#"use serde::{Deserialize, Serialize};
               use regex::Regex;
               extern crate serde;"#,
        )?;

        // Debug output
        println!("\nTest files created:");
        println!("main.rs content:\n{}", fs::read_to_string(&main_rs)?);
        println!("lib.rs content:\n{}", fs::read_to_string(&lib_rs)?);
        println!("\nStarting analysis...\n");

        let analyzer = DependencyAnalyzer::new(temp_dir.path().to_path_buf());
        let crate_refs = analyzer.analyze_dependencies()?;

        // Debug output
        println!("\nAnalysis complete. Found crates:");
        for (name, crate_ref) in &crate_refs {
            println!("- {} (used in {} files)", name, crate_ref.usage_count());
            println!("  Used in:");
            for path in &crate_ref.used_in {
                if let Ok(relative) = path.strip_prefix(temp_dir.path()) {
                    println!("    - {}", relative.display());
                }
            }
        }

        assert!(
            crate_refs.contains_key("serde"),
            "serde dependency not found"
        );
        assert!(
            crate_refs.contains_key("tokio"),
            "tokio dependency not found"
        );
        assert!(
            crate_refs.contains_key("anyhow"),
            "anyhow dependency not found"
        );
        assert!(
            crate_refs.contains_key("regex"),
            "regex dependency not found"
        );

        let serde_ref = crate_refs.get("serde").unwrap();
        assert_eq!(
            serde_ref.usage_count(),
            2,
            "serde should be used in two files"
        );

        Ok(())
    }

    #[test]
    fn test_load_existing_dependencies() -> Result<()> {
        let temp_dir = TempDir::new()?;
        
        // Create Cargo.toml with path dependencies
        let cargo_toml_content = r#"
[package]
name = "test-package"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
serde = "1.0"
internal-crate = { path = "../internal-crate" }
"#;
        
        let cargo_toml_path = temp_dir.path().join("Cargo.toml");
        let mut file = File::create(&cargo_toml_path)?;
        writeln!(file, "{}", cargo_toml_content)?;
        
        // Create a simple source file to ensure the analyzer has something to work with
        fs::create_dir_all(temp_dir.path().join("src"))?;
        let main_rs_path = temp_dir.path().join("src/main.rs");
        let main_rs_content = r#"
fn main() {
    println!("Hello, world!");
}
"#;
        let mut file = File::create(main_rs_path)?;
        writeln!(file, "{}", main_rs_content)?;
        
        // Run the analyzer with debug mode to see what's happening
        let analyzer = DependencyAnalyzer::with_debug(temp_dir.path().to_path_buf(), true);
        
        // Analyze dependencies (this will call load_existing_dependencies internally)
        let crate_refs = analyzer.analyze_dependencies()?;
        
        // Check that internal-crate was detected as a path dependency
        assert!(crate_refs.contains_key("internal-crate"), 
                "internal-crate dependency not found");
        
        if let Some(internal_crate) = crate_refs.get("internal-crate") {
            assert!(internal_crate.is_path_dependency, 
                    "internal-crate should be a path dependency");
            assert_eq!(internal_crate.path, Some("../internal-crate".to_string()), 
                      "internal-crate path should be ../internal-crate");
            assert_eq!(internal_crate.publish, Some(false), 
                      "publish should be false");
        }
        
        Ok(())
    }

    #[test]
    fn test_analyze_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let analyzer = DependencyAnalyzer::new(temp_dir.path().to_path_buf());
        let file_path = temp_dir.path().join("test.rs");
        let content = r#"use serde::Serialize;
                       use tokio::runtime::Runtime;
                       extern crate anyhow;
                       use std::fs;"#;

        println!("\nTest file content:\n{}", content);
        println!("\nStarting analysis...\n");

        let mut crate_refs = HashMap::new();
        let use_regex = Regex::new(r"^\s*use\s+([a-zA-Z_][a-zA-Z0-9_]*(?:::[a-zA-Z0-9_]*)*)")?;
        let extern_regex = Regex::new(r"^\s*extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)")?;

        analyzer.analyze_file(FileAnalysisContext {
            content: &content,
            file_path: &file_path,
            use_regex: &use_regex,
            extern_regex: &extern_regex,
            crate_refs: &mut crate_refs,
        })?;

        println!("\nAnalysis complete. Found crates:");
        for (name, crate_ref) in &crate_refs {
            println!("- {} (used in {} files)", name, crate_ref.usage_count());
        }

        assert!(crate_refs.contains_key("serde"), "serde dependency not found");
        assert!(crate_refs.contains_key("tokio"), "tokio dependency not found");
        assert!(crate_refs.contains_key("anyhow"), "anyhow dependency not found");

        Ok(())
    }
}
