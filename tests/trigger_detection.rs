use std::{
    fs::{self, remove_file},
    path::Path,
    process::{Command, Stdio},
};

use assert_cmd::{cargo, prelude::*};
use cargo_dirty::{RebuildGraph, RebuildNode, parse_rebuild_entry};
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

    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    cmd1.assert().success();

    let mut cmd2 = Command::new("cargo");
    cmd2.arg("build")
        .current_dir(project.path())
        .env_remove("CUSTOM_VAR");
    let _ = cmd2.assert();

    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--command")
        .arg("build")
        .env("CUSTOM_VAR", "test_value");

    let output = cmd.assert().success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("env:CUSTOM_VAR"),
        "Expected stderr to contain 'env:CUSTOM_VAR', got: {stderr}"
    );
}

#[test]
fn detects_rebuilds_when_c_compiler_environment_changes() {
    let project = create_test_project("cc-test");

    let mut cmd1 = Command::new("cargo");
    cmd1.arg("clean").current_dir(project.path());
    let _ = cmd1.assert();

    let mut cmd2 = Command::new("cargo");
    cmd2.arg("build")
        .current_dir(project.path())
        .env("CC", "gcc");
    let _ = cmd2.assert();

    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--command")
        .arg("build")
        .env("CC", "clang");

    let output = cmd.assert().success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("env:CC"),
        "Expected stderr to contain 'env:CC', got: {stderr}"
    );
}

#[test]
fn detects_rebuilds_when_source_files_are_modified() {
    let project = create_test_project("target-test");

    let _ = remove_file(project.path().join("build.rs"));

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

    let mut cmd1 = Command::new("cargo");
    cmd1.arg("build").current_dir(project.path());
    cmd1.assert().success();

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

    let mut cmd = Command::new(cargo::cargo_bin!("cargo-dirty"));
    cmd.arg("--path")
        .arg(project.path())
        .arg("--command")
        .arg("build");

    let output = cmd.assert().success();
    let stderr = String::from_utf8_lossy(&output.get_output().stderr);
    assert!(
        stderr.contains("file:") && stderr.contains("main.rs"),
        "Expected stderr to contain 'file:' and 'main.rs', got: {stderr}"
    );
}

/// Create a workspace with multiple crates to test dependency cascades
fn create_workspace_with_dependencies() -> TempDir {
    let temp_dir = TempDir::new().unwrap();

    // Create workspace Cargo.toml
    fs::write(
        temp_dir.path().join("Cargo.toml"),
        r#"
[workspace]
members = ["lib-a", "lib-b", "app"]
resolver = "2"
"#,
    )
    .unwrap();

    // Create lib-a (root library)
    let lib_a_dir = temp_dir.path().join("lib-a");
    fs::create_dir_all(lib_a_dir.join("src")).unwrap();
    fs::write(
        lib_a_dir.join("Cargo.toml"),
        r#"
[package]
name = "lib-a"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        lib_a_dir.join("src/lib.rs"),
        r#"
pub fn greet() -> &'static str {
    "Hello from lib-a"
}
"#,
    )
    .unwrap();

    // Create lib-b (depends on lib-a)
    let middle_lib_dir = temp_dir.path().join("lib-b");
    fs::create_dir_all(middle_lib_dir.join("src")).unwrap();
    fs::write(
        middle_lib_dir.join("Cargo.toml"),
        r#"
[package]
name = "lib-b"
version = "0.1.0"
edition = "2021"

[dependencies]
lib-a = { path = "../lib-a" }
"#,
    )
    .unwrap();
    fs::write(
        middle_lib_dir.join("src/lib.rs"),
        r#"
pub fn message() -> String {
    format!("lib-b says: {}", lib_a::greet())
}
"#,
    )
    .unwrap();

    // Create app (depends on lib-b)
    let app_dir = temp_dir.path().join("app");
    fs::create_dir_all(app_dir.join("src")).unwrap();
    fs::write(
        app_dir.join("Cargo.toml"),
        r#"
[package]
name = "app"
version = "0.1.0"
edition = "2021"

[dependencies]
lib-b = { path = "../lib-b" }
"#,
    )
    .unwrap();
    fs::write(
        app_dir.join("src/main.rs"),
        r#"
fn main() {
    println!("{}", lib_b::message());
}
"#,
    )
    .unwrap();

    temp_dir
}

/// Run cargo build with fingerprint logging and collect the log lines
fn collect_cargo_fingerprint_logs(project_path: &Path) -> Vec<String> {
    let output = Command::new("cargo")
        .arg("build")
        .current_dir(project_path)
        .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to run cargo build");

    let stderr = String::from_utf8_lossy(&output.stderr);
    stderr
        .lines()
        .filter(|line| {
            line.contains("fingerprint") && (line.contains("dirty:") || line.contains("stale:"))
        })
        .map(String::from)
        .collect()
}

/// Build trees from cargo fingerprint logs (JSON-serializable)
fn build_trees_from_logs(log_lines: &[String]) -> Vec<cargo_dirty::RebuildTree> {
    let mut graph = RebuildGraph::new();
    for line in log_lines {
        if let Some(entry) = parse_rebuild_entry(line) {
            graph.add_node(RebuildNode::new(entry.package, entry.reason));
        }
    }
    cargo_dirty::build_rebuild_trees(&graph)
}

/// Strict integration test that verifies the JSON tree structure of the rebuild
/// analysis.
#[test]
fn json_tree_structure_is_valid_for_workspace_rebuild() {
    let workspace = create_workspace_with_dependencies();

    let mut build_cmd = Command::new("cargo");
    build_cmd.arg("build").current_dir(workspace.path());
    build_cmd.assert().success();

    let lib_a_src = workspace.path().join("lib-a/src/lib.rs");
    fs::write(
        &lib_a_src,
        r#"
pub fn greet() -> &'static str {
    "Hello from modified lib-a!"
}
"#,
    )
    .unwrap();

    let log_lines = collect_cargo_fingerprint_logs(workspace.path());
    let trees = build_trees_from_logs(&log_lines);

    let json = serde_json::to_string_pretty(&trees).expect("JSON serialization should succeed");
    let parsed: serde_json::Value =
        serde_json::from_str(&json).expect("JSON should be valid and parseable");

    // JSON should be an array of root cause trees
    let root_array = parsed.as_array().expect("JSON should be an array");
    assert!(
        !root_array.is_empty(),
        "Should have at least one root cause"
    );

    // Each root cause should have package, reason, and optional cascades
    for root in root_array {
        assert!(root.get("package").is_some(), "Root should have package");
        assert!(root.get("reason").is_some(), "Root should have reason");

        // Root causes should NOT have "dep:" prefix (they're not dependency changes)
        let reason = root["reason"].as_str().unwrap();
        assert!(
            !reason.starts_with("dep:"),
            "Root cause should not be a dependency change: {reason}"
        );

        // Cascades (if present) should all be dependency changes
        if let Some(cascades) = root.get("cascades") {
            let cascades_arr = cascades.as_array().unwrap();
            for cascade in cascades_arr {
                let cascade_reason = cascade["reason"].as_str().unwrap();
                assert!(
                    cascade_reason.starts_with("dep:"),
                    "Cascade should be a dependency change: {cascade_reason}"
                );
            }
        }
    }

    // At least one root cause should mention lib-a
    let has_lib_a_root = root_array
        .iter()
        .any(|r| r["package"].as_str().is_some_and(|p| p.contains("lib-a")));
    assert!(
        has_lib_a_root,
        "lib-a should be identified as a root cause since we modified it"
    );
}
