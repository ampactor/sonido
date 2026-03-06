//! Graph topology DSL — thin re-export from [`sonido_graph_dsl`].
//!
//! All parser, builder, and slug logic lives in the shared `sonido-graph-dsl`
//! crate so that both CLI and GUI can use the same DSL.

pub use sonido_graph_dsl::{
    DslError, GraphSpec, build_graph_only, build_graph_slug, parse_graph_dsl, validate_spec,
};

/// Build a [`ProcessingGraph`](sonido_core::graph::ProcessingGraph) from a DSL spec.
///
/// Thin wrapper that discards the manifest. CLI's `--graph` flag uses this
/// because it wraps the graph in `GraphEngine::new()` (legacy path).
/// Use [`sonido_graph_dsl::build_graph`] for the manifest-aware version.
pub fn build_graph(
    spec: &GraphSpec,
    sample_rate: f32,
    block_size: usize,
) -> Result<sonido_core::graph::ProcessingGraph, DslError> {
    build_graph_only(spec, sample_rate, block_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reexport_parse_works() {
        let spec = parse_graph_dsl("distortion | reverb").unwrap();
        assert_eq!(spec.len(), 2);
    }

    #[test]
    fn reexport_build_works() {
        let spec = parse_graph_dsl("reverb:mix=0.3").unwrap();
        validate_spec(&spec).unwrap();
        let graph = build_graph(&spec, 48000.0, 256).unwrap();
        assert_eq!(graph.node_count(), 3);
    }

    #[test]
    fn reexport_slug_works() {
        assert_eq!(build_graph_slug("reverb"), "reverb");
    }
}
