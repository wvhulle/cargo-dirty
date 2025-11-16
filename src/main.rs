use cargo_dirty::analyze_dirty_reasons;
use clap::Parser;
use log::info;
use std::env;
use std::error::Error;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about = "Analyze what causes cargo rebuilds", long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to cargo project")]
    path: Option<PathBuf>,

    #[arg(short, long, help = "Verbose output")]
    verbose: bool,

    #[arg(long, help = "Cargo command to analyze", default_value = "check")]
    command: String,

    #[arg(help = "Additional arguments to pass to cargo", last = true)]
    cargo: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    if args.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::init();
    }

    let project_path = args
        .path
        .unwrap_or_else(|| env::current_dir().expect("Failed to get current directory"));

    info!("Analyzing cargo project at: {}", project_path.display());

    let full_command = if args.cargo.is_empty() {
        args.command
    } else {
        format!("{} {}", args.command, args.cargo.join(" "))
    };

    analyze_dirty_reasons(&project_path, &full_command)
}
