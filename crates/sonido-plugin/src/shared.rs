//! Thread-safe shared state for sonido CLAP plugins.
//!
//! `SonidoShared` lives for the lifetime of the plugin instance and is
//! accessible from both the main thread (params, state) and the audio
//! thread (processing). Parameter values are stored as atomic `u32`
//! (f32 bit-cast) for lock-free access.

use sonido_core::ParamDescriptor;
use sonido_registry::EffectRegistry;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Inner storage for plugin shared state.
///
/// Holds all data behind an `Arc` so that `SonidoShared` can be cheaply
/// cloned into `'static + Send` GUI closures.
struct SonidoSharedData {
    /// Registry ID of the effect (e.g., `"distortion"`).
    effect_id: &'static str,
    /// Parameter descriptors, indexed by parameter position.
    descriptors: Vec<ParamDescriptor>,
    /// Current parameter values as f32 bit-cast to u32 for atomic access.
    values: Vec<AtomicU32>,
}

/// Shared state accessible from all plugin threads.
///
/// Holds the effect's parameter descriptors (immutable after construction)
/// and the current parameter values as atomics. The audio processor reads
/// values here; the main thread writes them in response to host automation
/// or GUI interaction.
///
/// Wraps an `Arc<SonidoSharedData>` so it can be cloned into `'static + Send`
/// closures (e.g., the egui-baseview render loop) without lifetime issues.
#[derive(Clone)]
pub struct SonidoShared {
    inner: Arc<SonidoSharedData>,
}

impl SonidoShared {
    /// Create shared state for the given effect.
    ///
    /// Instantiates a temporary effect to extract parameter descriptors,
    /// then initializes all values to their defaults.
    pub fn new(effect_id: &'static str) -> Self {
        let registry = EffectRegistry::new();
        // Use a dummy sample rate — we only need descriptors, not DSP state.
        let effect = registry
            .create(effect_id, 48000.0)
            .expect("Unknown effect ID in SonidoShared::new");

        let count = effect.effect_param_count();
        let mut descriptors = Vec::with_capacity(count);
        let mut values = Vec::with_capacity(count);

        for i in 0..count {
            if let Some(desc) = effect.effect_param_info(i) {
                values.push(AtomicU32::new(desc.default.to_bits()));
                descriptors.push(desc);
            }
        }

        Self {
            inner: Arc::new(SonidoSharedData {
                effect_id,
                descriptors,
                values,
            }),
        }
    }

    /// Registry ID of the wrapped effect.
    pub fn effect_id(&self) -> &'static str {
        self.inner.effect_id
    }

    /// Number of parameters.
    pub fn param_count(&self) -> usize {
        self.inner.descriptors.len()
    }

    /// Get parameter descriptor by index.
    pub fn descriptor(&self, index: usize) -> Option<&ParamDescriptor> {
        self.inner.descriptors.get(index)
    }

    /// All parameter descriptors.
    pub fn descriptors(&self) -> &[ParamDescriptor] {
        &self.inner.descriptors
    }

    /// Find parameter index by stable `ParamId`.
    pub fn index_by_id(&self, id: u32) -> Option<usize> {
        self.inner.descriptors.iter().position(|d| d.id.0 == id)
    }

    /// Read the current value of a parameter (lock-free).
    pub fn get_value(&self, index: usize) -> Option<f32> {
        self.inner
            .values
            .get(index)
            .map(|v| f32::from_bits(v.load(Ordering::Acquire)))
    }

    /// Write a parameter value (lock-free). Clamps to descriptor bounds.
    pub fn set_value(&self, index: usize, value: f32) {
        if let Some((atomic, desc)) = self
            .inner
            .values
            .get(index)
            .zip(self.inner.descriptors.get(index))
        {
            let clamped = value.clamp(desc.min, desc.max);
            atomic.store(clamped.to_bits(), Ordering::Release);
        }
    }

    /// Sync all current parameter values into an effect instance.
    ///
    /// Called when the audio processor is activated or after loading state.
    pub fn apply_to_effect(&self, effect: &mut dyn sonido_registry::EffectWithParams) {
        for (i, atomic) in self.inner.values.iter().enumerate() {
            let val = f32::from_bits(atomic.load(Ordering::Acquire));
            effect.effect_set_param(i, val);
        }
    }
}

