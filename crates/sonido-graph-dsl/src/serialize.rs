//! Graph-to-DSL serialization.
//!
//! Converts a [`GraphSpec`] back into the text-based DSL format, and provides
//! [`GraphSnapshot`]-level roundtrip via [`snapshot_to_dsl`] / [`snapshot_from_dsl`]:
//!
//! ```text
//! distortion:drive=20,mix=100 | reverb:decay=2.5,mix=30
//! ```
//!
//! Parameter values are formatted with up to 6 significant digits; stepped
//! (enum) params use their canonical step-label strings.

use crate::DslError;
use crate::effects::create_effect_with_params;
use crate::parser::{GraphNode, GraphSpec};
use sonido_core::graph::{GraphSnapshot, SnapshotEntry, SnapshotTopology, TopoNode};
use sonido_registry::EffectRegistry;

/// Serialize a parsed graph specification back to DSL text.
///
/// Linear sequences produce pipe syntax: `distortion:drive=20 | reverb:mix=0.3`
/// Parallel paths produce split syntax: `split(distortion; reverb) | limiter`
/// Dry passthrough renders as `-`.
///
/// Parameters are emitted in alphabetical key order for deterministic output.
/// Only non-empty parameter maps are included.
pub fn graph_to_dsl(spec: &GraphSpec) -> String {
    serialize_path(spec)
}

/// Serialize a path (serial chain of nodes) joined by ` | `.
fn serialize_path(nodes: &[GraphNode]) -> String {
    let parts: Vec<String> = nodes.iter().map(serialize_node).collect();
    parts.join(" | ")
}

/// Serialize a single graph node to DSL text.
fn serialize_node(node: &GraphNode) -> String {
    match node {
        GraphNode::Effect { name, params } => {
            if params.is_empty() {
                name.clone()
            } else {
                let mut sorted: Vec<_> = params.iter().collect();
                sorted.sort_by_key(|(k, _)| k.as_str());
                let param_str: Vec<String> = sorted
                    .into_iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect();
                format!("{name}:{}", param_str.join(","))
            }
        }
        GraphNode::Dry => "-".to_string(),
        GraphNode::Split { paths } => {
            let inner: Vec<String> = paths.iter().map(|p| serialize_path(p)).collect();
            format!("split({})", inner.join("; "))
        }
    }
}

// ---------------------------------------------------------------------------
// GraphSnapshot ↔ DSL
// ---------------------------------------------------------------------------

/// Convert a [`GraphSnapshot`] to a DSL string.
///
/// Encodes each effect slot as `effect_id:param=value,...` joined by ` | `.
/// Bypassed effects are prefixed with `!` (e.g., `!reverb:mix=30`).
///
/// Parameter values are formatted using the descriptor's step labels for
/// stepped (enum) params, and compact decimal for continuous params.
/// Only non-default parameter values are included; effects with all-default
/// params are emitted as bare names (e.g., `reverb` not `reverb:decay=2.5,...`).
///
/// # Example
///
/// ```rust,ignore
/// let dsl = snapshot_to_dsl(&snapshot);
/// // "distortion:drive=20 | reverb:decay=2.5,mix=30"
/// ```
pub fn snapshot_to_dsl(snapshot: &GraphSnapshot) -> String {
    let registry = EffectRegistry::new();

    match &snapshot.topology {
        None | Some(SnapshotTopology::Linear) => {
            // Linear: emit entries in order joined by ` | `.
            let parts: Vec<String> = snapshot
                .entries
                .iter()
                .map(|entry| snapshot_entry_to_dsl(entry, &registry))
                .collect();
            parts.join(" | ")
        }
        Some(SnapshotTopology::Tree(nodes)) => {
            serialize_topo_path(nodes, &snapshot.entries, &registry)
        }
    }
}

/// Serialize a topology path (serial chain of [`TopoNode`]s) joined by ` | `.
fn serialize_topo_path(
    nodes: &[TopoNode],
    entries: &[SnapshotEntry],
    registry: &EffectRegistry,
) -> String {
    let parts: Vec<String> = nodes
        .iter()
        .map(|n| serialize_topo_node(n, entries, registry))
        .collect();
    parts.join(" | ")
}

