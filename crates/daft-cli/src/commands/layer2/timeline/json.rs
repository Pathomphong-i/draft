use super::graph::MultiverseTimelineGraph;

pub fn render_json(graph: &MultiverseTimelineGraph) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(graph)
}
