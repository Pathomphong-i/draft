use super::graph::{MultiverseTimelineGraph, TimelineNodeType};

pub fn render_svg(graph: &MultiverseTimelineGraph) -> String {
    let num_lanes = graph.dimensions.len().max(1);
    let num_nodes = graph.nodes.len().max(1);

    let width = (num_lanes * 180 + 160).max(800);
    let height = ((num_nodes + 2) * 70).max(400);

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" width=\"{}\" height=\"{}\" style=\"background:#0f172a;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;\">\n",
        width, height, width, height
    ));

    out.push_str("  <defs>\n");
    out.push_str(
        "    <linearGradient id=\"laneGrad\" x1=\"0%\" y1=\"0%\" x2=\"0%\" y2=\"100%\">\n",
    );
    out.push_str("      <stop offset=\"0%\" stop-color=\"#1e293b\" stop-opacity=\"0.6\"/>\n");
    out.push_str("      <stop offset=\"100%\" stop-color=\"#0f172a\" stop-opacity=\"0.8\"/>\n");
    out.push_str("    </linearGradient>\n");
    out.push_str("  </defs>\n\n");

    // Render lane columns
    for (i, dim) in graph.dimensions.iter().enumerate() {
        let x = 60 + i * 180;
        out.push_str(&format!(
            "  <rect x=\"{}\" y=\"30\" width=\"160\" height=\"{}\" rx=\"8\" fill=\"url(#laneGrad)\" stroke=\"#334155\" stroke-width=\"1\"/>\n",
            x, height - 60
        ));
        let active_mark = if dim.is_active { " *" } else { "" };
        out.push_str(&format!(
            "  <text x=\"{}\" y=\"54\" fill=\"#38bdf8\" font-size=\"14\" font-weight=\"bold\" text-anchor=\"middle\">{}{}</text>\n",
            x + 80, escape_xml(&dim.name), active_mark
        ));
    }

    // Render nodes
    let mut node_positions = std::collections::HashMap::new();

    for (idx, node) in graph.nodes.iter().enumerate() {
        let x = 60 + node.lane * 180 + 80;
        let y = 90 + idx * 60;
        node_positions.insert(node.id.clone(), (x, y));

        let (fill, stroke, shape) = match node.node_type {
            TimelineNodeType::Root => ("#eab308", "#ca8a04", "circle"),
            TimelineNodeType::ForkPoint => ("#f59e0b", "#d97706", "diamond"),
            TimelineNodeType::Merge => ("#10b981", "#059669", "hexagon"),
            TimelineNodeType::Convergence => ("#a855f7", "#9333ea", "hexagon"),
            TimelineNodeType::DirtyWorkspace => ("#f43f5e", "#e11d48", "rect"),
            TimelineNodeType::Commit => ("#38bdf8", "#0284c7", "circle"),
        };

        out.push_str(&format!("  <g id=\"node_{}\">\n", node.short_id));
        out.push_str(&format!(
            "    <title>Commit: {}&#10;Author: {}&#10;Summary: {}</title>\n",
            node.id,
            escape_xml(&node.author),
            escape_xml(&node.summary)
        ));

        match shape {
            "diamond" => {
                out.push_str(&format!(
                    "    <polygon points=\"{},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                    x, y - 10, x + 10, y, x, y + 10, x - 10, y, fill, stroke
                ));
            }
            "hexagon" => {
                out.push_str(&format!(
                    "    <polygon points=\"{},{} {},{} {},{} {},{} {},{} {},{}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                    x - 10, y - 6, x, y - 12, x + 10, y - 6, x + 10, y + 6, x, y + 12, x - 10, y + 6, fill, stroke
                ));
            }
            "rect" => {
                out.push_str(&format!(
                    "    <rect x=\"{}\" y=\"{}\" width=\"18\" height=\"18\" rx=\"3\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                    x - 9, y - 9, fill, stroke
                ));
            }
            _ => {
                out.push_str(&format!(
                    "    <circle cx=\"{}\" cy=\"{}\" r=\"9\" fill=\"{}\" stroke=\"{}\" stroke-width=\"2\"/>\n",
                    x, y, fill, stroke
                ));
            }
        }

        out.push_str(&format!(
            "    <text x=\"{}\" y=\"{}\" fill=\"#94a3b8\" font-size=\"11\" font-family=\"monospace\">{}</text>\n",
            x + 16, y + 4, node.short_id
        ));
        out.push_str("  </g>\n");
    }

    // Render connecting splines
    for edge in &graph.edges {
        if let (Some(&(x1, y1)), Some(&(x2, y2))) = (
            node_positions.get(&edge.source),
            node_positions.get(&edge.target),
        ) {
            let cy = (y1 + y2) / 2;
            out.push_str(&format!(
                "  <path d=\"M {} {} C {} {}, {} {}, {} {}\" fill=\"none\" stroke=\"#64748b\" stroke-width=\"2\"/>\n",
                x1, y1, x1, cy, x2, cy, x2, y2
            ));
        }
    }

    out.push_str("</svg>\n");
    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