/// Serialize a single [`TopoNode`] to DSL text.
fn serialize_topo_node(
    node: &TopoNode,
    entries: &[SnapshotEntry],
    registry: &EffectRegistry,
) -> String {
    match node {
        TopoNode::Effect(idx) => {
            if let Some(entry) = entries.get(*idx) {
                snapshot_entry_to_dsl(entry, registry)
            } else {
                String::new()
            }
        }
        TopoNode::Dry => "-".to_string(),
        TopoNode::Split(paths) => {
            let inner: Vec<String> = paths
                .iter()
                .map(|path| serialize_topo_path(path, entries, registry))
                .collect();
            format!("split({})", inner.join("; "))
        }
    }
}

/// Build a [`SnapshotTopology`] from a parsed [`GraphSpec`] by pre-order walk.
///
/// The walk order matches [`build_graph`](crate::builder::build_graph)'s manifest
/// order, so `TopoNode::Effect(i)` indices correspond to snapshot entry slots.
///
/// Returns `SnapshotTopology::Linear` for a flat (no-split) spec, otherwise
/// `SnapshotTopology::Tree(nodes)`.
pub fn topology_from_spec(spec: &GraphSpec) -> SnapshotTopology {
    if spec.iter().all(|n| matches!(n, GraphNode::Effect { .. })) {
        return SnapshotTopology::Linear;
    }
    let mut counter = 0usize;
    let nodes = topo_nodes_from_path(spec, &mut counter);
    SnapshotTopology::Tree(nodes)
}

/// Recursively convert a path of [`GraphNode`]s to [`TopoNode`]s, incrementing
/// `counter` for each effect in pre-order.
fn topo_nodes_from_path(nodes: &[GraphNode], counter: &mut usize) -> Vec<TopoNode> {
    nodes
        .iter()
        .map(|node| topo_node_from_graph_node(node, counter))
        .collect()
}

fn topo_node_from_graph_node(node: &GraphNode, counter: &mut usize) -> TopoNode {
    match node {
        GraphNode::Effect { .. } => {
            let idx = *counter;
            *counter += 1;
            TopoNode::Effect(idx)
        }
        GraphNode::Dry => TopoNode::Dry,
        GraphNode::Split { paths } => {
            let topo_paths = paths
                .iter()
                .map(|path| topo_nodes_from_path(path, counter))
                .collect();
            TopoNode::Split(topo_paths)
        }
    }
}

/// Serialize a single snapshot entry to `[!]effect_id[:param=value,...]`.
fn snapshot_entry_to_dsl(entry: &SnapshotEntry, registry: &EffectRegistry) -> String {
    let prefix = if entry.bypassed { "!" } else { "" };

    // Build a temporary effect to obtain descriptors.
    let Some(effect) = registry.create(&entry.effect_id, 48000.0) else {
        // Unknown effect — emit as bare name (best-effort).
        return format!("{}{}", prefix, entry.effect_id);
    };

    let mut params: Vec<String> = Vec::new();

    for (i, &value) in entry.params.iter().enumerate() {
        let Some(desc) = effect.effect_param_info(i) else {
            continue;
        };

        // Skip default values to keep the DSL compact.
        if (value - desc.default).abs() < f32::EPSILON {
            continue;
        }

        // Use step label for stepped/enum params; decimal otherwise.
        let formatted = if let Some(labels) = desc.step_labels {
            let idx = value.round() as usize;
            labels
                .get(idx)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format_f32(value))
        } else {
            format_f32(value)
        };

        // Use the short_name as DSL key (lower-cased, spaces → underscores).
        let key = desc.short_name.to_lowercase().replace(' ', "_");
        params.push(format!("{key}={formatted}"));
    }

    if params.is_empty() {
        format!("{}{}", prefix, entry.effect_id)
    } else {
        format!("{}{}:{}", prefix, entry.effect_id, params.join(","))
    }
}

/// Format an f32 value compactly (no trailing zeros).
fn format_f32(v: f32) -> String {
    if v.fract() == 0.0 && v.abs() < 1_000_000.0 {
        format!("{}", v as i64)
    } else {
        // Up to 4 decimal places, strip trailing zeros.
        let s = format!("{:.4}", v);
        let s = s.trim_end_matches('0');
        s.trim_end_matches('.').to_string()
    }
}

