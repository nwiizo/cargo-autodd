use toml_edit::DocumentMut;

/// Resolve the correct dependencies table path based on whether the document
/// is a workspace with workspace.dependencies.
pub fn resolve_deps_table_path(doc: &DocumentMut) -> &'static str {
    if doc
        .get("workspace")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("dependencies"))
        .is_some()
    {
        "workspace.dependencies"
    } else {
        "dependencies"
    }
}

/// Get the dependencies table from a TOML document, handling both
/// workspace.dependencies and regular dependencies paths.
pub fn get_deps_table<'a>(doc: &'a DocumentMut, deps_path: &str) -> Option<&'a toml_edit::Table> {
    if deps_path.contains('.') {
        let parts: Vec<&str> = deps_path.split('.').collect();
        doc.get(parts[0])
            .and_then(|t| t.as_table())
            .and_then(|t| t.get(parts[1]))
            .and_then(|t| t.as_table())
    } else {
        doc.get(deps_path).and_then(|t| t.as_table())
    }
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
    fn test_is_std_crate() {
        // Standard library crates
        assert!(is_std_crate("std"));
        assert!(is_std_crate("core"));
        assert!(is_std_crate("alloc"));
        assert!(is_std_crate("proc_macro"));

        // Standard library types
        assert!(is_std_crate("String"));
        assert!(is_std_crate("Vec"));
        assert!(is_std_crate("HashMap"));

        // Paths starting with std/core/alloc
        assert!(is_std_crate("std::collections"));
        assert!(is_std_crate("core::mem"));
        assert!(is_std_crate("alloc::vec"));

        // External crates (NOT std)
        assert!(!is_std_crate("serde"));
        assert!(!is_std_crate("tokio"));
        assert!(!is_std_crate("rand")); // rand is NOT a std crate
        assert!(!is_std_crate("libc")); // libc is NOT a std crate
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
