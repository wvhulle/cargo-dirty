use cargo_dirty::{parsing::DependencyChangeContext, RebuildReason};

#[test]
fn rebuild_reason_displays_environment_variable_changes() {
    let env_change = RebuildReason::EnvVarChanged {
        name: "CC".to_string(),
        old_value: Some("gcc".to_string()),
        new_value: None,
    };

    assert!(env_change.to_string().contains("Environment variable"));
    assert!(env_change.to_string().contains("CC"));

    let dep_change = RebuildReason::UnitDependencyInfoChanged {
        name: "rusqlite".to_string(),
        old_fingerprint: "123".to_string(),
        new_fingerprint: "456".to_string(),
        context: None,
    };

    assert!(dep_change.to_string().contains("Dependency"));
    assert!(dep_change.to_string().contains("rusqlite"));

    let target_change = RebuildReason::TargetConfigurationChanged;
    assert!(target_change.to_string().contains("TARGET CONFIGURATION"));
}

#[test]
fn rebuild_reason_provides_enhanced_explanations() {
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
fn dependency_change_displays_context_information() {
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
