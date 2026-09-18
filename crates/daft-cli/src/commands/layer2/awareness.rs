use crate::cli::{
    ClaimArgs, EntropyArgs, FenceArgs, ForeseeArgs, ObserveArgs, OverlapArgs, RadarArgs,
    TerritoryArgs, YieldArgs,
};
use crate::error::CliError;
use crate::output::print_output;
use daft_awareness::{
    EntropySubsystem, ForeseeSubsystem, ObserveSubsystem, RadarSubsystem, TerritoryManager,
};
use daft_core::diff::format_unified_diff;
use daft_core::Repository;
use daft_dimension::DimensionManager;
use serde::Serialize;
use std::env;
use std::path::Path;
use std::sync::Arc;

// ----------------------------------------------------------------------------
// Observe
// ----------------------------------------------------------------------------

pub fn execute_observe(args: ObserveArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let observe = ObserveSubsystem::new(Arc::clone(&repo));

    if args.args.is_empty() {
        return Err(CliError::General(
            "Usage: dft observe <dim> <path> | diff <d1> <d2> | log <d> | status <d>".into(),
        ));
    }

    match args.args[0].as_str() {
        "diff" => {
            if args.args.len() < 3 {
                return Err(CliError::General(
                    "Usage: dft observe diff <dim1> <dim2> [path]".into(),
                ));
            }
            let dim1 = &args.args[1];
            let dim2 = &args.args[2];
            let filter = args.args.get(3).map(Path::new);
            let patches = observe
                .diff(dim1, dim2, filter)
                .map_err(|e| CliError::General(e.to_string()))?;
            print!("{}", format_unified_diff(&patches));
        }
        "log" => {
            if args.args.len() < 2 {
                return Err(CliError::General("Usage: dft observe log <dim>".into()));
            }
            let dim = &args.args[1];
            let entries = observe
                .log(dim, None)
                .map_err(|e| CliError::General(e.to_string()))?;
            if entries.is_empty() {
                println!("No commits in dimension '{}'", dim);
            } else {
                for entry in entries {
                    println!("commit {}", entry.oid.to_hex());
                    println!("Author:     {} <{}>", entry.author.name, entry.author.email);
                    println!();
                    println!("    {}", entry.message.trim());
                    println!();
                }
            }
        }
        "status" => {
            if args.args.len() < 2 {
                return Err(CliError::General("Usage: dft observe status <dim>".into()));
            }
            let dim = &args.args[1];
            let report = observe
                .status(dim)
                .map_err(|e| CliError::General(e.to_string()))?;

            if report.is_clean {
                println!(
                    "nothing to commit, working tree clean in dimension '{}'",
                    dim
                );
            } else {
                for p in &report.staged_added {
                    println!("\tnew file:   {}", p.display());
                }
                for p in &report.staged_modified {
                    println!("\tmodified:   {}", p.display());
                }
                for p in &report.staged_deleted {
                    println!("\tdeleted:    {}", p.display());
                }
                for p in &report.unstaged_modified {
                    println!("\tmodified:   {}", p.display());
                }
                for p in &report.unstaged_deleted {
                    println!("\tdeleted:    {}", p.display());
                }
                for p in &report.untracked {
                    println!("\tuntracked:  {}", p.display());
                }
            }
        }
        dim => {
            if args.args.len() < 2 {
                return Err(CliError::General("Usage: dft observe <dim> <file>".into()));
            }
            let rel_file = &args.args[1];
            let bytes = observe
                .cat_file(dim, Path::new(rel_file))
                .map_err(|e| match e {
                    daft_awareness::AwarenessError::DimensionNotFound(d) => {
                        CliError::DimensionNotFound(d)
                    }
                    daft_awareness::AwarenessError::FileNotFoundInDimension { path, dimension } => {
                        CliError::General(format!(
                            "File '{}' not found in dimension '{}'",
                            path.display(),
                            dimension
                        ))
                    }
                    other => CliError::General(other.to_string()),
                })?;
            print!("{}", String::from_utf8_lossy(&bytes));
        }
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// Radar
// ----------------------------------------------------------------------------

#[derive(Serialize, Debug)]
pub struct RadarJsonOutput {
    pub hot_zones: Vec<String>,
    pub active_dimensions: Vec<String>,
}

pub fn execute_radar(args: RadarArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let radar = RadarSubsystem::new(Arc::clone(&repo));

    let report = radar
        .scan(2)
        .map_err(|e| CliError::General(e.to_string()))?;

    let hot_zones: Vec<String> = report
        .hot_zones
        .iter()
        .map(|a| a.path.to_string_lossy().to_string())
        .collect();

    if json {
        let out = RadarJsonOutput {
            hot_zones,
            active_dimensions: report.scanned_dimensions,
        };
        print_output(true, &out, "");
        return Ok(());
    }

    if let Some(target_p) = &args.path {
        if let Some(act) = radar
            .inspect_path(Path::new(target_p))
            .map_err(|e| CliError::General(e.to_string()))?
        {
            println!(
                "{}\t[active in {} contexts: {}]",
                act.path.display(),
                act.touch_count,
                act.dimensions.join(", ")
            );
        } else {
            println!("Path '{}' is not active in any dimension", target_p);
        }
        return Ok(());
    }

    if args.hot {
        for p in &hot_zones {
            println!("HOT ZONE: {}", p);
        }
    } else {
        for act in &report.activities {
            println!(
                "{}\t[active in {} contexts]",
                act.path.display(),
                act.touch_count
            );
        }
    }

    Ok(())
}

// ----------------------------------------------------------------------------
// Foresee & Overlap & Entropy
// ----------------------------------------------------------------------------

pub fn execute_foresee(args: ForeseeArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let foresee = ForeseeSubsystem::new(Arc::clone(&repo));

    let dim_mgr = DimensionManager::new(Arc::clone(&repo));
    let current = dim_mgr.current_dimension_name();

    let dim1 = args.dim1.unwrap_or(current);
    let dim2 = args.dim2.unwrap_or_else(|| "mainline".to_string());

    let report = foresee
        .predict(&dim1, &dim2)
        .map_err(|e| CliError::General(e.to_string()))?;

    if report.has_conflicts {
        println!(
            "conflict: potential 3-way merge conflict detected between '{}' and '{}'",
            dim1, dim2
        );
        for file in &report.conflicting_files {
            println!("  conflict in {}", file.display());
        }
    } else {
        println!(
            "0 conflicts: clean prediction between '{}' and '{}'",
            dim1, dim2
        );
    }

    Ok(())
}

pub fn execute_overlap(_args: OverlapArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let radar = RadarSubsystem::new(Arc::clone(&repo));

    let report = radar
        .scan(2)
        .map_err(|e| CliError::General(e.to_string()))?;

    for act in &report.hot_zones {
        println!(
            "{}\t[concurrently modified in {} dimensions]",
            act.path.display(),
            act.touch_count
        );
    }

    Ok(())
}

pub fn execute_entropy(args: EntropyArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);
    let entropy = EntropySubsystem::new(Arc::clone(&repo));

    let metrics = entropy
        .calculate(&args.dim1, &args.dim2)
        .map_err(|e| CliError::General(e.to_string()))?;

    println!("entropy: {:.2}", metrics.total_entropy);
    Ok(())
}

// ----------------------------------------------------------------------------
// Territory: Claim, Yield, Fence, Territory
// ----------------------------------------------------------------------------

#[derive(Serialize, Debug, Clone)]
pub struct ClaimRecord {
    pub path: String,
    pub dimension: String,
}

#[derive(Serialize, Debug)]
pub struct TerritoryJsonOutput {
    pub claims: Vec<ClaimRecord>,
    pub fences: Vec<String>,
}

pub fn execute_claim(args: ClaimArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let dim_mgr = DimensionManager::new(Arc::new(Repository::discover(&cwd)?));
    let claiming_dim = args
        .dimension
        .unwrap_or_else(|| dim_mgr.current_dimension_name());

    let territory_mgr = TerritoryManager::new(repo.dft_dir());
    territory_mgr
        .claim(&args.path, &claiming_dim, None, None, false)
        .map_err(|e| CliError::General(e.to_string()))?;

    println!("Claimed path: {} for dimension {}", args.path, claiming_dim);
    Ok(())
}

pub fn execute_yield(args: YieldArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let territory_mgr = TerritoryManager::new(repo.dft_dir());

    territory_mgr
        .yield_path(&args.path, None, None)
        .map_err(|e| CliError::General(e.to_string()))?;

    println!("Yielded claim on {}", args.path);
    Ok(())
}

pub fn execute_fence(args: FenceArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let territory_mgr = TerritoryManager::new(repo.dft_dir());
    let current_dim = crate::commands::layer2::dimension::get_current_dimension(repo.dft_dir());

    territory_mgr
        .fence(&args.path, Some(&current_dim), None, args.hard, None, None)
        .map_err(|e| CliError::General(e.to_string()))?;

    println!("Fenced path: {}", args.path);
    Ok(())
}

pub fn execute_territory(args: TerritoryArgs, json: bool) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Repository::discover(&cwd)?;
    let territory_mgr = TerritoryManager::new(repo.dft_dir());

    let is_audit = args.args.first().map(|s| s.as_str()) == Some("audit");
    if is_audit {
        let fix = args.args.iter().any(|a| a == "--fix");
        let report = territory_mgr
            .audit(fix)
            .map_err(|e| CliError::General(e.to_string()))?;
        println!(
            "Territory audit complete: {} violations found.",
            report.violations.len()
        );
        for v in &report.violations {
            println!("  [VIOLATION] {}", v.message);
        }
        return Ok(());
    }

    let claims = territory_mgr
        .list_claims()
        .map_err(|e| CliError::General(e.to_string()))?;
    let fences = territory_mgr
        .list_fences()
        .map_err(|e| CliError::General(e.to_string()))?;

    let claim_records: Vec<ClaimRecord> = claims
        .iter()
        .map(|c| ClaimRecord {
            path: c.path_glob.clone(),
            dimension: c.dimension.clone(),
        })
        .collect();

    let fence_strings: Vec<String> = fences.iter().map(|f| f.path_glob.clone()).collect();

    if json {
        let out = TerritoryJsonOutput {
            claims: claim_records,
            fences: fence_strings,
        };
        print_output(true, &out, "");
        return Ok(());
    }

    println!("Active Territory Claims:");
    for c in &claim_records {
        println!("  {} (claimed by: {})", c.path, c.dimension);
    }
    println!("Active Territory Fences:");
    for f in &fence_strings {
        println!("  [FENCE] {}", f);
    }

    Ok(())
}
