use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use hideo::{cli, config, discover, frontmatter, page, linkmap, markdown, images, diagrams, template, world, compile, report, error};

fn main() -> ExitCode {
    let cli = cli::Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("Error: {e:#}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}

fn run(cli: cli::Cli) -> anyhow::Result<()> {
    match cli.command {
        cli::Command::Build(args) => {
            todo!("Build command: {:?}", args)
        }
        cli::Command::Check(args) => {
            todo!("Check command: {:?}", args)
        }
        cli::Command::List(args) => {
            todo!("List command: {:?}", args)
        }
    }
}