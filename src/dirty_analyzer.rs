use std::{
    env,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{ChildStderr, Command, Stdio},
};

use clap::Parser;
use log::{debug, info};

use crate::{
    AnalyzerError,
    fingerprint_parser::parse_rebuild_entry,
    rebuild_graph::{RebuildGraph, RebuildNode},
};

#[derive(Parser, Debug)]
#[command(author, version, about = "Analyze what causes cargo rebuilds", long_about = None)]
pub struct Config {
    #[arg(short, long, help = "Path to cargo project", default_value = ".")]
    path: PathBuf,

    #[arg(short, long, help = "Verbose output")]
    verbose: bool,

    #[arg(long, help = "Output analysis as JSON")]
    json: bool,

    #[arg(long, help = "Cargo command to analyze", default_value = "check")]
    command: String,

    #[arg(help = "Additional arguments to pass to cargo", last = true)]
    cargo_args: Vec<String>,
}

impl Config {
    #[must_use]
    pub fn parse_args() -> Self {
        if env::args().nth(1).as_deref() == Some("frequent") {
            Self::parse_from(env::args().take(1).chain(env::args().skip(2)))
        } else {
            Self::parse()
        }
    }

    pub fn init_logging(&self) {
        if self.verbose {
            env_logger::Builder::from_default_env()
                .filter_level(log::LevelFilter::Debug)
                .init();
        } else {
            env_logger::init();
        }
    }

    fn cargo_command(&self) -> String {
        if self.cargo_args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.cargo_args.join(" "))
        }
    }

    pub fn run(&self) -> Result<(), AnalyzerError> {
        let cargo_command = self.cargo_command();

        let cargo_toml = self.path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Err(AnalyzerError::CargoTomlNotFound(cargo_toml));
        }

        info!(
            "Analyzing output of `cargo {}` on project {}",
            cargo_command,
            self.path.display()
        );

        let args: Vec<&str> = cargo_command.split_whitespace().collect();
        let (cmd, cmd_args) = args.split_first().ok_or(AnalyzerError::EmptyCommand)?;

        let output = Command::new("cargo")
            .arg(cmd)
            .args(cmd_args)
            .current_dir(&self.path)
            .env("CARGO_LOG", "cargo::core::compiler::fingerprint=info")
            .env("RUST_LOG", "debug")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        if let Some(stderr) = output.stderr {
            let reader = BufReader::new(stderr);
            self.analyze_logs(reader)?;
        }

        Ok(())
    }

    fn analyze_logs(&self, reader: BufReader<ChildStderr>) -> Result<(), AnalyzerError> {
        let mut graph = RebuildGraph::new();

        for line in reader.lines() {
            let line = line?;
            debug!("Cargo log: {line}");

            if line.contains("fingerprint") && (line.contains("dirty:") || line.contains("stale:"))
            {
                debug!("Rebuild trigger detected: {line}");
                if let Some(entry) = parse_rebuild_entry(&line) {
                    graph.add_node(RebuildNode::new(entry.package, entry.reason));
                }
            }

            if line.contains("recompiling") || line.contains("compiling") {
                debug!("Compilation: {line}");
            }
        }

        if self.json {
            println!("{}", graph.to_json()?);
        } else {
            let root_causes = graph.root_causes();

            if root_causes.is_empty() {
                println!("No rebuild triggers detected.");
            } else {
                println!(
                    "\n{} root cause{}:",
                    root_causes.len(),
                    if root_causes.len() == 1 { "" } else { "s" }
                );

                for root in &root_causes {
                    println!("  {} {}", root.package, root.reason);
                }
            }
        }

        Ok(())
    }
}
