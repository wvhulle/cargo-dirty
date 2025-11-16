//! # cargo-dirty
//!
//! A tool to analyze what causes cargo rebuilds by parsing cargo's fingerprint
//! logs and providing detailed explanations and actionable suggestions.

pub mod analysis;
pub mod parsing;

pub use analysis::analyze_dirty_reasons;
pub use parsing::{RebuildReason, parse_rebuild_reason};
