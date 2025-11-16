use std::{fs, process::Command};

use assert_cmd::{cargo, prelude::*};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn cli_displays_help_with_usage_information() {
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains(
            "Analyze what causes cargo rebuilds",
        ))
        .stdout(predicate::str::contains("--verbose"))
        .stdout(predicate::str::contains("--command"));
}

#[test]
fn cli_displays_version_information() {
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--version");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("cargo-dirty"));
}

#[test]
fn cli_reports_error_for_invalid_project_path() {
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path").arg("/nonexistent/path");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Cargo.toml not found"));
}

#[test]
fn cli_successfully_analyzes_valid_cargo_project() {
    // Create a temporary cargo project
    let temp_dir = TempDir::new().unwrap();
    let cargo_toml = temp_dir.path().join("Cargo.toml");

    // Create a minimal Cargo.toml
    fs::write(
        &cargo_toml,
        r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#,
    )
    .unwrap();

    // Create src directory and main.rs
    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    fs::write(
        src_dir.join("main.rs"),
        "fn main() { println!(\"Hello, world!\"); }",
    )
    .unwrap();

    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path").arg(temp_dir.path());
    cmd.arg("--verbose");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Analyzing:"));
}

#[test]
fn cli_supports_different_cargo_commands() {
    let temp_dir = TempDir::new().unwrap();
    let cargo_toml = temp_dir.path().join("Cargo.toml");

    fs::write(
        &cargo_toml,
        r#"
[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();
    fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

    // Test with different cargo commands
    for command in &["check", "build", "test"] {
        let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
        cmd.arg("--path").arg(temp_dir.path());
        cmd.arg("--command").arg(command);
        cmd.arg("--verbose");

        cmd.assert()
            .success()
            .stderr(predicate::str::contains(format!(
                "Running: cargo {command}"
            )));
    }
}

#[test]
fn parser_correctly_extracts_rebuild_reasons_from_cargo_logs() {
    // Test that we can parse real cargo log output
    use cargo_dirty::{RebuildReason, parse_rebuild_reason};

    let sample_logs = vec![
        r#"    0.102058909s  INFO prepare_target{force=false package_id=libz-sys v1.1.23 target="build-script-build"}: cargo::core::compiler::fingerprint:     dirty: EnvVarChanged { name: "CC", old_value: Some("gcc"), new_value: None }"#,
        r#"dirty: UnitDependencyInfoChanged { old_name: "rusqlite", old_fingerprint: 5920731552898212716, new_name: "rusqlite", new_fingerprint: 7766129310588964256 }"#,
        r"dirty: TargetConfigurationChanged",
    ];

    let mut parsed_reasons = Vec::new();
    for log_line in sample_logs {
        if let Some(reason) = parse_rebuild_reason(log_line) {
            parsed_reasons.push(reason);
        }
    }

    assert_eq!(parsed_reasons.len(), 3);

    // Check the first reason is an env var change
    if let RebuildReason::EnvVarChanged {
        name,
        old_value,
        new_value,
    } = &parsed_reasons[0]
    {
        assert_eq!(name, "CC");
        assert_eq!(old_value, &Some("gcc".to_string()));
        assert_eq!(new_value, &None);
    } else {
        panic!("Expected EnvVarChanged, got {:?}", parsed_reasons[0]);
    }

    // Check the second reason is a dependency change
    if let RebuildReason::UnitDependencyInfoChanged {
        name,
        old_fingerprint,
        new_fingerprint,
        ..
    } = &parsed_reasons[1]
    {
        assert_eq!(name, "rusqlite");
        assert_eq!(old_fingerprint, "5920731552898212716");
        assert_eq!(new_fingerprint, "7766129310588964256");
    } else {
        panic!(
            "Expected UnitDependencyInfoChanged, got {:?}",
            parsed_reasons[1]
        );
    }

    // Check the third reason is target configuration change
    assert_eq!(parsed_reasons[2], RebuildReason::TargetConfigurationChanged);
}
