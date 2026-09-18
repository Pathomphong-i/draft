use crate::cli::BisectArgs;
use crate::error::CliError;
use daft_core::history::{bisect_bad, bisect_good, bisect_reset, bisect_start, BisectStep};
use daft_core::Repository;
use std::env;

pub fn execute(args: BisectArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;

    let action = args.args.first().map(|s| s.as_str()).unwrap_or("start");
    match action {
        "start" => {
            bisect_start(&repo)?;
            println!("Bisecting started.");
        }
        "bad" => {
            let rev = args.args.get(1).map(|s| s.as_str());
            let step = bisect_bad(&repo, rev)?;
            match step {
                BisectStep::Step {
                    midpoint,
                    revisions_left,
                    estimated_steps,
                } => {
                    println!(
                        "Bisecting: {} revisions left to test after this (roughly {} steps)",
                        revisions_left, estimated_steps
                    );
                    println!("[{}]", midpoint.to_hex());
                }
                BisectStep::FoundFirstBad(oid) => {
                    println!("{} is the first bad commit", oid.to_hex());
                }
                BisectStep::WaitingForInput(msg) => {
                    println!("{}", msg);
                }
            }
        }
        "good" => {
            let rev = args.args.get(1).map(|s| s.as_str());
            let step = bisect_good(&repo, rev)?;
            match step {
                BisectStep::Step {
                    midpoint,
                    revisions_left,
                    estimated_steps,
                } => {
                    println!(
                        "Bisecting: {} revisions left to test after this (roughly {} steps)",
                        revisions_left, estimated_steps
                    );
                    println!("[{}]", midpoint.to_hex());
                }
                BisectStep::FoundFirstBad(oid) => {
                    println!("{} is the first bad commit", oid.to_hex());
                }
                BisectStep::WaitingForInput(msg) => {
                    println!("{}", msg);
                }
            }
        }
        "reset" => {
            bisect_reset(&repo)?;
            println!("Bisecting reset.");
        }
        other => {
            return Err(CliError::General(format!(
                "Unknown bisect subcommand: {}",
                other
            )));
        }
    }

    Ok(())
}
