use crate::chunk::{MycosChunk, Section};
use petgraph::graph::{DiGraph, NodeIndex};

pub fn build_internal_graph(chunk: &MycosChunk) -> DiGraph<(), ()> {
    let mut graph = DiGraph::<(), ()>::new();
    let nodes: Vec<NodeIndex> = (0..chunk.internal_count)
        .map(|_| graph.add_node(()))
        .collect();

    for conn in &chunk.connections {
        if matches!(conn.from_section, Section::Internal)
            && matches!(conn.to_section, Section::Internal)
        {
            let from = conn.from_index as usize;
            let to = conn.to_index as usize;
            graph.add_edge(nodes[from], nodes[to], ());
        }
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::{parse_chunk, validate_chunk};
    use std::fs;
    use std::path::PathBuf;

    fn fixtures() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("fixtures")
    }

    #[test]
    fn graph_node_and_edge_counts_match() {
        for entry in fs::read_dir(fixtures()).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().and_then(|s| s.to_str()) == Some("myc") {
                let data = fs::read(entry.path()).unwrap();
                let chunk = parse_chunk(&data).unwrap();
                validate_chunk(&chunk).unwrap();
                let graph = build_internal_graph(&chunk);

                assert_eq!(graph.node_count() as u32, chunk.internal_count);
                let expected_edges = chunk
                    .connections
                    .iter()
                    .filter(|c| {
                        matches!(c.from_section, Section::Internal)
                            && matches!(c.to_section, Section::Internal)
                    })
                    .count();
                assert_eq!(graph.edge_count(), expected_edges);
            }
        }
    }
}
