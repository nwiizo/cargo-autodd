use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use regex::Regex;
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
                println!("  Used in:");
                for path in &crate_ref.used_in {
                    println!("    - {:?}", path);
                }
            }
        }

        Ok(crate_refs)
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
                    let crate_ref = crate_refs
                        .entry(base_crate.clone())
                        .or_insert_with(|| CrateReference::new(base_crate));
                    crate_ref.add_usage(file_path.to_path_buf());
                }
            }

            // Handle extern crate statements
            if let Some(cap) = extern_regex.captures(line) {
                let crate_name = cap[1].to_string();
                if !is_std_crate(&crate_name) {
                    let crate_ref = crate_refs
                        .entry(crate_name.clone())
                        .or_insert_with(|| CrateReference::new(crate_name));
                    crate_ref.add_usage(file_path.to_path_buf());
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
        let mut crate_refs = HashMap::new();

        analyzer.analyze_file(FileAnalysisContext {
            content,
            file_path: &file_path,
            use_regex: &use_regex,
            extern_regex: &extern_regex,
            crate_refs: &mut crate_refs,
        })?;

        println!("\nAnalysis complete. Found crates:");
        for (name, crate_ref) in &crate_refs {
            println!("- {} (used in {} files)", name, crate_ref.usage_count());
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

        Ok(())
    }
}
