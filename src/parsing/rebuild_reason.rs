#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
pub struct DependencyChangeContext {
    pub package_id: Option<String>,
    pub target_type: Option<String>,
    pub root_cause: Option<String>,
}

impl RebuildReason {
    #[must_use]
    pub fn explanation(&self) -> String {
        match self {
            Self::EnvVarChanged { name, old_value, new_value } => {
                let change_desc = match (old_value, new_value) {
                    (Some(old), Some(new)) => format!("changed from '{old}' to '{new}'"),
                    (Some(old), None) => format!("was unset (was '{old}')"),
                    (None, Some(new)) => format!("was set to '{new}'"),
                    (None, None) => "changed (both old and new values are None)".to_string(),
                };

                let suggestion = match name.as_str() {
                    "CC" | "CXX" => "This usually happens when switching between development environments (e.g., nix-shell, different toolchains). Ensure consistent compiler environment.",
                    "CARGO_TARGET_DIR" => "Target directory location changed. Use consistent CARGO_TARGET_DIR or avoid setting it.",
                    "RUSTFLAGS" | "RUSTC_FLAGS" => "Rust compiler flags changed. Ensure consistent build flags across builds.",
                    "PATH" => "PATH environment variable changed, affecting which tools cargo finds. Ensure consistent PATH.",
                    _ if name.starts_with("CARGO_") => "Cargo-specific environment variable changed. Check your cargo configuration.",
                    _ => "Environment variable affects build process. Ensure consistent environment between builds.",
                };

                format!("🔧 ENVIRONMENT VARIABLE: '{name}' {change_desc}\n   💡 Suggestion: {suggestion}")
            }
            Self::UnitDependencyInfoChanged { name, context, .. } => {
                let base_suggestion = match name.as_str() {
                    s if s.contains("build_script") => "Build script output changed. This often happens when:\n      • Build script dependencies were updated\n      • Environment variables affecting the build script changed\n      • System dependencies (like C libraries) were updated",
                    s if s.ends_with("-sys") => "System library binding changed. This often means:\n      • The underlying C library was updated\n      • Library detection logic found different versions\n      • pkg-config or cmake output changed",
                    _ => "Dependency was rebuilt, causing this crate to rebuild. Common causes:\n      • Dependency source code changed\n      • Dependency's own dependencies changed\n      • Build flags or features changed for the dependency",
                };

                let mut explanation = format!("📦 DEPENDENCY: '{name}' was rebuilt\n   💡 Why this happens: {base_suggestion}");

                if let Some(ctx) = context {
                    if let Some(root_cause) = &ctx.root_cause {
                        explanation.push_str(&format!("\n   🔍 Root cause: {root_cause}"));
                    }
                    if let Some(package_id) = &ctx.package_id {
                        explanation.push_str(&format!("\n   📋 Package: {package_id}"));
                    }
                    if let Some(target_type) = &ctx.target_type {
                        explanation.push_str(&format!("\n   🎯 Target: {target_type}"));
                    }
                }

                explanation
            }
            Self::RustflagsChanged { old, new } => {
                let old_flags = if old.is_empty() { "(none)".to_string() } else { old.join(" ") };
                let new_flags = if new.is_empty() { "(none)".to_string() } else { new.join(" ") };

                format!(
                    "🚩 RUSTFLAGS CHANGED: {} → {}\n   💡 Common causes:\n      • Different development environments (nix-shell, different toolchains)\n      • Changed compiler optimization settings\n      • Added/removed debugging flags\n      • Modified target-specific compilation flags\n   💡 Suggestion: Use consistent RUSTFLAGS across builds or use cargo profiles",
                    old_flags, new_flags
                )
            }
            Self::FeaturesChanged { old, new } => {
                format!(
                    "🔧 FEATURES CHANGED: '{}' → '{}'\n   💡 Common causes:\n      • Different cargo commands (e.g., --features vs --all-features)\n      • Workspace vs individual package builds with different default features\n      • Changed feature selection in Cargo.toml dependencies\n   💡 Suggestion: Use consistent feature flags or expect rebuilds when changing features",
                    old, new
                )
            }
            Self::ProfileConfigurationChanged => {
                "📊 PROFILE CONFIGURATION: Build profile settings changed\n   💡 Common causes:\n      • Changed [profile.dev] or [profile.release] settings in Cargo.toml\n      • Modified optimization levels, debug info, or LTO settings\n      • Updated codegen-units or incremental compilation settings\n   💡 Suggestion: Keep profile configurations consistent or expect rebuilds when changing them".to_string()
            }
            Self::TargetConfigurationChanged => {
                "⚙️  TARGET CONFIGURATION: Build target settings changed\n   💡 Common causes:\n      • Switched between debug/release mode\n      • Changed optimization level or debug settings\n      • Modified target architecture or features\n      • Updated Cargo.toml [profile] settings\n   💡 Suggestion: Use consistent build profiles or expect rebuilds when switching".to_string()
            }
            Self::FileChanged { path } => {
                let suggestion = if path.contains("Cargo.toml") || path.contains("Cargo.lock") {
                    "Project configuration changed. This triggers rebuilds of affected crates."
                } else if path.contains(".rs") {
                    "Source file was modified. This is expected when code changes."
                } else if path.contains("build.rs") {
                    "Build script changed. This often triggers rebuilds of multiple crates."
                } else {
                    "A file that affects the build process was modified."
                };

                format!("📝 FILE CHANGED: {path}\n   💡 Explanation: {suggestion}")
            }
            Self::Unknown(msg) => {
                format!("❓ UNKNOWN REBUILD REASON: {}\n   💡 This might be a new type of rebuild trigger. Consider reporting this for analysis.", msg.trim())
            }
        }
    }
}

