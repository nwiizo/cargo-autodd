use anyhow::Result;
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Configuration for cargo-autodd
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Config {
    /// Crates to exclude from analysis
    #[serde(default)]
    pub exclude: HashSet<String>,

    /// Additional essential dependencies (never removed)
    #[serde(default)]
    pub essential: HashSet<String>,

    /// Crates to always treat as dev-dependencies
    #[serde(default)]
    pub dev_only: HashSet<String>,

    /// Whether to skip tests/ directory analysis
    #[serde(default)]
    pub skip_tests: bool,
}

impl Config {
    /// Load config from a file path
    pub fn load(path: &Path) -> Result<Self> {
        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Load config from the default path (.cargo-autodd.toml)
    pub fn load_default(project_root: &Path) -> Result<Self> {
        let config_path = project_root.join(".cargo-autodd.toml");
        Self::load(&config_path)
    }

    /// Check if a crate should be excluded
    pub fn should_exclude(&self, crate_name: &str) -> bool {
        self.exclude.contains(crate_name)
    }

    /// Check if a crate is essential (should never be removed)
    pub fn is_essential(&self, crate_name: &str) -> bool {
        self.essential.contains(crate_name)
    }

    /// Check if a crate should always be a dev-dependency
    pub fn is_dev_only(&self, crate_name: &str) -> bool {
        self.dev_only.contains(crate_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_default_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config = Config::load_default(temp_dir.path())?;
        assert!(config.exclude.is_empty());
        assert!(config.essential.is_empty());
        assert!(config.dev_only.is_empty());
        assert!(!config.skip_tests);
        Ok(())
    }

    #[test]
    fn test_load_config_file() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join(".cargo-autodd.toml");

        let config_content = r#"
exclude = ["internal_crate", "another_crate"]
essential = ["custom_essential"]
dev_only = ["proptest", "criterion"]
skip_tests = true
"#;

        let mut file = fs::File::create(&config_path)?;
        write!(file, "{}", config_content)?;

        let config = Config::load(&config_path)?;
        assert!(config.should_exclude("internal_crate"));
        assert!(config.should_exclude("another_crate"));
        assert!(!config.should_exclude("external_crate"));
        assert!(config.is_essential("custom_essential"));
        assert!(config.is_dev_only("proptest"));
        assert!(config.is_dev_only("criterion"));
        assert!(config.skip_tests);

        Ok(())
    }

    #[test]
    fn test_partial_config() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let config_path = temp_dir.path().join(".cargo-autodd.toml");

        // Only specify exclude
        let config_content = r#"
exclude = ["internal_crate"]
"#;

        let mut file = fs::File::create(&config_path)?;
        write!(file, "{}", config_content)?;

        let config = Config::load(&config_path)?;
        assert!(config.should_exclude("internal_crate"));
        assert!(config.essential.is_empty());
        assert!(config.dev_only.is_empty());
        assert!(!config.skip_tests);

        Ok(())
    }
}
