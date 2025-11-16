use cargo_dirty::{parse_rebuild_reason, RebuildReason};

#[test]
fn nom_parser_handles_env_var_changed_with_some_to_none() {
    let log_line =
        r#"dirty: EnvVarChanged { name: "CC", old_value: Some("gcc"), new_value: None }"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(
        result,
        Some(RebuildReason::EnvVarChanged {
            name: "CC".to_string(),
            old_value: Some("gcc".to_string()),
            new_value: None,
        })
    );
}

#[test]
fn nom_parser_handles_env_var_changed_with_none_to_some() {
    let log_line =
        r#"dirty: EnvVarChanged { name: "RUST_LOG", old_value: None, new_value: Some("debug") }"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(
        result,
        Some(RebuildReason::EnvVarChanged {
            name: "RUST_LOG".to_string(),
            old_value: None,
            new_value: Some("debug".to_string()),
        })
    );
}

#[test]
fn nom_parser_handles_env_var_changed_with_complex_paths() {
    let log_line = r#"dirty: EnvVarChanged { name: "PATH", old_value: Some("/usr/bin:/bin"), new_value: Some("/nix/store/abc:/usr/bin:/bin") }"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(
        result,
        Some(RebuildReason::EnvVarChanged {
            name: "PATH".to_string(),
            old_value: Some("/usr/bin:/bin".to_string()),
            new_value: Some("/nix/store/abc:/usr/bin:/bin".to_string()),
        })
    );
}

#[test]
fn nom_parser_handles_unit_dependency_info_changed() {
    let log_line = r#"dirty: UnitDependencyInfoChanged { old_name: "rusqlite", old_fingerprint: 5920731552898212716, new_name: "rusqlite", new_fingerprint: 7766129310588964256 }"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(
        result,
        Some(RebuildReason::UnitDependencyInfoChanged {
            name: "rusqlite".to_string(),
            old_fingerprint: "5920731552898212716".to_string(),
            new_fingerprint: "7766129310588964256".to_string(),
            context: None,
        })
    );
}

#[test]
fn nom_parser_handles_target_configuration_changed() {
    let log_line = r"dirty: TargetConfigurationChanged";
    let result = parse_rebuild_reason(log_line);

    assert_eq!(result, Some(RebuildReason::TargetConfigurationChanged));
}

#[test]
fn nom_parser_handles_fs_status_outdated_with_file_change() {
    let log_line = r#"dirty: FsStatusOutdated(StaleItem(ChangedFile { reference: "/tmp/.tmp6t5LHE/target/debug/.fingerprint/target-test-d08e845c3c592b51/dep-bin-target-test", reference_mtime: FileTime { seconds: 1763310414, nanos: 599971397 }, stale: "/tmp/.tmp6t5LHE/src/main.rs", stale_mtime: FileTime { seconds: 1763310414, nanos: 663971117 } }))"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(
        result,
        Some(RebuildReason::FileChanged {
            path: "/tmp/.tmp6t5LHE/src/main.rs".to_string(),
        })
    );
}

#[test]
fn nom_parser_extracts_reason_from_complex_cargo_log_line() {
    let log_line = r#"    0.102058909s  INFO prepare_target{force=false package_id=libz-sys v1.1.23 target="build-script-build"}: cargo::core::compiler::fingerprint:     dirty: EnvVarChanged { name: "CC", old_value: Some("gcc"), new_value: None }"#;
    let result = parse_rebuild_reason(log_line);

    if let Some(RebuildReason::EnvVarChanged {
        name,
        old_value,
        new_value,
    }) = result
    {
        assert_eq!(name, "CC");
        assert_eq!(old_value, Some("gcc".to_string()));
        assert_eq!(new_value, None);
    } else {
        panic!("Expected EnvVarChanged, got {result:?}");
    }
}

#[test]
fn nom_parser_returns_none_for_unknown_dirty_reason_format() {
    let log_line = r#"dirty: SomeUnknownReason { data: "value" }"#;
    let result = parse_rebuild_reason(log_line);

    assert_eq!(result, None);
}

#[test]
fn nom_parser_returns_none_for_lines_without_dirty_marker() {
    let log_line =
        r"    0.102058909s  INFO cargo::core::compiler::fingerprint: some other log message";
    let result = parse_rebuild_reason(log_line);

    assert_eq!(result, None);
}

#[test]
fn nom_parser_handles_malformed_input_gracefully() {
    let malformed_lines = vec![
        r#"dirty: EnvVarChanged { name: "CC", old_value: Some("gcc")"#, // Missing closing brace
        r#"dirty: EnvVarChanged { name: CC", old_value: Some("gcc"), new_value: None }"#, // Missing quote
        r#"dirty: UnitDependencyInfoChanged { old_name: "rusqlite""#, // Incomplete
        r"dirty:",                                                    // Just the marker
        r"",                                                          // Empty string
    ];

    for line in malformed_lines {
        let result = parse_rebuild_reason(line);
        assert_eq!(result, None, "Expected None for malformed line: {line}");
    }
}
