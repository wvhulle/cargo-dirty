use log::{debug, info};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::parsing::parse_rebuild_reason;
use super::reporter::print_rebuild_analysis;

pub fn analyze_dirty_reasons(project_path: &PathBuf, cargo_command: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Check if Cargo.toml exists
    let cargo_toml = project_path.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(format!("Cargo.toml not found at {cargo_toml:?}").into());
    }

    info!("Found Cargo project at: {project_path:?}");
    info!("Running cargo {cargo_command} with fingerprint logging...");

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
        analyze_cargo_logs(reader)?;
    }

    Ok(())
}

pub fn analyze_cargo_logs(reader: BufReader<std::process::ChildStderr>) -> Result<(), Box<dyn std::error::Error>> {
    let mut rebuild_reasons = Vec::new();

    for line in reader.lines() {
        let line = line?;
        debug!("Cargo log: {line}");

        if line.contains("fingerprint") && line.contains("dirty:") {
            info!("Rebuild trigger: {line}");
            if let Some(reason) = parse_rebuild_reason(&line) {
                rebuild_reasons.push(reason);
            }
        }

        if line.contains("recompiling") || line.contains("compiling") {
            info!("Compilation: {line}");
        }
    }

    if rebuild_reasons.is_empty() {
        info!("🎉 No rebuild reasons detected - this suggests an incremental build with no changes!");
        info!("💡 This is good! It means cargo's incremental compilation is working effectively.");
    } else {
        print_rebuild_analysis(&rebuild_reasons);
    }

    Ok(())
}