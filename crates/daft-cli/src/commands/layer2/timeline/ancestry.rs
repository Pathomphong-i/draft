use super::graph::MultiverseTimelineGraph;
use std::collections::{HashMap, HashSet};

pub fn render_ancestry_tree(graph: &MultiverseTimelineGraph) -> String {
    let mut out = String::new();
    out.push_str("Multiverse Ancestry Graph:\n");

    // Build children map: parent -> Vec<child_dimension>
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut root_dims = Vec::new();

    for d in &graph.dimensions {
        if let Some(ref p) = d.parent {
            if p != &d.name {
                children_map
                    .entry(p.clone())
                    .or_default()
                    .push(d.name.clone());
            } else {
                root_dims.push(d.name.clone());
            }
        } else {
            root_dims.push(d.name.clone());
        }
    }

    if !root_dims.contains(&"mainline".to_string()) {
        root_dims.insert(0, "mainline".to_string());
    }

    let mut visited: HashSet<String> = HashSet::new();

    for root in root_dims {
        if visited.insert(root.clone()) {
            let root_dim = graph.dimensions.iter().find(|d| d.name == root);
            let head_str = root_dim
                .and_then(|d| d.head_commit.as_ref())
                .map(|h| if h.len() >= 7 { &h[..7] } else { h.as_str() })
                .unwrap_or("unborn");

            out.push_str(&format!("* [{}] (origin) [head: {}]\n", root, head_str));
            render_children(&root, &children_map, graph, &mut visited, &mut out, "");
        }
    }

    out
}

fn render_children(
    parent: &str,
    children_map: &HashMap<String, Vec<String>>,
    graph: &MultiverseTimelineGraph,
    visited: &mut HashSet<String>,
    out: &mut String,
    prefix: &str,
) {
    if let Some(children) = children_map.get(parent) {
        let count = children.len();
        for (i, child) in children.iter().enumerate() {
            let is_last = i + 1 == count;
            let connector = if is_last { "└── " } else { "├── " };
            let next_prefix = format!("{}{}", prefix, if is_last { "    " } else { "│   " });

            if !visited.insert(child.clone()) {
                out.push_str(&format!(
                    "{}{} [{}] (cycle detected)\n",
                    prefix, connector, child
                ));
                continue;
            }

            let child_dim = graph.dimensions.iter().find(|d| &d.name == child);
            let head_str = child_dim
                .and_then(|d| d.head_commit.as_ref())
                .map(|h| if h.len() >= 7 { &h[..7] } else { h.as_str() })
                .unwrap_or("unborn");

            let status_str = child_dim.map(|d| d.status.as_str()).unwrap_or("clean");

            let fork_info = if let Some(fb) = child_dim.and_then(|d| d.fork_base_commit.as_ref()) {
                let short_fb = if fb.len() >= 7 { &fb[..7] } else { fb.as_str() };
                format!("forked from {} @ {}", parent, short_fb)
            } else {
                format!("forked from {}", parent)
            };

            out.push_str(&format!(
                "{}{} [{}] ({}) [head: {}, {}]\n",
                prefix, connector, child, fork_info, head_str, status_str
            ));

            render_children(child, children_map, graph, visited, out, &next_prefix);
        }
    }
}
