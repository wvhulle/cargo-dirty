use std::process::ExitCode;

use cargo_dirty::Config;

fn main() -> ExitCode {
    let cli = Config::parse_args();
    cli.init_logging();

    match cli.run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {e}");
            ExitCode::FAILURE
        }
    }
}
