use super::graph::{MultiverseTimelineGraph, TimelineNodeType};
use std::collections::HashSet;

pub fn render_terminal_graph(graph: &MultiverseTimelineGraph) -> String {
    let mut out = String::new();
    out.push_str("Multiverse Timeline Graph:\n");

    if graph.nodes.is_empty() {
        for dim in &graph.dimensions {
            out.push_str(&format!("* ({}) [unborn]\n", dim.name));
        }
        return out;
    }

    let mut rendered_dims: HashSet<String> = HashSet::new();

    for node in &graph.nodes {
        let glyph = match node.node_type {
            TimelineNodeType::Root => "*",
            TimelineNodeType::ForkPoint => "◆",
            TimelineNodeType::Merge => "◈",
            TimelineNodeType::Convergence => "◉",
            TimelineNodeType::DirtyWorkspace => "○",
            TimelineNodeType::Commit => "*",
        };

        let mut lane_str = String::new();
        for i in 0..=node.lane {
            if i == node.lane {
                lane_str.push_str(glyph);
                lane_str.push(' ');
            } else {
                lane_str.push_str("| ");
            }
        }

        let heads_info = if !node.head_of_dimensions.is_empty() {
            for d in &node.head_of_dimensions {
                rendered_dims.insert(d.clone());
            }
            format!("({}) ", node.head_of_dimensions.join(", "))
        } else {
            String::new()
        };

        out.push_str(&format!(
            "{:<4} {}{:<7} {} - {}\n",
            lane_str.trim_end(),
            heads_info,
            node.short_id,
            node.summary,
            node.author
        ));
    }

    // Ensure any dimensions not explicitly tagged on commits are also displayed
    for dim in &graph.dimensions {
        if !rendered_dims.contains(&dim.name) {
            let head_str = dim
                .head_commit
                .as_ref()
                .map(|h| if h.len() >= 7 { &h[..7] } else { h.as_str() })
                .unwrap_or("unborn");
            out.push_str(&format!("* ({}) [head: {}]\n", dim.name, head_str));
        }
    }

    out
}

pub fn render_single_dimension(graph: &MultiverseTimelineGraph, dimension: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("Timeline Graph for {}:\n", dimension));

    let matching_nodes: Vec<_> = graph
        .nodes
        .iter()
        .filter(|n| {
            n.head_of_dimensions.iter().any(|d| d == dimension)
                || n.reachable_dimensions.iter().any(|d| d == dimension)
        })
        .collect();

    if matching_nodes.is_empty() {
        out.push_str(&format!("* ({})\n", dimension));
    } else {
        for n in matching_nodes {
            out.push_str(&format!("* ({}) {} {}\n", dimension, n.short_id, n.summary));
        }
    }

    out
}