impl std::fmt::Display for RebuildReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.explanation())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rebuild_reason_display() {
        let env_change = RebuildReason::EnvVarChanged {
            name: "CC".to_string(),
            old_value: Some("gcc".to_string()),
            new_value: None,
        };

        assert!(env_change.to_string().contains("🔧 ENVIRONMENT VARIABLE"));
        assert!(env_change.to_string().contains("CC"));

        let dep_change = RebuildReason::UnitDependencyInfoChanged {
            name: "rusqlite".to_string(),
            old_fingerprint: "123".to_string(),
            new_fingerprint: "456".to_string(),
            context: None,
        };

        assert!(dep_change.to_string().contains("📦 DEPENDENCY"));
        assert!(dep_change.to_string().contains("rusqlite"));

        let target_change = RebuildReason::TargetConfigurationChanged;
        assert!(target_change.to_string().contains("⚙️  TARGET CONFIGURATION"));
    }

    #[test]
    fn test_enhanced_explanations() {
        let rustflags_change = RebuildReason::RustflagsChanged {
            old: vec!["--cfg".to_string(), "test".to_string()],
            new: vec!["--cfg".to_string(), "test".to_string(), "-C".to_string(), "target-cpu=native".to_string()],
        };

        let explanation = rustflags_change.explanation();
        assert!(explanation.contains("🚩 RUSTFLAGS CHANGED"));
        assert!(explanation.contains("--cfg test"));
        assert!(explanation.contains("target-cpu=native"));

        let features_change = RebuildReason::FeaturesChanged {
            old: "default".to_string(),
            new: "default,serde".to_string(),
        };

        let explanation = features_change.explanation();
        assert!(explanation.contains("🔧 FEATURES CHANGED"));
        assert!(explanation.contains("'default' → 'default,serde'"));

        let profile_change = RebuildReason::ProfileConfigurationChanged;
        let explanation = profile_change.explanation();
        assert!(explanation.contains("📊 PROFILE CONFIGURATION"));
    }

    #[test]
    fn test_dependency_context_display() {
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
        assert!(explanation.contains("📦 DEPENDENCY"));
        assert!(explanation.contains("libz-sys"));
        assert!(explanation.contains("🔍 Root cause: CC environment variable changed"));
        assert!(explanation.contains("📋 Package: libz-sys v1.1.23"));
        assert!(explanation.contains("🎯 Target: build-script-build"));
    }
}
