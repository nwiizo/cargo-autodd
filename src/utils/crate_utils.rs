use std::path::Path;

/// Checks if a path represents a hidden file or directory
pub fn is_hidden(path: &Path) -> bool {
    path.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

/// Checks if a crate name represents a standard library crate or type
pub fn is_std_crate(name: &str) -> bool {
    let std_crates = [
        // Standard library crates
        "std",
        "core",
        "alloc",
        "test",
        "proc_macro",
        "rand",
        "libc",
        "collections",
        // Common standard library types that might be mistaken for crates
        "String",
        "Vec",
        "HashMap",
        "HashSet",
        "BTreeMap",
        "BTreeSet",
        "PathBuf",
        "Path",
        "Result",
        "Option",
        "Box",
        "Arc",
        "Rc",
        "Cell",
        "RefCell",
        "Mutex",
        "RwLock",
    ];
    std_crates.contains(&name)
        || name.starts_with("std::")
        || name.starts_with("core::")
        || name.starts_with("alloc::")
}

/// Checks if a dependency is considered essential and should not be removed
pub fn is_essential_dep(name: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_hidden() {
        assert!(is_hidden(Path::new(".git")));
        assert!(is_hidden(Path::new("/path/to/.hidden")));
        assert!(!is_hidden(Path::new("visible")));
        assert!(!is_hidden(Path::new("/path/to/visible")));
    }

    #[test]
    fn test_is_std_crate() {
        assert!(is_std_crate("std"));
        assert!(is_std_crate("core"));
        assert!(!is_std_crate("serde"));
        assert!(!is_std_crate("custom_crate"));
    }

    #[test]
    fn test_is_essential_dep() {
        assert!(is_essential_dep("serde"));
        assert!(is_essential_dep("tokio"));
        assert!(!is_essential_dep("custom_crate"));
        assert!(!is_essential_dep("std"));
    }
}
