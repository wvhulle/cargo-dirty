use serde::Serialize;

use crate::{
    rebuild_graph::{RebuildGraph, RebuildNode, RootCauseChain, reason_dedup_key},
    rebuild_reason::RebuildReason,
};

/// Tree node representing a rebuild cause with nested cascades
#[derive(Debug, Serialize)]
pub struct RebuildTree {
    pub package: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cascades: Vec<RebuildTree>,
}

impl RebuildTree {
    fn from_chain(chain: &RootCauseChain) -> Self {
        Self {
            package: chain.root_cause.package.package_id.clone(),
            reason: reason_dedup_key(&chain.root_cause.reason),
            cascades: chain
                .affected_packages
                .iter()
                .map(|node| Self {
                    package: node.package.package_id.clone(),
                    reason: reason_dedup_key(&node.reason),
                    cascades: Vec::new(),
                })
                .collect(),
        }
    }
}

/// Simple JSON output: array of root cause trees
pub fn build_rebuild_trees(graph: &RebuildGraph) -> Vec<RebuildTree> {
    graph
        .root_cause_chains()
        .iter()
        .map(RebuildTree::from_chain)
        .collect()
}

/// Print JSON representation of the rebuild analysis to stdout
///
/// # Errors
/// Returns error if serialization fails
pub fn print_rebuild_analysis_json(graph: &RebuildGraph) -> Result<(), serde_json::Error> {
    let trees = build_rebuild_trees(graph);
    let json = serde_json::to_string_pretty(&trees)?;
    println!("{json}");
    Ok(())
}

/// JSON-serializable representation of the rebuild analysis (detailed version)
#[derive(Debug, Serialize)]
pub struct RebuildAnalysis {
    pub total_rebuilds: usize,
    pub root_cause_count: usize,
    pub cascade_count: usize,
    pub root_causes: Vec<RebuildNode>,
    pub root_cause_chains: Vec<RootCauseChain>,
    pub summary: RebuildSummary,
}

/// Summary breakdown by rebuild trigger type
#[derive(Debug, Serialize)]
pub struct RebuildSummary {
    pub env_vars: usize,
    pub dependencies: usize,
    pub target_configs: usize,
    pub files: usize,
}

impl RebuildAnalysis {
    /// Build analysis from a `RebuildGraph`
    #[must_use]
    pub fn from_graph(graph: &RebuildGraph) -> Self {
        let root_causes: Vec<_> = graph.root_causes().iter().copied().cloned().collect();
        let root_cause_count = root_causes.len();
        let total_rebuilds = graph.len();
        let cascade_count = total_rebuilds - root_cause_count;

        let nodes = graph.nodes();
        let summary = RebuildSummary {
            env_vars: nodes
                .iter()
                .filter(|n| matches!(n.reason, RebuildReason::EnvVarChanged { .. }))
                .count(),
            dependencies: nodes
                .iter()
                .filter(|n| matches!(n.reason, RebuildReason::UnitDependencyInfoChanged { .. }))
                .count(),
            target_configs: nodes
                .iter()
                .filter(|n| matches!(n.reason, RebuildReason::TargetConfigurationChanged))
                .count(),
            files: nodes
                .iter()
                .filter(|n| matches!(n.reason, RebuildReason::FileChanged { .. }))
                .count(),
        };

        Self {
            total_rebuilds,
            root_cause_count,
            cascade_count,
            root_causes,
            root_cause_chains: graph.root_cause_chains(),
            summary,
        }
    }

    /// Serialize to JSON string
    ///
    /// # Errors
    /// Returns error if serialization fails
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

pub fn print_rebuild_analysis(graph: &RebuildGraph) {
    let root_causes = graph.root_causes();

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
