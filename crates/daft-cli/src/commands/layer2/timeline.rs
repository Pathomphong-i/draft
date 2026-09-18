pub mod ancestry;
pub mod ascii;
pub mod dot;
pub mod graph;
pub mod json;
pub mod svg;

pub use ancestry::render_ancestry_tree;
pub use ascii::{render_single_dimension, render_terminal_graph};
pub use dot::render_dot;
pub use graph::TimelineGraphBuilder;
pub use json::render_json;
pub use svg::render_svg;

use crate::cli::TimelineArgs;
use crate::error::CliError;
use daft_core::Repository;
use std::env;
use std::sync::Arc;

pub fn execute(args: TimelineArgs) -> Result<(), CliError> {
    let cwd = env::current_dir()?;
    let repo = Arc::new(Repository::discover(&cwd)?);

    let is_export = args.dimension.as_deref() == Some("export")
        || args.args.first().map(|s| s.as_str()) == Some("export");

    if is_export {
        let mut fmt = args.format;
        if fmt.is_none() {
            let mut i = 0;
            while i < args.args.len() {
                if (args.args[i] == "--format" || args.args[i] == "-f") && i + 1 < args.args.len() {
                    fmt = Some(args.args[i + 1].clone());
                    break;
                } else if args.args[i].starts_with("--format=") {
                    fmt = Some(args.args[i]["--format=".len()..].to_string());
                    break;
                }
                i += 1;
            }
        }

        let format_str = fmt.as_deref().unwrap_or("json");
        let graph = TimelineGraphBuilder::build(&repo, None)?;

        match format_str {
            "dot" => {
                let dot = render_dot(&graph);
                print!("{}", dot);
            }
            "svg" => {
                let svg = render_svg(&graph);
                print!("{}", svg);
            }
            _ => {
                let json = render_json(&graph).map_err(|e| CliError::General(e.to_string()))?;
                println!("{}", json);
            }
        }
        return Ok(());
    }

    let is_ancestry = args.ancestry || args.args.iter().any(|a| a == "--ancestry");

    if is_ancestry {
        let graph = TimelineGraphBuilder::build(&repo, None)?;
        let tree = render_ancestry_tree(&graph);
        print!("{}", tree);
        return Ok(());
    }

    let single_target = args
        .dimension
        .as_deref()
        .filter(|d| *d != "export" && !d.starts_with('-'))
        .or_else(|| {
            args.args
                .first()
                .map(|s| s.as_str())
                .filter(|d| *d != "export" && !d.starts_with('-'))
        });

    if let Some(target) = single_target {
        let graph = TimelineGraphBuilder::build(&repo, Some(target))?;
        let single_out = render_single_dimension(&graph, target);
        print!("{}", single_out);
        return Ok(());
    }

    // Default: multiverse timeline graph
    let graph = TimelineGraphBuilder::build(&repo, None)?;
    let graph_out = render_terminal_graph(&graph);
    print!("{}", graph_out);

    Ok(())
}
