//! Rebuild causality graph for tracking root causes of rebuilds
//!
//! Cargo's rebuild triggers form a directed acyclic graph where:
//! - Root causes are nodes with no incoming edges (file changes, env var
//!   changes)
//! - `UnitDependencyInfoChanged` creates edges between dependent packages
//! - Finding root causes means traversing back to nodes with in-degree 0

use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
};

use serde::Serialize;

use crate::rebuild_reason::RebuildReason;

/// Identifies a compilation unit in the rebuild graph
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct PackageTarget {
    pub package_id: String,
    pub target: Option<String>,
}

impl PackageTarget {
    pub fn new(package_id: impl Into<String>, target: Option<String>) -> Self {
        Self {
            package_id: package_id.into(),
            target,
        }
    }
}

impl Display for PackageTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let package_name = self
            .package_id
            .split_whitespace()
            .next()
            .unwrap_or(&self.package_id);

        match &self.target {
            Some(target) => write!(f, "{package_name} [{target}]"),
            None => write!(f, "{package_name}"),
        }
    }
}

/// A node in the rebuild graph: a package with its direct rebuild reason
#[derive(Debug, Clone, Serialize)]
pub struct RebuildNode {
    pub package: PackageTarget,
    pub reason: RebuildReason,
}

impl RebuildNode {
    #[must_use]
    pub const fn new(package: PackageTarget, reason: RebuildReason) -> Self {
        Self { package, reason }
    }

    /// Returns true if this is a root cause (not caused by another package
    /// rebuild)
    #[must_use]
    pub const fn is_root_cause(&self) -> bool {
        !matches!(self.reason, RebuildReason::UnitDependencyInfoChanged { .. })
    }
}

/// Directed graph of rebuild causality
///
/// Edges point from cause to effect:
/// - Package A (root cause) -> Package B (depends on A)
/// - An edge exists when Package B's rebuild reason is
///   `UnitDependencyInfoChanged` mentioning A
#[derive(Debug, Default)]
pub struct RebuildGraph {
    nodes: Vec<RebuildNode>,
    /// Map from dependency name to indices of nodes that caused its rebuild
    dependency_causes: HashMap<String, Vec<usize>>,
    /// Map from package to its node index
    package_to_node: HashMap<PackageTarget, usize>,
    /// Track seen (`package_name`, `reason_key`) to deduplicate
    seen_entries: HashSet<(String, String)>,
}

