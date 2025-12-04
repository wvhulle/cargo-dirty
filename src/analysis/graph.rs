//! Rebuild causality graph for tracking root causes of rebuilds
//!
//! Cargo's rebuild triggers form a directed acyclic graph where:
//! - Root causes are nodes with no incoming edges (file changes, env var changes)
//! - `UnitDependencyInfoChanged` creates edges between dependent packages
//! - Finding root causes means traversing back to nodes with in-degree 0

use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter, Result as FmtResult},
};

use crate::parsing::rebuild_reason::RebuildReason;
use serde::Serialize;

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

    #[must_use]
    pub fn unknown() -> Self {
        Self {
            package_id: "unknown".to_string(),
            target: None,
        }
    }
}

impl Display for PackageTarget {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match &self.target {
            Some(target) => write!(f, "{} ({})", self.package_id, target),
            None => write!(f, "{}", self.package_id),
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

    /// Returns true if this is a root cause (not caused by another package rebuild)
    #[must_use]
    pub const fn is_root_cause(&self) -> bool {
        !matches!(self.reason, RebuildReason::UnitDependencyInfoChanged { .. })
    }
}

/// Directed graph of rebuild causality
///
/// Edges point from cause to effect:
/// - Package A (root cause) -> Package B (depends on A)
/// - An edge exists when Package B's rebuild reason is `UnitDependencyInfoChanged` mentioning A
#[derive(Debug, Default)]
pub struct RebuildGraph {
    nodes: Vec<RebuildNode>,
    /// Map from dependency name to indices of nodes that caused its rebuild
    dependency_causes: HashMap<String, Vec<usize>>,
    /// Map from package to its node index
    package_to_node: HashMap<PackageTarget, usize>,
    /// Track seen (package_name, reason_key) to deduplicate
    seen_entries: HashSet<(String, String)>,
}

impl RebuildGraph {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a rebuild node to the graph, deduplicating by package name and reason
    pub fn add_node(&mut self, node: RebuildNode) -> Option<usize> {
        let package_name = extract_package_name(&node.package.package_id);
        let reason_key = reason_dedup_key(&node.reason);
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

    /// Find the causal chain for a given node
    /// Returns nodes in order from root cause to the given node
    #[must_use]
    pub fn causal_chain(&self, node_idx: usize) -> Vec<&RebuildNode> {
        let node = &self.nodes[node_idx];

        if let RebuildReason::UnitDependencyInfoChanged { name, .. } = &node.reason
            && let Some(cause_indices) = self.dependency_causes.get(name)
            && let Some(&cause_idx) = cause_indices.first()
        {
            let mut chain = self.causal_chain(cause_idx);
            chain.push(node);
            return chain;
        }

        // This is a root cause or we couldn't find the cause
        vec![node]
    }

    /// Get all nodes in the graph
    #[must_use]
    pub fn nodes(&self) -> &[RebuildNode] {
        &self.nodes
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
                if dep_name_normalized == root_name_normalized
                    || self.is_transitively_affected(name, &root_name)
                {
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
            if package_name_normalized == dep_name_normalized
                && let RebuildReason::UnitDependencyInfoChanged { name, .. } = &node.reason
            {
                let name_normalized = normalize_crate_name(name);
                if name_normalized == root_name_normalized {
                    return true;
                }
                // Recursively check
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

    /// Get the number of nodes
    #[must_use]
    pub const fn len(&self) -> usize {
        self.nodes.len()
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

/// Normalize a crate name for comparison (hyphens and underscores are equivalent)
fn normalize_crate_name(name: &str) -> String {
    name.replace('-', "_")
}

/// Generate a deduplication key for a rebuild reason
fn reason_dedup_key(reason: &RebuildReason) -> String {
    match reason {
        RebuildReason::EnvVarChanged { name, .. } => format!("env:{name}"),
        RebuildReason::FileChanged { path } => format!("file:{path}"),
        RebuildReason::UnitDependencyInfoChanged { name, .. } => format!("dep:{name}"),
        RebuildReason::TargetConfigurationChanged => "config".to_string(),
        RebuildReason::ProfileConfigurationChanged => "profile".to_string(),
        RebuildReason::RustflagsChanged { .. } => "rustflags".to_string(),
        RebuildReason::FeaturesChanged { .. } => "features".to_string(),
        RebuildReason::Unknown(msg) => format!("unknown:{msg}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifies_root_causes() {
        let mut graph = RebuildGraph::new();

        graph.add_node(RebuildNode::new(
            PackageTarget::new("libz-sys v1.1.23", Some("build-script-build".to_string())),
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
    }

    #[test]
    fn finds_causal_chains() {
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

        let chains = graph.root_cause_chains();
        assert_eq!(chains.len(), 1);
        assert_eq!(chains[0].total_rebuilds(), 2);
    }
}
