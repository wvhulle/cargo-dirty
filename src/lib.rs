//! # cargo-dirty
//!
//! A tool to analyze what causes cargo rebuilds by parsing cargo's fingerprint
//! logs and providing detailed explanations and actionable suggestions.

mod dirty_analyzer;
mod fingerprint_parser;
mod rebuild_graph;
mod rebuild_reason;
mod rebuild_reporter;

pub use dirty_analyzer::analyze_dirty_reasons;
pub use fingerprint_parser::{ParsedRebuildEntry, parse_rebuild_entry, parse_rebuild_reason};
pub use rebuild_graph::{PackageTarget, RebuildGraph, RebuildNode, RootCauseChain};
pub use rebuild_reason::{DependencyChangeContext, RebuildReason};
pub use rebuild_reporter::{
    RebuildAnalysis, RebuildSummary, RebuildTree, build_rebuild_trees, print_rebuild_analysis_json,
};
