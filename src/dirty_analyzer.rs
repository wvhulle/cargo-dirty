use std::{
    error::Error,
    io::{BufRead, BufReader},
    path::Path,
    process::{ChildStderr, Command, Stdio},
};

use log::debug;

use crate::{
    fingerprint_parser::parse_rebuild_entry,
    rebuild_graph::{RebuildGraph, RebuildNode},
    rebuild_reporter::{print_rebuild_analysis, print_rebuild_analysis_json},
};

/// Analyzes dirty reasons for cargo rebuilds
///
/// # Errors
///
/// Returns an error if:
/// - Cargo.toml is not found at the project path
/// - The cargo command fails to spawn
/// - Log analysis fails
pub fn analyze_dirty_reasons(
    project_path: &Path,
    cargo_command: &str,
    json_output: bool,
) -> Result<(), Box<dyn Error>> {
    let cargo_toml = project_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(format!("Cargo.toml not found at {}", cargo_toml.display()).into());
    }

    eprintln!("Analyzing: {}", project_path.display());
    eprintln!("Running: cargo {cargo_command}\n");

    let args: Vec<&str> = cargo_command.split_whitespace().collect();
    let (cmd, cmd_args) = args.split_first().ok_or("Empty cargo command")?;

    let output = Command::new("cargo")
        .arg(cmd)
        .args(cmd_args)
        .current_dir(project_path)
        .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
        .env("RUST_LOG", "debug")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(stderr) = output.stderr {
        let reader = BufReader::new(stderr);
        analyze_cargo_logs(reader, json_output)?;
    }

    Ok(())
}

/// Analyzes cargo log output for rebuild reasons and builds a causality graph
///
/// # Errors
///
/// Returns an error if reading from the stderr stream fails
pub fn analyze_cargo_logs(
    reader: BufReader<ChildStderr>,
    json_output: bool,
) -> Result<(), Box<dyn Error>> {
    let mut graph = RebuildGraph::new();

    for line in reader.lines() {
        let line = line?;
        debug!("Cargo log: {line}");

        if line.contains("fingerprint") && (line.contains("dirty:") || line.contains("stale:")) {
            debug!("Rebuild trigger detected: {line}");
            if let Some(entry) = parse_rebuild_entry(&line) {
                graph.add_node(RebuildNode::new(entry.package, entry.reason));
            }
        }

        if line.contains("recompiling") || line.contains("compiling") {
            debug!("Compilation: {line}");
        }
    }

    if graph.is_empty() {
        if json_output {
            println!("[]");
        } else {
            eprintln!("No rebuild reasons detected - incremental build with no changes");
            eprintln!("Cargo's incremental compilation is working effectively");
        }
    } else if json_output {
        print_rebuild_analysis_json(&graph)?;
    } else {
        print_rebuild_analysis(&graph);
    }

    Ok(())
}
