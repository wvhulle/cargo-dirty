//! # cargo-dirty
//!
//! A tool to analyze what causes cargo rebuilds by parsing cargo's fingerprint
//! logs and providing detailed explanations and actionable suggestions.

mod dirty_analyzer;
mod fingerprint_parser;
mod rebuild_graph;
mod rebuild_reason;

pub use dirty_analyzer::analyze_dirty_reasons;