/// Parse a DSL string into a [`GraphSnapshot`].
///
/// Supports linear chains only (no `split(...)` topology).
/// Bypassed effects may be prefixed with `!`.
///
/// # Errors
///
/// Returns [`DslError`] on syntax errors, unknown effect names, or invalid
/// parameter values.
///
/// # Example
///
/// ```rust,ignore
/// use sonido_graph_dsl::snapshot_from_dsl;
/// use sonido_registry::EffectRegistry;
///
/// let registry = EffectRegistry::new();
/// let snapshot = snapshot_from_dsl("distortion:drive=20 | reverb:mix=30", &registry)?;
/// assert_eq!(snapshot.entries.len(), 2);
/// ```
pub fn snapshot_from_dsl(s: &str, registry: &EffectRegistry) -> Result<GraphSnapshot, DslError> {
    let entries: Vec<SnapshotEntry> = s
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| dsl_part_to_snapshot_entry(part, registry))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(GraphSnapshot {
        entries,
        topology: None,
    })
}

/// Parse one pipe-separated segment into a [`SnapshotEntry`].
fn dsl_part_to_snapshot_entry(
    part: &str,
    registry: &EffectRegistry,
) -> Result<SnapshotEntry, DslError> {
    // Detect and strip bypass prefix `!`.
    let (bypassed, spec) = if let Some(rest) = part.strip_prefix('!') {
        (true, rest)
    } else {
        (false, part)
    };

    // Split into name and optional params string.
    let (name_part, params_str) = if let Some(colon) = spec.find(':') {
        (&spec[..colon], Some(&spec[colon + 1..]))
    } else {
        (spec, None)
    };

    let effect_id_raw = name_part.trim();
    let canonical_id = crate::effects::resolve_effect_name(effect_id_raw);

    // Create a temporary effect to get descriptors and defaults.
    let effect = registry.create(canonical_id, 48000.0).ok_or_else(|| {
        DslError::Effect(crate::effects::EffectError::UnknownEffect(
            effect_id_raw.to_string(),
        ))
    })?;

    let param_count = effect.effect_param_count();
    let mut params: Vec<f32> = (0..param_count)
        .map(|i| effect.effect_param_info(i).map_or(0.0, |d| d.default))
        .collect();

    // Apply overrides from the DSL string.
    if let Some(params_str) = params_str {
        use std::collections::HashMap;
        let raw_params: HashMap<String, String> = params_str
            .split(',')
            .filter_map(|kv| {
                let mut it = kv.splitn(2, '=');
                let k = it.next()?.trim().to_string();
                let v = it.next()?.trim().to_string();
                Some((k, v))
            })
            .collect();

        if !raw_params.is_empty() {
            let (temp_effect, _) = create_effect_with_params(canonical_id, 48000.0, &raw_params)?;

            // Overwrite all param slots with post-override values.
            // `create_effect_with_params` starts from defaults and applies overrides,
            // so unmentioned params stay at their defaults.
            for i in 0..params.len() {
                params[i] = temp_effect.effect_get_param(i);
            }
        }
    }

    Ok(SnapshotEntry {
        effect_id: canonical_id.to_string(),
        params,
        bypassed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_graph_dsl;

    /// Helper: parse then serialize, verify round-trip.
    fn roundtrip(input: &str) -> String {
        let spec = parse_graph_dsl(input).unwrap();
        graph_to_dsl(&spec)
    }

    #[test]
    fn single_effect() {
        assert_eq!(roundtrip("reverb"), "reverb");
    }

    #[test]
    fn linear_chain() {
        assert_eq!(roundtrip("distortion | reverb"), "distortion | reverb");
    }

    #[test]
    fn effect_with_params() {
        assert_eq!(
            roundtrip("distortion:drive=20,mix=0.8"),
            "distortion:drive=20,mix=0.8"
        );
    }

    #[test]
    fn parallel_split() {
        assert_eq!(
            roundtrip("split(distortion; reverb) | limiter"),
            "split(distortion; reverb) | limiter"
        );
    }

    #[test]
    fn dry_path() {
        assert_eq!(roundtrip("split(distortion; -)"), "split(distortion; -)");
    }

    #[test]
    fn nested_split() {
        let input = "split(split(chorus; flanger); reverb)";
        let result = roundtrip(input);
        // Re-parse to verify structural equivalence
        let spec1 = parse_graph_dsl(input).unwrap();
        let spec2 = parse_graph_dsl(&result).unwrap();
        assert_eq!(spec1, spec2);
    }

    #[test]
    fn whitespace_normalization() {
        assert_eq!(
            roundtrip("  distortion  |  reverb  "),
            "distortion | reverb"
        );
    }

    #[test]
    fn params_sorted_alphabetically() {
        // Parser may not preserve order, but serializer sorts
        let spec = parse_graph_dsl("distortion:mix=0.5,drive=20").unwrap();
        let result = graph_to_dsl(&spec);
        assert_eq!(result, "distortion:drive=20,mix=0.5");
    }

    #[test]
    fn complex_topology() {
        let input = "preamp | split(distortion:drive=15; chorus | flanger) | limiter";
        let result = roundtrip(input);
        let spec1 = parse_graph_dsl(input).unwrap();
        let spec2 = parse_graph_dsl(&result).unwrap();
        assert_eq!(spec1, spec2);
    }

    #[test]
    fn three_way_split() {
        assert_eq!(
            roundtrip("split(distortion; chorus; reverb)"),
            "split(distortion; chorus; reverb)"
        );
    }

    #[test]
    fn negative_param_values() {
        assert_eq!(
            roundtrip("limiter:ceiling=-0.5,threshold=-12"),
            "limiter:ceiling=-0.5,threshold=-12"
        );
    }

    #[test]
    fn split_with_chains_inside() {
        assert_eq!(
            roundtrip("split(distortion | chorus; reverb:mix=1.0)"),
            "split(distortion | chorus; reverb:mix=1.0)"
        );
    }

    // --- GraphSnapshot roundtrip tests ---

    #[test]
    fn snapshot_from_dsl_single_effect() {
        let registry = EffectRegistry::new();
        let snap = snapshot_from_dsl("distortion", &registry).unwrap();
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].effect_id, "distortion");
        assert!(!snap.entries[0].bypassed);
        // All params should be at defaults.
        let effect = registry.create("distortion", 48000.0).unwrap();
        for i in 0..snap.entries[0].params.len() {
            let default = effect.effect_param_info(i).unwrap().default;
            assert_eq!(
                snap.entries[0].params[i], default,
                "param {i} should be default"
            );
        }
    }

    #[test]
    fn snapshot_from_dsl_with_params() {
        let registry = EffectRegistry::new();
        let snap = snapshot_from_dsl("distortion:drive=20", &registry).unwrap();
        assert_eq!(snap.entries[0].effect_id, "distortion");
        // param 0 is drive
        assert_eq!(snap.entries[0].params[0], 20.0);
    }

    #[test]
    fn snapshot_from_dsl_bypassed() {
        let registry = EffectRegistry::new();
        let snap = snapshot_from_dsl("!reverb", &registry).unwrap();
        assert_eq!(snap.entries[0].effect_id, "reverb");
        assert!(snap.entries[0].bypassed);
    }

    #[test]
    fn snapshot_from_dsl_chain() {
        let registry = EffectRegistry::new();
        let snap = snapshot_from_dsl("distortion:drive=20 | !reverb", &registry).unwrap();
        assert_eq!(snap.entries.len(), 2);
        assert_eq!(snap.entries[0].effect_id, "distortion");
        assert!(!snap.entries[0].bypassed);
        assert_eq!(snap.entries[1].effect_id, "reverb");
        assert!(snap.entries[1].bypassed);
    }

    #[test]
    fn snapshot_to_dsl_default_params_omitted() {
        let registry = EffectRegistry::new();
        let effect = registry.create("distortion", 48000.0).unwrap();
        let param_count = effect.effect_param_count();
        let params: Vec<f32> = (0..param_count)
            .map(|i| effect.effect_param_info(i).unwrap().default)
            .collect();
        let snap = GraphSnapshot {
            entries: vec![SnapshotEntry {
                effect_id: "distortion".to_string(),
                params,
                bypassed: false,
            }],
            topology: None,
        };
        let dsl = snapshot_to_dsl(&snap);
        assert_eq!(dsl, "distortion");
    }

    #[test]
    fn snapshot_roundtrip() {
        let registry = EffectRegistry::new();
        // Parse DSL → snapshot → DSL → snapshot, check structural equality.
        let original_dsl = "distortion:drive=20 | !reverb";
        let snap1 = snapshot_from_dsl(original_dsl, &registry).unwrap();
        let round_dsl = snapshot_to_dsl(&snap1);
        let snap2 = snapshot_from_dsl(&round_dsl, &registry).unwrap();

        assert_eq!(snap1.entries.len(), snap2.entries.len());
        for (a, b) in snap1.entries.iter().zip(snap2.entries.iter()) {
            assert_eq!(a.effect_id, b.effect_id);
            assert_eq!(a.bypassed, b.bypassed);
            for (i, (va, vb)) in a.params.iter().zip(b.params.iter()).enumerate() {
                assert!((va - vb).abs() < 1e-4, "param {i} diverged: {va} vs {vb}");
            }
        }
    }

    // --- Topology round-trip tests ---

    /// Helper: build a snapshot with Tree topology from a DSL spec string.
    fn snap_with_topology(dsl: &str, registry: &EffectRegistry) -> GraphSnapshot {
        let spec = parse_graph_dsl(dsl).unwrap();
        let topo = topology_from_spec(&spec);
        // Collect entries in pre-order (matches topology_from_spec counter).
        let entries = collect_entries_preorder(&spec, registry);
        GraphSnapshot {
            entries,
            topology: Some(topo),
        }
    }

    /// Walk spec in pre-order and parse each effect into a SnapshotEntry.
    fn collect_entries_preorder(
        nodes: &[GraphNode],
        registry: &EffectRegistry,
    ) -> Vec<SnapshotEntry> {
        let mut entries = Vec::new();
        for node in nodes {
            match node {
                GraphNode::Effect { name, .. } => {
                    let canonical = crate::effects::resolve_effect_name(name);
                    if let Some(effect) = registry.create(canonical, 48000.0) {
                        let param_count = effect.effect_param_count();
                        let params: Vec<f32> = (0..param_count)
                            .map(|i| effect.effect_param_info(i).map_or(0.0, |d| d.default))
                            .collect();
                        entries.push(SnapshotEntry {
                            effect_id: canonical.to_string(),
                            params,
                            bypassed: false,
                        });
                    }
                }
                GraphNode::Dry => {}
                GraphNode::Split { paths } => {
                    for path in paths {
                        entries.extend(collect_entries_preorder(path, registry));
                    }
                }
            }
        }
        entries
    }

    // 1. Linear (no topology): topology: None → linear serialization path.
    #[test]
    fn topo_rt_linear_no_topology() {
        let registry = EffectRegistry::new();
        let snap = snapshot_from_dsl("distortion | reverb", &registry).unwrap();
        assert!(snap.topology.is_none());
        let dsl = snapshot_to_dsl(&snap);
        let snap2 = snapshot_from_dsl(&dsl, &registry).unwrap();
        assert_eq!(snap.entries.len(), snap2.entries.len());
        assert_eq!(snap.entries[0].effect_id, snap2.entries[0].effect_id);
        assert_eq!(snap.entries[1].effect_id, snap2.entries[1].effect_id);
    }

    // 2. Simple split (2 paths).
    #[test]
    fn topo_rt_simple_split() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(distortion; reverb)", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        assert!(matches!(&spec[0], GraphNode::Split { paths } if paths.len() == 2));
    }

    // 3. Split + dry path.
    #[test]
    fn topo_rt_split_with_dry() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(distortion; -)", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        if let GraphNode::Split { paths } = &spec[0] {
            assert_eq!(paths.len(), 2);
            assert!(matches!(paths[1][0], GraphNode::Dry));
        } else {
            panic!("expected split");
        }
    }

    // 4. Fan topology: split followed by merge effect.
    #[test]
    fn topo_rt_fan_topology() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(distortion; chorus) | limiter", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        assert_eq!(spec.len(), 2);
        assert!(matches!(&spec[0], GraphNode::Split { .. }));
        assert!(matches!(&spec[1], GraphNode::Effect { name, .. } if name == "limiter"));
    }

    // 5. Chains inside split paths.
    #[test]
    fn topo_rt_chains_in_split() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(distortion | chorus; reverb)", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        if let GraphNode::Split { paths } = &spec[0] {
            assert_eq!(paths[0].len(), 2); // distortion | chorus
            assert_eq!(paths[1].len(), 1); // reverb
        } else {
            panic!("expected split");
        }
    }

    // 6. Nested splits.
    #[test]
    fn topo_rt_nested_splits() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(split(chorus; flanger); reverb)", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        if let GraphNode::Split { paths } = &spec[0] {
            assert!(matches!(&paths[0][0], GraphNode::Split { .. }));
            assert!(matches!(&paths[1][0], GraphNode::Effect { name, .. } if name == "reverb"));
        } else {
            panic!("expected split");
        }
    }

    // 7. 3-way split.
    #[test]
    fn topo_rt_three_way_split() {
        let registry = EffectRegistry::new();
        let snap = snap_with_topology("split(distortion; chorus; reverb)", &registry);
        let dsl = snapshot_to_dsl(&snap);
        let spec = parse_graph_dsl(&dsl).unwrap();
        if let GraphNode::Split { paths } = &spec[0] {
            assert_eq!(paths.len(), 3);
        } else {
            panic!("expected 3-way split");
        }
    }

    // 8. Bypassed effect in split path.
    #[test]
    fn topo_rt_bypassed_in_split() {
        let registry = EffectRegistry::new();
        let spec = parse_graph_dsl("split(distortion; reverb)").unwrap();
        let topo = topology_from_spec(&spec);
        let mut entries = collect_entries_preorder(&spec, &registry);
        entries[1].bypassed = true; // bypass reverb
        let snap = GraphSnapshot {
            entries,
            topology: Some(topo),
        };
        let dsl = snapshot_to_dsl(&snap);
        // The bypassed reverb should be prefixed with '!'
        assert!(
            dsl.contains("!reverb"),
            "bypassed effect must emit '!': {dsl}"
        );
    }

    // 9. Params in split paths.
    #[test]
    fn topo_rt_params_in_split_paths() {
        let registry = EffectRegistry::new();
        let spec = parse_graph_dsl("split(distortion; reverb)").unwrap();
        let topo = topology_from_spec(&spec);
        let mut entries = collect_entries_preorder(&spec, &registry);
        // Set distortion drive (param 0) to non-default value 25.
        entries[0].params[0] = 25.0;
        let snap = GraphSnapshot {
            entries,
            topology: Some(topo),
        };
        let dsl = snapshot_to_dsl(&snap);
        // Params should appear in the serialized DSL.
        assert!(
            dsl.contains("distortion:"),
            "drive param must appear: {dsl}"
        );
    }

    // 10. Backward compat: topology: None → uses linear serialization path.
    #[test]
    fn topo_rt_backward_compat_none_is_linear() {
        let registry = EffectRegistry::new();
        let snap = GraphSnapshot {
            entries: vec![
                SnapshotEntry {
                    effect_id: "distortion".to_string(),
                    params: {
                        let e = registry.create("distortion", 48000.0).unwrap();
                        (0..e.effect_param_count())
                            .map(|i| e.effect_param_info(i).unwrap().default)
                            .collect()
                    },
                    bypassed: false,
                },
                SnapshotEntry {
                    effect_id: "reverb".to_string(),
                    params: {
                        let e = registry.create("reverb", 48000.0).unwrap();
                        (0..e.effect_param_count())
                            .map(|i| e.effect_param_info(i).unwrap().default)
                            .collect()
                    },
                    bypassed: false,
                },
            ],
            topology: None,
        };
        let dsl = snapshot_to_dsl(&snap);
        // Linear serialization: no split syntax, just `effect1 | effect2`.
        assert!(
            !dsl.contains("split("),
            "topology: None must use linear format: {dsl}"
        );
        assert!(dsl.contains("distortion"), "{dsl}");
        assert!(dsl.contains("reverb"), "{dsl}");
        let snap2 = snapshot_from_dsl(&dsl, &registry).unwrap();
        assert_eq!(snap2.entries.len(), 2);
    }
}
