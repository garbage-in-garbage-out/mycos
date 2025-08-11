use crate::chunk::{MycosChunk, Section};
use petgraph::algo::kosaraju_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::{HashSet, VecDeque};

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

pub fn scc_ids_and_topo_levels(chunk: &MycosChunk) -> (Vec<usize>, Vec<usize>) {
    let graph = build_internal_graph(chunk);
    let sccs = kosaraju_scc(&graph);

    let mut scc_ids = vec![0usize; graph.node_count()];
    for (id, component) in sccs.iter().enumerate() {
        for node in component {
            scc_ids[node.index()] = id;
        }
    }

    let scc_count = sccs.len();
    let mut dag: Vec<HashSet<usize>> = vec![HashSet::new(); scc_count];
    for edge in graph.edge_references() {
        let u = scc_ids[edge.source().index()];
        let v = scc_ids[edge.target().index()];
        if u != v {
            dag[u].insert(v);
        }
    }

    let mut indegree = vec![0usize; scc_count];
    for edges in &dag {
        for &v in edges {
            indegree[v] += 1;
        }
    }

    let mut levels = vec![0usize; scc_count];
    let mut queue = VecDeque::new();
    for (i, &deg) in indegree.iter().enumerate() {
        if deg == 0 {
            queue.push_back(i);
        }
    }

    while let Some(u) = queue.pop_front() {
        for &v in dag[u].iter() {
            if levels[v] < levels[u] + 1 {
                levels[v] = levels[u] + 1;
            }
            indegree[v] -= 1;
            if indegree[v] == 0 {
                queue.push_back(v);
            }
        }
    }

    (scc_ids, levels)
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

    #[test]
    fn oscillator_two_cycle_scc() {
        let path = fixtures().join("oscillator_2cycle.myc");
        let data = fs::read(path).unwrap();
        let chunk = parse_chunk(&data).unwrap();
        validate_chunk(&chunk).unwrap();
        let (scc_ids, levels) = scc_ids_and_topo_levels(&chunk);
        assert_eq!(scc_ids, vec![0, 0]);
        assert_eq!(levels, vec![0]);
    }
}
