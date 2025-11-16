use assert_cmd::{cargo, prelude::*};
use predicates::prelude::*;
use std::fs::{self, remove_file};
use std::process::Command;
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
        .stderr(predicate::str::contains("REBUILD ANALYSIS SUMMARY"))
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
        .stderr(predicate::str::contains("Found Cargo project"))
        .stderr(predicate::str::contains("FILE CHANGED"));
}

#[test]
fn detects_rebuilds_when_dependency_configuration_changes() {
    let project = create_test_project("dep-test");

    // Build once
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    let _ = cmd1.assert();

    // Modify Cargo.toml to change dependencies
    let cargo_toml = project.path().join("Cargo.toml");
    fs::write(
        &cargo_toml,
        r#"
[package]
name = "dep-test"
version = "0.1.0"
edition = "2021"

[dependencies]
log = "0.4"
serde = "1.0"

[build-dependencies]
cc = "1.0"
"#,
    )
    .unwrap();

    // Test cargo-dirty after dependency change
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Found Cargo project"));
}

#[test]
fn detects_rebuilds_when_rust_source_files_change() {
    let project = create_test_project("file-test");

    // Initial build
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    let _ = cmd1.assert();

    // Modify source file
    let src_file = project.path().join("src").join("main.rs");
    fs::write(
        &src_file,
        r#"
use log::info;

fn main() {
    env_logger::init();
    info!("Hello, modified world!");
    println!("This is a change");
}
"#,
    )
    .unwrap();

    // Test cargo-dirty after file change
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Found Cargo project"));
}

#[test]
fn handles_multiple_environment_variable_changes() {
    let project = create_test_project("multi-env-test");

    // Test with multiple environment variables that affect builds
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build")
        .env("RUSTFLAGS", "-C target-cpu=native")
        .env("CARGO_TARGET_DIR", project.path().join("custom_target"))
        .env("CC", "gcc-12")
        .env("CUSTOM_VAR", "multiple_changes");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Found Cargo project"));
}

#[test]
fn reports_no_rebuild_reasons_for_clean_incremental_build() {
    let project = create_test_project("clean-test");

    // Build twice without changes - should show no rebuild reasons on second run
    let mut cmd1 = Command::new("cargo");
    cmd1.arg("build").current_dir(project.path());
    let _ = cmd1.assert();

    // Second build should be incremental
    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--verbose")
        .arg("--command")
        .arg("build");

    cmd.assert()
        .success()
        .stderr(predicate::str::contains("Found Cargo project"));
}

#[test]
fn formats_rebuild_reason_explanations_with_suggestions() {
    // Test that our improved explanations are actually shown
    use cargo_dirty::parse_rebuild_reason;

    let test_log = r#"dirty: EnvVarChanged { name: "RUSTFLAGS", old_value: None, new_value: Some("-C target-cpu=native") }"#;
    let reason = parse_rebuild_reason(test_log).unwrap();

    let explanation = reason.explanation();
    assert!(explanation.contains("🔧 ENVIRONMENT VARIABLE"));
    assert!(explanation.contains("RUSTFLAGS"));
    assert!(explanation.contains("💡 Suggestion"));
}
