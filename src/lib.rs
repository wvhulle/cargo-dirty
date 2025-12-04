//! # cargo-dirty
//!
//! A tool to analyze what causes cargo rebuilds by parsing cargo's fingerprint
//! logs and providing detailed explanations and actionable suggestions.

mod analysis;
mod parsing;

pub use analysis::analyzer::analyze_dirty_reasons;
pub use analysis::graph::{PackageTarget, RebuildGraph, RebuildNode, RootCauseChain};
pub use analysis::reporter::{
    RebuildAnalysis, RebuildSummary, RebuildTree, build_rebuild_trees, print_rebuild_analysis_json,
};
pub use parsing::parser::{ParsedRebuildEntry, parse_rebuild_entry, parse_rebuild_reason};
pub use parsing::rebuild_reason::{DependencyChangeContext, RebuildReason};
////
