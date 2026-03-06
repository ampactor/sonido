//! Graph builder: constructs a `ProcessingGraph` from a parsed [`GraphSpec`].
//!
//! Returns a manifest of `(NodeId, effect_id)` pairs alongside the graph,
//! enabling `GraphEngine::new_dag()` to populate its slot-indexed access tables.

use crate::DslError;
use crate::effects::create_effect_with_params;
use crate::parser::{GraphNode, GraphSpec};

use sonido_core::graph::{NodeId, ProcessingGraph};

/// An effect manifest entry: node ID + registry effect ID.
pub type ManifestEntry = (NodeId, &'static str);

/// Build a `ProcessingGraph` and effect manifest from a parsed [`GraphSpec`].
///
/// Creates effects via the sonido-effects registry, connects them according
/// to the topology, and compiles the graph for audio processing.
///
/// The manifest contains one `(NodeId, &'static str)` per effect node in
/// topological order. Pass it to `GraphEngine::new_dag()` for slot-indexed
/// parameter access.
///
/// # Errors
///
/// Returns [`DslError`] if an effect name is unknown, a parameter is invalid,
/// or the resulting graph is malformed.
pub fn build_graph(
    spec: &GraphSpec,
    sample_rate: f32,
    block_size: usize,
) -> Result<(ProcessingGraph, Vec<ManifestEntry>), DslError> {
    tracing::debug!("dsl_build: constructing graph at {sample_rate}Hz, block_size={block_size}");
    let mut graph = ProcessingGraph::new(sample_rate, block_size);
    let mut manifest = Vec::new();

    let input = graph.add_input();
    let output = graph.add_output();

    let (entry, exit) = build_path(&mut graph, &mut manifest, spec, sample_rate)?;
    graph.connect(input, entry)?;
    graph.connect(exit, output)?;

    graph.compile()?;
    tracing::info!(
        "dsl_complete: {} nodes, {} edges, {} effects",
        graph.node_count(),
        graph.edge_count(),
        manifest.len()
    );
    Ok((graph, manifest))
}

/// Build a `ProcessingGraph` from a parsed [`GraphSpec`] (without manifest).
///
/// Convenience wrapper that discards the manifest. Use [`build_graph`] when
/// you need slot-indexed access via `GraphEngine::new_dag()`.
pub fn build_graph_only(
    spec: &GraphSpec,
    sample_rate: f32,
    block_size: usize,
) -> Result<ProcessingGraph, DslError> {
    build_graph(spec, sample_rate, block_size).map(|(graph, _)| graph)
}

/// Build a serial path, returning `(entry, exit)` node IDs.
///
/// Dry nodes in mixed paths (e.g., `- | reverb`) are skipped — the dry is
/// a no-op when other effects are present. Fully-dry paths are handled at
/// the split level via direct `connect(split, merge)`.
fn build_path(
    graph: &mut ProcessingGraph,
    manifest: &mut Vec<ManifestEntry>,
    nodes: &[GraphNode],
    sample_rate: f32,
) -> Result<(NodeId, NodeId), DslError> {
    let mut segments: Vec<(NodeId, NodeId)> = Vec::with_capacity(nodes.len());

    for node in nodes {
        if matches!(node, GraphNode::Dry) {
            continue; // skip dry in paths with other effects
        }
        let (entry, exit) = build_segment(graph, manifest, node, sample_rate)?;
        segments.push((entry, exit));
    }

    // If we get here with no segments, the path was all-dry, which should have
    // been handled by build_split. Panic indicates a logic error.
    assert!(
        !segments.is_empty(),
        "all-dry paths should be handled by build_split"
    );

    // Wire segments in series
    for i in 1..segments.len() {
        graph.connect(segments[i - 1].1, segments[i].0)?;
    }

    Ok((segments[0].0, segments[segments.len() - 1].1))
}

/// Build a single segment, returning `(entry, exit)` node IDs.
fn build_segment(
    graph: &mut ProcessingGraph,
    manifest: &mut Vec<ManifestEntry>,
    node: &GraphNode,
    sample_rate: f32,
) -> Result<(NodeId, NodeId), DslError> {
    match node {
        GraphNode::Effect { name, params } => {
            let (effect, resolved_id) = create_effect_with_params(name, sample_rate, params)?;
            let id = graph.add_effect(effect);
            manifest.push((id, resolved_id));
            Ok((id, id))
        }
        GraphNode::Dry => {
            // Dry nodes are filtered by build_path or handled by build_split.
            unreachable!("Dry nodes are handled by build_path / build_split")
        }
        GraphNode::Split { paths } => build_split(graph, manifest, paths, sample_rate),
    }
}

/// Build a split/merge topology, returning `(split_node, merge_node)`.
///
/// Dry-only paths get a direct `connect(split, merge)` — no intermediate
/// node. Other paths are built as serial chains wired between the split
/// and merge nodes.
fn build_split(
    graph: &mut ProcessingGraph,
    manifest: &mut Vec<ManifestEntry>,
    paths: &[Vec<GraphNode>],
    sample_rate: f32,
) -> Result<(NodeId, NodeId), DslError> {
    tracing::debug!("dsl_split: {} paths", paths.len());
    let split = graph.add_split();
    let merge = graph.add_merge();

    for path in paths {
        let all_dry = path.iter().all(|n| matches!(n, GraphNode::Dry));
        if all_dry {
            graph.connect(split, merge)?;
        } else {
            let (entry, exit) = build_path(graph, manifest, path, sample_rate)?;
            graph.connect(split, entry)?;
            graph.connect(exit, merge)?;
        }
    }

    Ok((split, merge))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_graph_dsl, validate_spec};

    #[test]
    fn build_linear_chain_with_manifest() {
        let spec = parse_graph_dsl("distortion:drive=15 | reverb:mix=0.3").unwrap();
        validate_spec(&spec).unwrap();
        let (graph, manifest) = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 4); // Input + 2 effects + Output
        assert_eq!(manifest.len(), 2);
        assert_eq!(manifest[0].1, "distortion");
        assert_eq!(manifest[1].1, "reverb");
    }

    #[test]
    fn build_single_effect_manifest() {
        let spec = parse_graph_dsl("reverb:decay=0.8").unwrap();
        validate_spec(&spec).unwrap();
        let (graph, manifest) = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 3); // Input + Effect + Output
        assert_eq!(manifest.len(), 1);
        assert_eq!(manifest[0].1, "reverb");
    }

    #[test]
    fn build_split_with_dry_manifest() {
        let spec = parse_graph_dsl("split(distortion:drive=20; -)").unwrap();
        validate_spec(&spec).unwrap();
        let (graph, manifest) = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 5); // Input + Split + Distortion + Merge + Output
        assert_eq!(manifest.len(), 1); // only distortion, not dry
        assert_eq!(manifest[0].1, "distortion");
    }

    #[test]
    fn build_split_with_chains_manifest() {
        let spec =
            parse_graph_dsl("split(distortion:drive=20 | chorus; phaser | flanger) | reverb")
                .unwrap();
        validate_spec(&spec).unwrap();
        let (graph, manifest) = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 9);
        assert_eq!(manifest.len(), 5);
        // Effects appear in topological (build) order within their paths
        let ids: Vec<&str> = manifest.iter().map(|m| m.1).collect();
        assert!(ids.contains(&"distortion"));
        assert!(ids.contains(&"chorus"));
        assert!(ids.contains(&"phaser"));
        assert!(ids.contains(&"flanger"));
        assert!(ids.contains(&"reverb"));
    }

    #[test]
    fn build_nested_split_manifest() {
        let spec =
            parse_graph_dsl("split(split(distortion; chorus); reverb:mix=1.0) | limiter").unwrap();
        validate_spec(&spec).unwrap();
        let (graph, manifest) = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 10);
        assert_eq!(manifest.len(), 4); // distortion, chorus, reverb, limiter
    }

    #[test]
    fn build_graph_processes_audio() {
        let spec = parse_graph_dsl("split(distortion:drive=15; -) | limiter").unwrap();
        validate_spec(&spec).unwrap();
        let (mut graph, _) = build_graph(&spec, 48000.0, 256).unwrap();

        let left_in = vec![0.1_f32; 256];
        let right_in = vec![0.1_f32; 256];
        let mut left_out = vec![0.0_f32; 256];
        let mut right_out = vec![0.0_f32; 256];

        // Should not panic
        graph.process_block(&left_in, &right_in, &mut left_out, &mut right_out);

        // Output should be non-zero
        let energy: f32 = left_out.iter().map(|s| s * s).sum();
        assert!(energy > 0.0, "output should contain signal");
    }

    #[test]
    fn build_graph_only_compat() {
        let spec = parse_graph_dsl("reverb").unwrap();
        validate_spec(&spec).unwrap();
        let graph = build_graph_only(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 3);
    }
}
