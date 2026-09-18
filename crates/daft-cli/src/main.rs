//! Draft VCS Unified CLI entry point ('draft' / 'dft').

use clap::Parser;
use std::process::ExitCode;

mod cli;
mod commands;
mod error;
mod output;

fn main() -> ExitCode {
    let args = cli::Cli::parse();

    match commands::dispatch(args) {
        Ok(_) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{}", err);
            ExitCode::from(err.exit_code())
        }
    }
}
