use super::graph::{MultiverseTimelineGraph, TimelineNodeType};

pub fn render_dot(graph: &MultiverseTimelineGraph) -> String {
    let mut out = String::new();
    out.push_str("digraph DaftMultiverse {\n");
    out.push_str("  rankdir=BT;\n");
    out.push_str("  compound=true;\n");
    out.push_str("  fontname=\"Helvetica,Arial,sans-serif\";\n");
    out.push_str(
        "  node [fontname=\"Helvetica,Arial,sans-serif\", fontsize=10, margin=\"0.12,0.06\"];\n",
    );
    out.push_str("  edge [fontname=\"Helvetica,Arial,sans-serif\", fontsize=9];\n\n");

    // Group nodes by dimension cluster
    for dim in &graph.dimensions {
        let safe_dim_id = dim.name.replace('-', "_");
        let active_str = if dim.is_active { " (active)" } else { "" };
        out.push_str(&format!("  subgraph cluster_{} {{\n", safe_dim_id));
        out.push_str(&format!(
            "    label=\"{}{}\";\n",
            escape_str(&dim.name),
            active_str
        ));
        out.push_str("    style=\"rounded,filled\";\n");
        out.push_str("    color=\"#CBD5E1\";\n");
        out.push_str("    fillcolor=\"#F8FAFC\";\n\n");

        // Dimension virtual node if no commits
        if dim.head_commit.is_none() {
            out.push_str(&format!(
                "    \"dim_{}\" [label=\"{} [unborn]\", shape=box, style=\"filled,rounded\", fillcolor=\"#E2E8F0\"];\n",
                safe_dim_id,
                escape_str(&dim.name)
            ));
        }

        // Commit nodes assigned to this dimension
        for node in &graph.nodes {
            if node.head_of_dimensions.iter().any(|d| d == &dim.name) {
                let shape = match node.node_type {
                    TimelineNodeType::Commit => {
                        "shape=ellipse, style=filled, fillcolor=\"#E2E8F0\""
                    }
                    TimelineNodeType::Root => "shape=ellipse, style=filled, fillcolor=\"#FEF08A\"",
                    TimelineNodeType::ForkPoint => {
                        "shape=diamond, style=filled, fillcolor=\"#FEF08A\""
                    }
                    TimelineNodeType::Merge => "shape=hexagon, style=filled, fillcolor=\"#BBF7D0\"",
                    TimelineNodeType::Convergence => {
                        "shape=hexagon, style=filled, fillcolor=\"#C084FC\""
                    }
                    TimelineNodeType::DirtyWorkspace => {
                        "shape=note, style=\"dashed,filled\", fillcolor=\"#FECDD3\""
                    }
                };
                let label = format!("{}\n{}", node.short_id, escape_str(&node.summary));
                out.push_str(&format!(
                    "    \"node_{}\" [label=\"{}\", {}];\n",
                    node.short_id, label, shape
                ));
            }
        }

        out.push_str("  }\n\n");
    }

    // Nodes not inside any dimension cluster
    for node in &graph.nodes {
        if node.head_of_dimensions.is_empty() {
            let shape = "shape=ellipse, style=filled, fillcolor=\"#E2E8F0\"";
            let label = format!("{}\n{}", node.short_id, escape_str(&node.summary));
            out.push_str(&format!(
                "  \"node_{}\" [label=\"{}\", {}];\n",
                node.short_id, label, shape
            ));
        }
    }

    // Edges
    for edge in &graph.edges {
        let src_short = if edge.source.len() >= 7 {
            &edge.source[..7]
        } else {
            &edge.source
        };
        let tgt_short = if edge.target.len() >= 7 {
            &edge.target[..7]
        } else {
            &edge.target
        };

        let style = match edge.edge_type {
            super::graph::TimelineEdgeType::CommitParent => "color=\"#64748B\"",
            super::graph::TimelineEdgeType::DimensionFork => {
                "color=\"#A855F7\", style=dashed, label=\"fork\""
            }
            super::graph::TimelineEdgeType::MergeInput => "color=\"#10B981\"",
            super::graph::TimelineEdgeType::LiveWorkspace => "color=\"#E11D48\", style=dashed",
            super::graph::TimelineEdgeType::EntangleLink => {
                "color=\"#06B6D4\", style=dotted, dir=both, label=\"entangled\""
            }
        };

        out.push_str(&format!(
            "  \"node_{}\" -> \"node_{}\" [{}];\n",
            src_short, tgt_short, style
        ));
    }

    // Dimension parent links if nodes are absent
    if graph.nodes.is_empty() {
        for dim in &graph.dimensions {
            if let Some(ref p) = dim.parent {
                out.push_str(&format!(
                    "  \"dim_{}\" -> \"dim_{}\" [style=dashed, label=\"forked\"];\n",
                    dim.name.replace('-', "_"),
                    p.replace('-', "_")
                ));
            }
        }
    }

    out.push_str("}\n");
    out
}

fn escape_str(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}