impl RebuildGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rebuild node to the graph, deduplicating by package name and
    /// reason
    pub fn add_node(&mut self, node: RebuildNode) -> Option<usize> {
        let package_name = extract_package_name(&node.package.package_id);
        let reason_key = node.reason.to_string();
        let entry_key = (package_name.clone(), reason_key);

        if !self.seen_entries.insert(entry_key) {
            return None;
        }

        let idx = self.nodes.len();
        self.package_to_node.insert(node.package.clone(), idx);

        // If this is a root cause, record it as a potential cause for dependencies
        if node.is_root_cause() {
            self.dependency_causes
                .entry(package_name)
                .or_default()
                .push(idx);
        }

        self.nodes.push(node);
        Some(idx)
    }

    /// Find all root causes (nodes that are not caused by dependency changes)
    #[must_use]
    pub fn root_causes(&self) -> Vec<&RebuildNode> {
        self.nodes.iter().filter(|n| n.is_root_cause()).collect()
    }

    /// Find root causes with their full downstream impact chains
    #[must_use]
    pub fn root_cause_chains(&self) -> Vec<RootCauseChain> {
        let mut chains = Vec::new();
        let root_causes: Vec<_> = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.is_root_cause())
            .collect();

        for (root_idx, root_node) in root_causes {
            let affected = self.find_affected_packages(root_idx);
            chains.push(RootCauseChain {
                root_cause: root_node.clone(),
                affected_packages: affected,
            });
        }

        chains
    }

    /// Find all packages affected by a root cause (BFS traversal)
    fn find_affected_packages(&self, root_idx: usize) -> Vec<RebuildNode> {
        let root_name = extract_package_name(&self.nodes[root_idx].package.package_id);
        let root_name_normalized = normalize_crate_name(&root_name);
        let mut affected = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(root_idx);

        // Find nodes that were rebuilt because of this root cause
        for (idx, node) in self.nodes.iter().enumerate() {
            if visited.contains(&idx) {
                continue;
            }

            if let RebuildReason::UnitDependencyInfoChanged { name, .. } = &node.reason {
                let dep_name_normalized = normalize_crate_name(name);
                let is_affected = dep_name_normalized == root_name_normalized
                    || self.is_transitively_affected(name, &root_name);

                if is_affected {
                    affected.push(node.clone());
                    visited.insert(idx);
                }
            }
        }

        affected
    }

    /// Check if a dependency was transitively affected by a root cause
    fn is_transitively_affected(&self, dep_name: &str, root_name: &str) -> bool {
        let root_name_normalized = normalize_crate_name(root_name);
        // Check if dep_name was rebuilt because of root_name through the chain
        for node in &self.nodes {
            let package_name = extract_package_name(&node.package.package_id);
            let package_name_normalized = normalize_crate_name(&package_name);
            let dep_name_normalized = normalize_crate_name(dep_name);

            if package_name_normalized != dep_name_normalized {
                continue;
            }

            if let RebuildReason::UnitDependencyInfoChanged { name, .. } = &node.reason {
                let name_normalized = normalize_crate_name(name);
                if name_normalized == root_name_normalized {
                    return true;
                }
                if self.is_transitively_affected(name, root_name) {
                    return true;
                }
            }
        }
        false
    }

    /// Check if the graph is empty
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Serialize the graph to a JSON string
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.root_cause_chains())
    }

    /// Print the graph as JSON to stdout
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn print_json(&self) -> Result<(), serde_json::Error> {
        println!("{}", self.to_json()?);
        Ok(())
    }

    /// Print a human-readable analysis to stderr
    pub fn print_analysis(&self) {
        let root_causes = self.root_causes();

        if root_causes.is_empty() {
            eprintln!("No rebuild triggers detected.");
            return;
        }

        eprintln!(
            "\n{} root cause{}:",
            root_causes.len(),
            if root_causes.len() == 1 { "" } else { "s" }
        );

        for root in &root_causes {
            eprintln!("  {} {}", root.package, root.reason);
        }
    }
}

/// A root cause and all packages affected by it
#[derive(Debug, Clone, Serialize)]
pub struct RootCauseChain {
    pub root_cause: RebuildNode,
    pub affected_packages: Vec<RebuildNode>,
}

impl RootCauseChain {
    /// Total number of rebuilds caused (root + affected)
    #[must_use]
    #[cfg(test)]
    pub const fn total_rebuilds(&self) -> usize {
        1 + self.affected_packages.len()
    }
}

/// Extract just the package name from a `package_id` like "libz-sys v1.1.23"
fn extract_package_name(package_id: &str) -> String {
    package_id
        .split_whitespace()
        .next()
        .unwrap_or(package_id)
        .to_string()
}

