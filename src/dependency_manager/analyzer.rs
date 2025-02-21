use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use regex::Regex;
use walkdir::WalkDir;

use crate::models::CrateReference;
use crate::utils::{is_hidden, is_std_crate};

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
        let nested_regex = Regex::new(r"\{([^}]*)\}")?;
        let item_regex = Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)")?;

        // Use rust-analyzer CLI to analyze the project
        let output = Command::new("rust-analyzer")
            .arg("analysis")
            .arg("--workspace")
            .current_dir(&self.project_root)
            .output()
            .context("Failed to run rust-analyzer. Is it installed?")?;

        if !output.status.success() {
            println!("Warning: rust-analyzer analysis returned non-zero status. Falling back to regex-based analysis.");
        }

        // Walk through all Rust files in the project
        for entry in WalkDir::new(&self.project_root) {
            let entry = entry?;
            let path = entry.path();
            
            if path.extension().map_or(false, |ext| ext == "rs") {
                if self.debug {
                    println!("Found Rust file: {:?}", path);
                }
                let content = fs::read_to_string(path)?;
                let file_path = path.to_path_buf();

                self.analyze_file(
                    &content,
                    &file_path,
                    &use_regex,
                    &extern_regex,
                    &nested_regex,
                    &item_regex,
                    &mut crate_refs,
                )?;
            }
        }

        if self.debug {
            println!("\nFinal crate references:");
            for (name, crate_ref) in &crate_refs {
                println!("- {} (used in {} files)", name, crate_ref.usage_count());
                println!("  Used in:");
                for path in &crate_ref.used_in {
                    println!("    - {:?}", path);
                }
            }
        }

        Ok(crate_refs)
    }

    fn analyze_file(
        &self,
        content: &str,
        file_path: &PathBuf,
        use_regex: &Regex,
        extern_regex: &Regex,
        nested_regex: &Regex,
        item_regex: &Regex,
        crate_refs: &mut HashMap<String, CrateReference>,
    ) -> Result<()> {
        if self.debug {
            println!("Analyzing file: {:?}", file_path);
        }

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            if self.debug {
                println!("Processing line: {}", line);
            }

            // Handle use statements
            if let Some(cap) = use_regex.captures(line) {
                let full_path = cap[1].to_string();
                let base_crate = full_path.split("::").next().unwrap_or(&full_path).to_string();
                
                if self.debug {
                    println!("Found use statement: {} -> base crate: {}", full_path, base_crate);
                }

                if !is_std_crate(&base_crate) {
                    let crate_ref = crate_refs
                        .entry(base_crate.clone())
                        .or_insert_with(|| CrateReference::new(base_crate.clone()));
                    crate_ref.add_usage(file_path.clone());

                    if self.debug {
                        println!("Added crate reference: {}", crate_ref.name);
                    }
                }

                // Handle nested imports
                if let Some(nested) = nested_regex.captures(line) {
                    let nested_content = nested[1].to_string();
                    if self.debug {
                        println!("Found nested imports: {}", nested_content);
                    }

                    let nested_items = nested_content.split(',');
                    for item in nested_items {
                        let item = item.trim();
                        if self.debug {
                            println!("Processing nested item: {}", item);
                        }

                        if let Some(cap) = item_regex.captures(item) {
                            let crate_name = cap[1].to_string();
                            if self.debug {
                                println!("Found nested crate: {}", crate_name);
                            }

                            if !is_std_crate(&crate_name) {
                                let crate_ref = crate_refs
                                    .entry(crate_name.clone())
                                    .or_insert_with(|| CrateReference::new(crate_name.clone()));
                                crate_ref.add_usage(file_path.clone());

                                if self.debug {
                                    println!("Added nested crate reference: {}", crate_ref.name);
                                }
                            }
                        }
                    }
                }
            }
            
            // Handle extern crate statements
            if let Some(cap) = extern_regex.captures(line) {
                let crate_name = cap[1].to_string();
                if self.debug {
                    println!("Found extern crate: {}", crate_name);
                }

                if !is_std_crate(&crate_name) {
                    let crate_ref = crate_refs
                        .entry(crate_name.clone())
                        .or_insert_with(|| CrateReference::new(crate_name.clone()));
                    crate_ref.add_usage(file_path.clone());

                    if self.debug {
                        println!("Added extern crate reference: {}", crate_ref.name);
                    }
                }
            }
        }

        if self.debug {
            println!("Current crate references:");
            for (name, crate_ref) in crate_refs {
                println!("- {} (used in {} files)", name, crate_ref.usage_count());
            }
        }

        Ok(())
    }
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
                if let Some(relative) = path.strip_prefix(&temp_dir.path()).ok() {
                    println!("    - {}", relative.display());
                }
            }
        }

        assert!(crate_refs.contains_key("serde"), "serde dependency not found");
        assert!(crate_refs.contains_key("tokio"), "tokio dependency not found");
        assert!(crate_refs.contains_key("anyhow"), "anyhow dependency not found");
        assert!(crate_refs.contains_key("regex"), "regex dependency not found");

        let serde_ref = crate_refs.get("serde").unwrap();
        assert_eq!(serde_ref.usage_count(), 2, "serde should be used in two files");

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

        let use_regex = Regex::new(r"^\s*use\s+([a-zA-Z_][a-zA-Z0-9_]*(?:::[a-zA-Z0-9_]*)*)")?;
        let extern_regex = Regex::new(r"^\s*extern\s+crate\s+([a-zA-Z_][a-zA-Z0-9_]*)")?;
        let nested_regex = Regex::new(r"\{([^}]*)\}")?;
        let item_regex = Regex::new(r"([a-zA-Z_][a-zA-Z0-9_]*)")?;
        let mut crate_refs = HashMap::new();

        analyzer.analyze_file(
            content,
            &file_path,
            &use_regex,
            &extern_regex,
            &nested_regex,
            &item_regex,
            &mut crate_refs,
        )?;

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