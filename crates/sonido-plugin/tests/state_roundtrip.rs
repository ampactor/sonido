//! State save/restore roundtrip tests for all sonido effects.
//!
//! Verifies that parameter state survives JSON serialization, including
//! edge cases like min/max values and non-default configurations.

use sonido_plugin::SonidoShared;
use sonido_registry::EffectRegistry;

/// Serialize shared state to JSON bytes (same format as CLAP state save).
fn serialize_state(shared: &SonidoShared) -> Vec<u8> {
    let mut state = serde_json::Map::new();
    for (i, desc) in shared.descriptors().iter().enumerate() {
        if let Some(val) = shared.get_value(i) {
            state.insert(
                desc.id.0.to_string(),
                serde_json::Value::from(f64::from(val)),
            );
        }
    }
    serde_json::to_vec(&serde_json::Value::Object(state)).unwrap()
}

/// Deserialize JSON bytes into shared state (same format as CLAP state load).
fn deserialize_state(shared: &SonidoShared, json: &[u8]) {
    let value: serde_json::Value = serde_json::from_slice(json).unwrap();
    let obj = value.as_object().unwrap();
    for (key, val) in obj {
        let id: u32 = key.parse().unwrap();
        let v = val.as_f64().unwrap();
        if let Some(index) = shared.index_by_id(id) {
            shared.set_value(index, v as f32);
        }
    }
}

#[test]
fn all_effects_state_roundtrip_defaults() {
    let registry = EffectRegistry::new();

    for desc in registry.all_effects() {
        let shared = SonidoShared::new(desc.id, None);
        let json = serialize_state(&shared);

        // Load into a fresh instance.
        let shared2 = SonidoShared::new(desc.id, None);
        deserialize_state(&shared2, &json);

        // All values should match.
        for i in 0..shared.param_count() {
            let v1 = shared.get_value(i).unwrap();
            let v2 = shared2.get_value(i).unwrap();
            assert_eq!(
                v1, v2,
                "effect {} param {i} default roundtrip mismatch: {v1} != {v2}",
                desc.id
            );
        }
    }
}

#[test]
fn all_effects_state_roundtrip_extremes() {
    let registry = EffectRegistry::new();

    for desc in registry.all_effects() {
        let shared = SonidoShared::new(desc.id, None);

        // Set all params to their maximum values.
        for (i, param) in shared.descriptors().iter().enumerate() {
            shared.set_value(i, param.max);
        }
        let json_max = serialize_state(&shared);

        let shared2 = SonidoShared::new(desc.id, None);
        deserialize_state(&shared2, &json_max);

        for i in 0..shared.param_count() {
            let v1 = shared.get_value(i).unwrap();
            let v2 = shared2.get_value(i).unwrap();
            assert_eq!(
                v1, v2,
                "effect {} param {i} max roundtrip mismatch: {v1} != {v2}",
                desc.id
            );
        }

        // Set all params to their minimum values.
        for (i, param) in shared.descriptors().iter().enumerate() {
            shared.set_value(i, param.min);
        }
        let json_min = serialize_state(&shared);

        let shared3 = SonidoShared::new(desc.id, None);
        deserialize_state(&shared3, &json_min);

        for i in 0..shared.param_count() {
            let v1 = shared.get_value(i).unwrap();
            let v3 = shared3.get_value(i).unwrap();
            assert_eq!(
                v1, v3,
                "effect {} param {i} min roundtrip mismatch: {v1} != {v3}",
                desc.id
            );
        }
    }
}

#[test]
fn all_effects_state_roundtrip_midpoints() {
    let registry = EffectRegistry::new();

    for desc in registry.all_effects() {
        let shared = SonidoShared::new(desc.id, None);

        // Set all params to midpoint between min and max.
        for (i, param) in shared.descriptors().iter().enumerate() {
            let mid = (param.min + param.max) / 2.0;
            shared.set_value(i, mid);
        }
        let json = serialize_state(&shared);

        let shared2 = SonidoShared::new(desc.id, None);
        deserialize_state(&shared2, &json);

        for i in 0..shared.param_count() {
            let v1 = shared.get_value(i).unwrap();
            let v2 = shared2.get_value(i).unwrap();
            // f32→f64→f32 roundtrip may lose precision, allow small epsilon.
            assert!(
                (v1 - v2).abs() < 1e-4,
                "effect {} param {i} midpoint roundtrip mismatch: {v1} vs {v2}",
                desc.id
            );
        }
    }
}

#[test]
fn state_ignores_unknown_param_ids() {
    let shared = SonidoShared::new("distortion", None);
    let json = br#"{"200": 15.0, "99999": 42.0, "201": 0.8}"#;
    deserialize_state(&shared, json);

    // Known params should be set.
    assert_eq!(
        shared.get_value(shared.index_by_id(200).unwrap()).unwrap(),
        15.0
    );
    assert_eq!(
        shared.get_value(shared.index_by_id(201).unwrap()).unwrap(),
        0.8
    );
    // Unknown ID 99999 should be silently ignored (no panic).
}

#[test]
fn state_clamps_out_of_range_values() {
    let shared = SonidoShared::new("distortion", None);
    let desc = shared.descriptor(0).unwrap();

    // Set value way above max via JSON.
    let json = format!("{{\"{}\" : 999999.0}}", desc.id.0);
    deserialize_state(&shared, json.as_bytes());

    // Should be clamped to max.
    assert_eq!(shared.get_value(0).unwrap(), desc.max);
}
