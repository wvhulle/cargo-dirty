use std::{fs, process::Command};

use assert_cmd::{cargo, prelude::*};
use predicates::prelude::*;
use tempfile::TempDir;

#[test]
fn cli_reports_error_for_invalid_project_path() {
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path").arg("/nonexistent/path");

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("Cargo.toml not found"));
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

        cmd.assert()
            .success()
            .stderr(predicate::str::contains(format!(
                "Running: cargo {command}"
            )));
    }
}
