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
    /// Whether this crate is a path dependency (internal crate)
    pub is_path_dependency: bool,
    /// Path to the internal crate if it's a path dependency
    pub path: Option<String>,
    /// Whether this crate is marked as not publishable
    pub publish: Option<bool>,
}

impl CrateReference {
    pub fn new(name: String) -> Self {
        Self {
            name,
            features: HashSet::new(),
            used_in: HashSet::new(),
            is_path_dependency: false,
            path: None,
            publish: None,
        }
    }

    pub fn with_path(name: String, path: String) -> Self {
        Self {
            name,
            features: HashSet::new(),
            used_in: HashSet::new(),
            is_path_dependency: true,
            path: Some(path),
            publish: None,
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

    pub fn set_as_path_dependency(&mut self, path: String) {
        self.is_path_dependency = true;
        self.path = Some(path);
    }

    pub fn set_publish(&mut self, publish: bool) {
        self.publish = Some(publish);
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
        assert!(!crate_ref.is_path_dependency);
        assert!(crate_ref.path.is_none());
        assert!(crate_ref.publish.is_none());
    }

    #[test]
    fn test_with_path() {
        let crate_ref =
            CrateReference::with_path("test_crate".to_string(), "../test_crate".to_string());
        assert_eq!(crate_ref.name, "test_crate");
        assert!(crate_ref.features.is_empty());
        assert!(crate_ref.used_in.is_empty());
        assert!(crate_ref.is_path_dependency);
        assert_eq!(crate_ref.path, Some("../test_crate".to_string()));
        assert!(crate_ref.publish.is_none());
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

    #[test]
    fn test_set_as_path_dependency() {
        let mut crate_ref = CrateReference::new("test_crate".to_string());
        crate_ref.set_as_path_dependency("../test_crate".to_string());
        assert!(crate_ref.is_path_dependency);
        assert_eq!(crate_ref.path, Some("../test_crate".to_string()));
    }

    #[test]
    fn test_set_publish() {
        let mut crate_ref = CrateReference::new("test_crate".to_string());
        crate_ref.set_publish(false);
        assert_eq!(crate_ref.publish, Some(false));
    }
}
