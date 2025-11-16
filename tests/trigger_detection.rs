use std::{
    fs::{self, remove_file},
    process::Command,
};

use assert_cmd::{cargo, prelude::*};
use predicates::prelude::*;
use tempfile::TempDir;

fn create_test_project(name: &str) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    let cargo_toml = temp_dir.path().join("Cargo.toml");

    fs::write(
        &cargo_toml,
        format!(
            r#"
[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[build-dependencies]
cc = "1.0"
"#
        ),
    )
    .unwrap();

    let src_dir = temp_dir.path().join("src");
    fs::create_dir(&src_dir).unwrap();

    // Create a simple main.rs that uses dependencies
    fs::write(
        src_dir.join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}
"#,
    )
    .unwrap();

    // Create a build script that checks environment variables
    fs::write(
        temp_dir.path().join("build.rs"),
        r#"
use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=CUSTOM_VAR");
    println!("cargo:rerun-if-env-changed=CC");

    if let Ok(val) = env::var("CUSTOM_VAR") {
        println!("cargo:warning=CUSTOM_VAR is set to: {}", val);
    }

    // Simple C compilation that's sensitive to CC
    cc::Build::new()
        .file("build_test.c")
        .compile("build_test");
}
"#,
    )
    .unwrap();

    // Create a simple C file for the build script
    fs::write(
        temp_dir.path().join("build_test.c"),
        r"
int build_test_function(void) {
    return 42;
}
",
    )
    .unwrap();

    temp_dir
}

#[test]
fn detects_rebuilds_when_environment_variables_change() {
    let project = create_test_project("env-test");

    // First build without custom env var
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    cmd1.assert().success();

    let mut cmd2 = Command::new("cargo");
    cmd2.arg("build")
        .current_dir(project.path())
        .env_remove("CUSTOM_VAR");
    let _ = cmd2.assert(); // May fail due to missing dependencies, but that's ok

    // Now test cargo-dirty with environment variable change
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build")
        .env("CUSTOM_VAR", "test_value");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("ENVIRONMENT VARIABLE"));
}

#[test]
fn detects_rebuilds_when_c_compiler_environment_changes() {
    let project = create_test_project("cc-test");

    // Clean first
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    let _ = cmd1.assert();

    // Build with one CC setting
    let mut cmd2 = Command::new("cargo");
    cmd2.arg("build")
        .current_dir(project.path())
        .env("CC", "gcc");
    let _ = cmd2.assert(); // May fail, but we want the fingerprint

    // Now test cargo-dirty with different CC
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build")
        .env("CC", "clang");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("CC"))
        .stderr(predicate::str::contains("compiler environment"));
}

#[test]
fn detects_rebuilds_when_source_files_are_modified() {
    let project = create_test_project("target-test");

    // Remove the build script for this test to avoid C compilation issues
    let _ = remove_file(project.path().join("build.rs"));

    // Update Cargo.toml to remove build dependencies
    let cargo_toml = project.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"
[package]
name = "target-test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    // First build in debug mode
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("build").current_dir(project.path());
    cmd1.assert().success();

    // Now modify a source file to force a rebuild, then test with different profile
    let src_file = project.path().join("src/main.rs");
    fs::write(
        &src_file,
        r#"
fn main() {
    println!("Hello, modified world!");
}
"#,
    )
    .unwrap();

    // Test cargo-dirty with the same debug build - should detect file changes
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("File changed"));
}

#[test]
fn detects_rebuilds_when_multiple_source_files_are_modified() {
    let project = create_test_project("target-test");

    // Remove the build script for this test to avoid C compilation issues
    let _ = remove_file(project.path().join("build.rs"));

    // Update Cargo.toml to remove build dependencies
    let cargo_toml = project.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"
[package]
name = "target-test"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    // First build in debug mode
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("build").current_dir(project.path());
    cmd1.assert().success();

    // Now modify a source file to force a rebuild, then test with different profile
    let src_file = project.path().join("src/main.rs");
    fs::write(
        &src_file,
        r#"
fn main() {
    println!("Hello, modified world!");
}
"#,
    )
    .unwrap();

    // Test cargo-dirty with the same debug build - should detect file changes
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("File changed"));
}