impl clack_plugin::prelude::PluginShared<'_> for SonidoShared {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_new_creates_all_params() {
        let shared = SonidoShared::new("distortion");
        assert_eq!(shared.effect_id(), "distortion");
        assert_eq!(shared.param_count(), 5);
        assert!(shared.param_count() > 0);
    }

    #[test]
    fn shared_defaults_match_descriptors() {
        let shared = SonidoShared::new("reverb");
        for (i, desc) in shared.descriptors().iter().enumerate() {
            let val = shared.get_value(i).unwrap();
            assert_eq!(
                val, desc.default,
                "param {i} ({}) default mismatch: got {val}, expected {}",
                desc.name, desc.default
            );
        }
    }

    #[test]
    fn shared_set_value_clamps() {
        let shared = SonidoShared::new("distortion");
        let desc = shared.descriptor(0).unwrap();

        // Set above max — should clamp.
        shared.set_value(0, desc.max + 100.0);
        assert_eq!(shared.get_value(0).unwrap(), desc.max);

        // Set below min — should clamp.
        shared.set_value(0, desc.min - 100.0);
        assert_eq!(shared.get_value(0).unwrap(), desc.min);
    }

    #[test]
    fn shared_index_by_id_finds_params() {
        let shared = SonidoShared::new("distortion");
        // Distortion base ID is 200.
        assert_eq!(shared.index_by_id(200), Some(0));
        assert_eq!(shared.index_by_id(201), Some(1));
        // Non-existent ID.
        assert_eq!(shared.index_by_id(999), None);
    }

    #[test]
    fn shared_out_of_range_safe() {
        let shared = SonidoShared::new("distortion");
        assert_eq!(shared.get_value(999), None);
        assert_eq!(shared.descriptor(999), None);
        // Should not panic.
        shared.set_value(999, 1.0);
    }

    #[test]
    fn shared_apply_to_effect() {
        let shared = SonidoShared::new("distortion");
        // Set a non-default value.
        shared.set_value(0, 20.0);

        let registry = EffectRegistry::new();
        let mut effect = registry.create("distortion", 48000.0).unwrap();
        shared.apply_to_effect(effect.as_mut());

        // Verify the effect got the value.
        assert_eq!(effect.effect_get_param(0), 20.0);
    }

    #[test]
    fn all_effects_create_shared() {
        let registry = EffectRegistry::new();
        for desc in registry.all_effects() {
            let shared = SonidoShared::new(desc.id);
            assert!(shared.param_count() > 0, "effect {} has no params", desc.id);
            // Verify all param IDs are unique within the effect.
            let ids: Vec<u32> = shared.descriptors().iter().map(|d| d.id.0).collect();
            for (i, id) in ids.iter().enumerate() {
                assert_eq!(
                    shared.index_by_id(*id),
                    Some(i),
                    "effect {} param ID {id} lookup mismatch",
                    desc.id
                );
            }
        }
    }

    #[test]
    fn state_roundtrip_json() {
        let shared = SonidoShared::new("compressor");

        // Set some non-default values.
        shared.set_value(0, 10.0);
        shared.set_value(1, 42.0);

        // Serialize.
        let mut state = serde_json::Map::new();
        for (i, desc) in shared.descriptors().iter().enumerate() {
            if let Some(val) = shared.get_value(i) {
                state.insert(
                    desc.id.0.to_string(),
                    serde_json::Value::from(f64::from(val)),
                );
            }
        }
        let json = serde_json::to_vec(&serde_json::Value::Object(state)).unwrap();

        // Create fresh shared, load state.
        let shared2 = SonidoShared::new("compressor");
        let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
        let obj = value.as_object().unwrap();
        for (key, val) in obj {
            let id: u32 = key.parse().unwrap();
            let v = val.as_f64().unwrap();
            if let Some(index) = shared2.index_by_id(id) {
                shared2.set_value(index, v as f32);
            }
        }

        // Verify values match.
        for i in 0..shared.param_count() {
            assert_eq!(
                shared.get_value(i).unwrap(),
                shared2.get_value(i).unwrap(),
                "param {i} value mismatch after roundtrip"
            );
        }
    }
}
