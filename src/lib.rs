//! # cargo-dirty
//!
//! A tool to analyze what causes cargo rebuilds by parsing cargo's fingerprint logs
//! and providing detailed explanations and actionable suggestions.

pub mod parsing;
pub mod analysis;

// Re-export the main public API
pub use analysis::analyze_dirty_reasons;
pub use parsing::{parse_rebuild_reason, RebuildReason};