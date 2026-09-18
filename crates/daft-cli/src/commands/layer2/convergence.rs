use crate::cli::{CascadeArgs, CollapseArgs, ConvergeArgs, SpliceArgs, WeaveArgs};
use crate::error::CliError;
use daft_convergence::{
    CascadeEngine, CascadeOutcome, CollapseEngine, CollapseOptions, CollapseOutcome,
    ConvergeEngine, ConvergeOptions, ConvergeOutcome, MergeStrategy, SpliceEngine, WeaveEngine,
};
use daft_core::Repository;
use std::env;
use std::str::FromStr;
use std::sync::Arc;

pub fn execute_collapse(args: CollapseArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let collapse_engine = CollapseEngine::new(Arc::clone(&repo));

    let strategy = match args.strategy {
        Some(s) => Some(MergeStrategy::from_str(&s).map_err(CliError::General)?),
        None => None,
    };

    let outcome = collapse_engine
        .collapse(
            &[],
            CollapseOptions {
                into: args.into,
                strategy,
                message: None,
            },
        )
        .map_err(|e| match e {
            daft_convergence::ConvergenceError::DimensionNotFound(d) => {
                CliError::DimensionNotFound(d)
            }
            other => CliError::General(other.to_string()),
        })?;

    match outcome {
        CollapseOutcome::Clean {
            target_dimension, ..
        } => {
            println!("Collapsed all dimensions into '{}'", target_dimension);
        }
        CollapseOutcome::NoOp {
            target_dimension, ..
        } => {
            println!("Collapsed all dimensions into '{}'", target_dimension);
        }
        CollapseOutcome::Conflict {
            conflicting_files, ..
        } => {
            println!(
                "Merge conflict encountered during collapse: {} files",
                conflicting_files.len()
            );
        }
    }

    Ok(())
}

pub fn execute_converge(args: ConvergeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let converge_engine = ConvergeEngine::new(Arc::clone(&repo));

    let outcome = converge_engine
        .converge(
            &args.dimensions,
            ConvergeOptions {
                into: args.into,
                strategy: MergeStrategy::ManualMarkers,
                message: None,
                no_commit: false,
            },
        )
        .map_err(|e| match e {
            daft_convergence::ConvergenceError::DimensionNotFound(d) => {
                CliError::DimensionNotFound(d)
            }
            other => CliError::General(other.to_string()),
        })?;

    match outcome {
        ConvergeOutcome::Clean {
            target_dimension,
            merged_dimensions,
            ..
        } => {
            println!(
                "Converged dimensions {:?} into '{}'",
                merged_dimensions, target_dimension
            );
        }
        ConvergeOutcome::Conflict {
            conflicting_files, ..
        } => {
            println!(
                "Merge conflict encountered during converge: {} files",
                conflicting_files.len()
            );
        }
    }

    Ok(())
}

pub fn execute_cascade(args: CascadeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let cascade_engine = CascadeEngine::new(Arc::clone(&repo));

    let outcome = cascade_engine
        .cascade(&args.dimension, args.to.as_deref(), true)
        .map_err(|e| match e {
            daft_convergence::ConvergenceError::DimensionNotFound(d) => {
                CliError::DimensionNotFound(d)
            }
            other => CliError::General(other.to_string()),
        })?;

    match outcome {
        CascadeOutcome::Success {
            propagated_chain, ..
        } => {
            println!(
                "Cascade propagation completed successfully along chain: {:?}",
                propagated_chain
            );
        }
        CascadeOutcome::AbortedOnConflict {
            at_dimension,
            conflicting_files,
            ..
        } => {
            println!(
                "Cascade aborted due to conflict at dimension '{}' in files: {:?}",
                at_dimension, conflicting_files
            );
        }
    }

    Ok(())
}

pub fn execute_weave(args: WeaveArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let weave_engine = WeaveEngine::new(Arc::clone(&repo));

    let outcome = weave_engine
        .weave(&args.dim1, &args.dim2, None, None)
        .map_err(|e| match e {
            daft_convergence::ConvergenceError::DimensionNotFound(d) => {
                CliError::DimensionNotFound(d)
            }
            other => CliError::General(other.to_string()),
        })?;

    println!(
        "Weave completed into '{}' ({} commits woven)",
        outcome.target_dimension, outcome.commit_count
    );
    Ok(())
}

pub fn execute_splice(args: SpliceArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let splice_engine = SpliceEngine::new(Arc::clone(&repo));

    let outcome = splice_engine
        .splice(&args.dimension, &args.into, &args.range, None)
        .map_err(|e| match e {
            daft_convergence::ConvergenceError::DimensionNotFound(d) => {
                CliError::DimensionNotFound(d)
            }
            other => CliError::General(other.to_string()),
        })?;

    println!(
        "Spliced {} commits from '{}' into '{}'",
        outcome.spliced_commits.len(),
        outcome.donor_dim,
        outcome.target_dim
    );
    Ok(())
}