/// Normalize a crate name for comparison (hyphens and underscores are
/// equivalent)
fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
        process::{Command, Stdio},
    };

    use assert_cmd::prelude::*;
    use tempfile::TempDir;

    use super::*;
    use crate::fingerprint_parser::parse_rebuild_entry;

    #[test]
    fn builds_and_analyzes_rebuild_graph() {
        let mut graph = RebuildGraph::new();

        graph.add_node(RebuildNode::new(
            PackageTarget::new("libz-sys v1.1.23", None),
            RebuildReason::EnvVarChanged {
                name: "CC".to_string(),
                old_value: Some("gcc".to_string()),
                new_value: None,
            },
        ));

        graph.add_node(RebuildNode::new(
            PackageTarget::new("rusqlite v0.31.0", None),
            RebuildReason::UnitDependencyInfoChanged {
                name: "libz-sys".to_string(),
                old_fingerprint: "123".to_string(),
                new_fingerprint: "456".to_string(),
                context: None,
            },
        ));

        let roots = graph.root_causes();
        assert_eq!(roots.len(), 1);
        assert!(matches!(
            roots[0].reason,
            RebuildReason::EnvVarChanged { .. }
        ));

        let chains = graph.root_cause_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].total_rebuilds(), 2);
    }

    fn create_workspace_with_dependencies() -> TempDir {
        let temp_dir = TempDir::new().unwrap();

        fs::write(
            temp_dir.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["lib-a", "lib-b", "app"]
resolver = "2"
"#,
        )
        .unwrap();

        let lib_a_dir = temp_dir.path().join("lib-a");
        fs::create_dir_all(lib_a_dir.join("src")).unwrap();
        fs::write(
            lib_a_dir.join("Cargo.toml"),
            r#"
[package]
name = "lib-a"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(
            lib_a_dir.join("src/lib.rs"),
            r#"
pub fn greet() -> &'static str {
    "Hello from lib-a"
}
"#,
        )
        .unwrap();

        let middle_lib_dir = temp_dir.path().join("lib-b");
        fs::create_dir_all(middle_lib_dir.join("src")).unwrap();
        fs::write(
            middle_lib_dir.join("Cargo.toml"),
            r#"
[package]
name = "lib-b"
version = "0.1.0"
edition = "2021"

[dependencies]
lib-a = { path = "../lib-a" }
"#,
        )
        .unwrap();
        fs::write(
            middle_lib_dir.join("src/lib.rs"),
            r#"
pub fn message() -> String {
    format!("lib-b says: {}", lib_a::greet())
}
"#,
        )
        .unwrap();

        let app_dir = temp_dir.path().join("app");
        fs::create_dir_all(app_dir.join("src")).unwrap();
        fs::write(
            app_dir.join("Cargo.toml"),
            r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
lib-b = { path = "../lib-b" }
"#,
        )
        .unwrap();
        fs::write(
            app_dir.join("src/main.rs"),
            r#"
fn main() {
    println!("{}", lib_b::message());
}
"#,
        )
        .unwrap();

        temp_dir
    }

    fn collect_cargo_fingerprint_logs(project_path: &Path) -> Vec<String> {
        let output = Command::new("cargo")
            .arg("build")
            .current_dir(project_path)
            .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to run cargo build");

        let stderr = String::from_utf8_lossy(&output.stderr);
        stderr
            .lines()
            .filter(|line| {
                line.contains("fingerprint") && (line.contains("dirty:") || line.contains("stale:"))
            })
            .map(String::from)
            .collect()
    }

    fn build_graph_from_logs(log_lines: &[String]) -> RebuildGraph {
        let mut graph = RebuildGraph::new();
        for line in log_lines {
            if let Some(entry) = parse_rebuild_entry(line) {
                graph.add_node(RebuildNode::new(entry.package, entry.reason));
            }
        }
        graph
    }

    #[test]
    fn json_structure_is_valid_for_workspace_rebuild() {
        let workspace = create_workspace_with_dependencies();

        let mut build_cmd = Command::new("cargo");
        build_cmd.arg("build").current_dir(workspace.path());
        build_cmd.assert().success();

        let lib_a_src = workspace.path().join("lib-a/src/lib.rs");
        fs::write(
            &lib_a_src,
            r#"
pub fn greet() -> &'static str {
    "Hello from modified lib-a!"
}
"#,
        )
        .unwrap();

        let log_lines = collect_cargo_fingerprint_logs(workspace.path());
        let graph = build_graph_from_logs(&log_lines);

        let json = graph.to_json().expect("JSON serialization should succeed");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should be valid and parseable");

        let root_array = parsed.as_array().expect("JSON should be an array");
        assert!(
            !root_array.is_empty(),
            "Should have at least one root cause"
        );

        for root in root_array {
            assert!(
                root.get("root_cause").is_some(),
                "Root should have root_cause"
            );

            let root_cause = &root["root_cause"];
            let reason = &root_cause["reason"];
            assert!(
                reason.get("UnitDependencyInfoChanged").is_none(),
                "Root cause should not be a dependency change: {reason}"
            );

            if let Some(affected) = root.get("affected_packages") {
                let affected_arr = affected.as_array().unwrap();
                for pkg in affected_arr {
                    let pkg_reason = &pkg["reason"];
                    assert!(
                        pkg_reason.get("UnitDependencyInfoChanged").is_some(),
                        "Affected package should be a dependency change: {pkg_reason}"
                    );
                }
            }
        }

        let has_lib_a_root = root_array.iter().any(|r| {
            r["root_cause"]["package"]["package_id"]
                .as_str()
                .is_some_and(|p| p.contains("lib-a"))
        });
        assert!(
            has_lib_a_root,
            "lib-a should be identified as a root cause since we modified it"
        );
    }
}
