use std::collections::HashSet;
use std::path::PathBuf;

/// A reference to a crate and its usage within the project
#[derive(Debug, Clone)]
pub struct CrateReference {
    /// Name of the crate
    pub name: String,
    /// Set of features used by this crate
    pub features: HashSet<String>,
    /// Set of file paths where this crate is used
    pub used_in: HashSet<PathBuf>,
}

impl CrateReference {
    pub fn new(name: String) -> Self {
        Self {
            name,
            features: HashSet::new(),
            used_in: HashSet::new(),
        }
    }

    pub fn add_usage(&mut self, path: PathBuf) {
        self.used_in.insert(path);
    }

    pub fn add_feature(&mut self, feature: String) {
        self.features.insert(feature);
    }

    pub fn usage_count(&self) -> usize {
        self.used_in.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_new_crate_reference() {
        let crate_ref = CrateReference::new("test_crate".to_string());
        assert_eq!(crate_ref.name, "test_crate");
        assert!(crate_ref.features.is_empty());
        assert!(crate_ref.used_in.is_empty());
    }

    #[test]
    fn test_add_usage() {
        let mut crate_ref = CrateReference::new("test_crate".to_string());
        let path = Path::new("/test/path.rs").to_path_buf();
        crate_ref.add_usage(path.clone());
        assert!(crate_ref.used_in.contains(&path));
        assert_eq!(crate_ref.usage_count(), 1);
    }

    #[test]
    fn test_add_feature() {
        let mut crate_ref = CrateReference::new("test_crate".to_string());
        crate_ref.add_feature("test_feature".to_string());
        assert!(crate_ref.features.contains("test_feature"));
    }
} 