use std::fmt::{Display, Formatter, Result as FmtResult};

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum RebuildReason {
    EnvVarChanged {
        name: String,
        old_value: Option<String>,
        new_value: Option<String>,
    },
    UnitDependencyInfoChanged {
        name: String,
        old_fingerprint: String,
        new_fingerprint: String,
        context: Option<DependencyChangeContext>,
    },
    RustflagsChanged {
        old: Vec<String>,
        new: Vec<String>,
    },
    FeaturesChanged {
        old: String,
        new: String,
    },
    ProfileConfigurationChanged,
    TargetConfigurationChanged,
    FileChanged {
        path: String,
    },
    Unknown(String),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DependencyChangeContext {
    pub package_id: Option<String>,
    pub target_type: Option<String>,
    pub root_cause: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct ExplanationParts {
    title: &'static str,
    details: String,
    suggestions: Vec<String>,
    context_lines: Vec<String>,
}

impl ExplanationParts {
    fn new(title: &'static str) -> Self {
        Self {
            title,
            ..Default::default()
        }
    }

    fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = details.into();
        self
    }

    fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    fn with_suggestions(
        mut self,
        suggestions: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.suggestions
            .extend(suggestions.into_iter().map(Into::into));
        self
    }

    fn with_context(mut self, label: &str, value: &str) -> Self {
        self.context_lines.push(format!("   {label}: {value}"));
        self
    }

    fn build(self) -> String {
        let mut lines = vec![format!(" {}: {}", self.title, self.details)];

        if self.suggestions.len() == 1 {
            lines.push(format!("   Suggestion: {}", self.suggestions[0]));
        } else if !self.suggestions.is_empty() {
            lines.push("   Suggestions:".to_string());
            lines.extend(self.suggestions.into_iter().map(|s| format!("      • {s}")));
        }

        lines.extend(self.context_lines);
        lines.join("\n")
    }
}

impl RebuildReason {
    #[must_use]
    pub fn explanation(&self) -> String {
        match self {
            Self::EnvVarChanged {
                name,
                old_value,
                new_value,
            } => Self::explain_env_var_change(name, old_value.as_ref(), new_value.as_ref()),
            Self::UnitDependencyInfoChanged { name, context, .. } => {
                Self::explain_dependency_change(name, context.as_ref())
            }
            Self::RustflagsChanged { old, new } => Self::explain_rustflags_change(old, new),
            Self::FeaturesChanged { old, new } => Self::explain_features_change(old, new),
            Self::ProfileConfigurationChanged => Self::explain_profile_configuration_change(),
            Self::TargetConfigurationChanged => Self::explain_target_configuration_change(),
            Self::FileChanged { path } => Self::explain_file_change(path),
            Self::Unknown(msg) => Self::explain_unknown_reason(msg),
        }
    }

    fn explain_env_var_change(
        name: &str,
        old_value: Option<&String>,
        new_value: Option<&String>,
    ) -> String {
        let change_desc = match (old_value, new_value) {
            (Some(old), Some(new)) => format!("changed from '{old}' to '{new}'"),
            (Some(old), None) => format!("was unset (was '{old}')"),
            (None, Some(new)) => format!("was set to '{new}'"),
            (None, None) => "changed (both old and new values are None)".to_string(),
        };

        let suggestions = match name {
            "CC" | "CXX" => vec![
                "Ensure consistent compiler environment across builds",
                "This usually happens when switching between development environments",
                "Consider using direnv or similar tools to manage environment",
            ],
            "CARGO_TARGET_DIR" => vec![
                "Use consistent CARGO_TARGET_DIR or avoid setting it",
                "Consider using a fixed location for target directory",
            ],
            "RUSTFLAGS" | "RUSTC_FLAGS" => vec![
                "Ensure consistent build flags across builds",
                "Consider using cargo profiles instead of environment variables",
            ],
            "PATH" => vec![
                "Ensure consistent PATH across builds",
                "Check if new tools were added/removed from PATH",
            ],
            name if name.starts_with("CARGO_") => vec![
                "Check your cargo configuration",
                "Ensure consistent cargo environment variables",
            ],
            _ => vec!["Ensure consistent environment between builds"],
        };

        ExplanationParts::new("Environment variable")
            .with_details(format!("'{name}' {change_desc}"))
            .with_suggestions(suggestions)
            .build()
    }

    fn explain_dependency_change(name: &str, context: Option<&DependencyChangeContext>) -> String {
        let suggestions = match name {
            s if s.contains("build_script") => vec![
                "Build script dependencies were updated",
                "Environment variables affecting the build script changed",
                "System dependencies (like C libraries) were updated",
            ],
            s if s.ends_with("-sys") => vec![
                "The underlying C library was updated",
                "Library detection logic found different versions",
                "pkg-config or cmake output changed",
            ],
            _ => vec![
                "Dependency source code changed",
                "Dependency's own dependencies changed",
                "Build flags or features changed for the dependency",
            ],
        };

        let mut parts = ExplanationParts::new("Dependency")
            .with_details(format!("'{name}' was rebuilt"))
            .with_suggestions(suggestions);

        if let Some(ctx) = context {
            if let Some(root_cause) = &ctx.root_cause {
                parts = parts.with_context("Root cause", root_cause);
            }
            if let Some(package_id) = &ctx.package_id {
                parts = parts.with_context("Package", package_id);
            }
            if let Some(target_type) = &ctx.target_type {
                parts = parts.with_context("Target", target_type);
            }
        }

        parts.build()
    }

    fn explain_rustflags_change(old: &[String], new: &[String]) -> String {
        let old_flags = if old.is_empty() {
            "(none)"
        } else {
            &old.join(" ")
        };
        let new_flags = if new.is_empty() {
            "(none)"
        } else {
            &new.join(" ")
        };

        ExplanationParts::new("RUSTFLAGS CHANGED")
            .with_details(format!("{old_flags} → {new_flags}"))
            .with_suggestions([
                "Different development environments (nix-shell, different toolchains)",
                "Changed compiler optimization settings",
                "Added/removed debugging flags",
                "Modified target-specific compilation flags",
                "Use consistent RUSTFLAGS across builds or use cargo profiles",
            ])
            .build()
    }

    fn explain_features_change(old: &str, new: &str) -> String {
        ExplanationParts::new("FEATURES CHANGED")
            .with_details(format!("'{old}' → '{new}'"))
            .with_suggestions([
                "Different cargo commands (e.g., --features vs --all-features)",
                "Workspace vs individual package builds with different default features",
                "Changed feature selection in Cargo.toml dependencies",
                "Use consistent feature flags or expect rebuilds when changing features",
            ])
            .build()
    }

    fn explain_profile_configuration_change() -> String {
        ExplanationParts::new("PROFILE CONFIGURATION")
            .with_details("Build profile settings changed")
            .with_suggestions([
                "Changed [profile.dev] or [profile.release] settings in Cargo.toml",
                "Modified optimization levels, debug info, or LTO settings",
                "Updated codegen-units or incremental compilation settings",
                "Keep profile configurations consistent or expect rebuilds when changing them",
            ])
            .build()
    }

    fn explain_target_configuration_change() -> String {
        ExplanationParts::new("TARGET CONFIGURATION")
            .with_details("Build target settings changed")
            .with_suggestions([
                "Switched between debug/release mode",
                "Changed optimization level or debug settings",
                "Modified target architecture or features",
                "Updated Cargo.toml [profile] settings",
                "Use consistent build profiles or expect rebuilds when switching",
            ])
            .build()
    }

    fn explain_file_change(path: &str) -> String {
        let (suggestion, note) = if path.contains("Cargo.toml") || path.contains("Cargo.lock") {
            (
                "Project configuration changed. This triggers rebuilds of affected crates.",
                None,
            )
        } else if path.contains(".rs") {
            (
                "Source file modified",
                Some(
                    "Note: Cargo only reports top-level files. If this file imports modules, \
                     those changes are included but not separately listed.",
                ),
            )
        } else if path.contains("build.rs") {
            (
                "Build script changed. This often triggers rebuilds of multiple crates.",
                None,
            )
        } else {
            ("A file that affects the build process was modified.", None)
        };

        let mut parts = ExplanationParts::new("File changed")
            .with_details(path.to_string())
            .with_suggestion(suggestion);

        if let Some(note_text) = note {
            parts = parts.with_suggestion(note_text);
        }

        parts.build()
    }

    fn explain_unknown_reason(msg: &str) -> String {
        ExplanationParts::new("UNKNOWN REBUILD REASON")
            .with_details(msg.trim().to_string())
            .with_suggestion(
                "This might be a new type of rebuild trigger. Consider reporting this for \
                 analysis.",
            )
            .build()
    }
}

impl Display for RebuildReason {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::EnvVarChanged {
                name,
                old_value,
                new_value,
            } => {
                let change = match (old_value, new_value) {
                    (Some(old), Some(new)) => format!("'{old}' -> '{new}'"),
                    (Some(old), None) => format!("'{old}' -> unset"),
                    (None, Some(new)) => format!("unset -> '{new}'"),
                    (None, None) => "changed".to_string(),
                };
                write!(f, "env:{name} ({change})")
            }
            Self::UnitDependencyInfoChanged { name, .. } => write!(f, "dep:{name}"),
            Self::RustflagsChanged { .. } => write!(f, "rustflags changed"),
            Self::FeaturesChanged { old, new } => write!(f, "features: {old} -> {new}"),
            Self::ProfileConfigurationChanged => write!(f, "profile changed"),
            Self::TargetConfigurationChanged => write!(f, "target config changed"),
            Self::FileChanged { path } => {
                let short_path = path
                    .split('/')
                    .rev()
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join("/");
                write!(f, "file:{short_path}")
            }
            Self::Unknown(msg) => write!(f, "unknown:{msg}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn displays_environment_variable_changes() {
        let env_change = RebuildReason::EnvVarChanged {
            name: "CC".to_string(),
            old_value: Some("gcc".to_string()),
            new_value: None,
        };

        assert!(env_change.to_string().contains("env:CC"));
        assert!(env_change.to_string().contains("'gcc' -> unset"));

        let dep_change = RebuildReason::UnitDependencyInfoChanged {
            name: "rusqlite".to_string(),
            old_fingerprint: "123".to_string(),
            new_fingerprint: "456".to_string(),
            context: None,
        };

        assert!(dep_change.to_string().contains("dep:rusqlite"));

        let target_change = RebuildReason::TargetConfigurationChanged;
        assert!(target_change.to_string().contains("target config changed"));
    }

    #[test]
    fn provides_enhanced_explanations() {
        let features_change = RebuildReason::FeaturesChanged {
            old: "default".to_string(),
            new: "default,serde".to_string(),
        };

        let explanation = features_change.explanation();
        assert!(explanation.contains(" FEATURES CHANGED"));
        assert!(explanation.contains("'default' → 'default,serde'"));

        let profile_change = RebuildReason::ProfileConfigurationChanged;
        let explanation = profile_change.explanation();
        assert!(explanation.contains(" PROFILE CONFIGURATION"));
    }

    #[test]
    fn displays_context_information() {
        let dep_with_context = RebuildReason::UnitDependencyInfoChanged {
            name: "libz-sys".to_string(),
            old_fingerprint: "123".to_string(),
            new_fingerprint: "456".to_string(),
            context: Some(DependencyChangeContext {
                package_id: Some("libz-sys v1.1.23".to_string()),
                target_type: Some("build-script-build".to_string()),
                root_cause: Some("CC environment variable changed".to_string()),
            }),
        };

        let explanation = dep_with_context.explanation();
        assert!(explanation.contains("Dependency"));
        assert!(explanation.contains("libz-sys"));
        assert!(explanation.contains("Root cause: CC environment variable changed"));
        assert!(explanation.contains("Package: libz-sys v1.1.23"));
        assert!(explanation.contains("Target: build-script-build"));
    }

    #[test]
    fn formats_explanations_with_suggestions() {
        let rustflags_change = RebuildReason::RustflagsChanged {
            old: vec!["--cfg".to_string(), "test".to_string()],
            new: vec![
                "--cfg".to_string(),
                "test".to_string(),
                "-C".to_string(),
                "target-cpu=native".to_string(),
            ],
        };

        let explanation = rustflags_change.explanation();
        assert!(explanation.contains(" RUSTFLAGS CHANGED"));
        assert!(explanation.contains("--cfg test"));
        assert!(explanation.contains("target-cpu=native"));
        assert!(explanation.contains("Suggestion"));
    }
}
