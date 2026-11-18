use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    hideo::app::run(hideo::cli::Cli::parse())
}
