//! Standalone `ParamBridge` implementation backed by atomic floats.
//!
//! [`AtomicParamBridge`] stores parameter values in lock-free atomics,
//! mirroring the [`ParameterInfo`](sonido_core::ParameterInfo) index
//! convention. The GUI thread calls `set()`, the audio thread calls `get()`.
//!
//! Created once at startup from the [`EffectRegistry`], it replaces the
//! hand-mapped `SharedParams` struct with a fully generic, registry-driven
//! parameter store.

use parking_lot::RwLock;
use sonido_core::ParamDescriptor;
use sonido_gui_core::ParamBridge;
use sonido_registry::EffectRegistry;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Per-slot parameter storage.
struct SlotState {
    effect_id: &'static str,
    /// Atomic f32 values, one per ParameterInfo parameter.
    values: Vec<AtomicU32>,
    /// Cached descriptors for display and validation.
    descriptors: Vec<ParamDescriptor>,
    /// Bypass state.
    bypassed: AtomicBool,
}

/// Thread-safe parameter bridge for the standalone dashboard.
///
/// Each slot corresponds to one effect in the processing chain, with
/// parameters indexed to match [`ParameterInfo`](sonido_core::ParameterInfo).
/// Values are stored in ParameterInfo units (dB, Hz, ms, 0–100 for percent).
///
/// Slots are protected by an `RwLock`: reads (parameter get/set, sync) take
/// a shared lock, while structural mutations (add/remove slot) take an
/// exclusive lock. Since add/remove only happens on user interaction, the
/// write lock is rarely contended.
pub struct AtomicParamBridge {
    slots: RwLock<Vec<SlotState>>,
}

impl AtomicParamBridge {
    /// Build a bridge from the registry, creating one slot per effect ID.
    ///
    /// Parameters are initialized to their `ParamDescriptor::default` values.
    /// All effects start active (not bypassed).
    pub fn new(registry: &EffectRegistry, effect_ids: &[&'static str], sample_rate: f32) -> Self {
        let slots = effect_ids
            .iter()
            .map(|&id| {
                let (values, descriptors) = if let Some(effect) = registry.create(id, sample_rate) {
                    let count = effect.effect_param_count();
                    let mut vals = Vec::with_capacity(count);
                    let mut descs = Vec::with_capacity(count);
                    for i in 0..count {
                        if let Some(desc) = effect.effect_param_info(i) {
                            vals.push(AtomicU32::new(desc.default.to_bits()));
                            descs.push(desc);
                        }
                    }
                    (vals, descs)
                } else {
                    (Vec::new(), Vec::new())
                };

                SlotState {
                    effect_id: id,
                    values,
                    descriptors,
                    bypassed: AtomicBool::new(false),
                }
            })
            .collect();

        Self {
            slots: RwLock::new(slots),
        }
    }

    /// Set the default bypass state for a slot.
    ///
    /// Called after construction to configure which effects start bypassed.
    /// Uses a read lock because `AtomicBool::store` does not require exclusive
    /// access to the `Vec`.
    pub fn set_default_bypass(&self, slot: usize, bypassed: bool) {
        let slots = self.slots.read();
        if let Some(s) = slots.get(slot) {
            s.bypassed.store(bypassed, Ordering::Relaxed);
        }
    }

    /// Sync all parameter values from the bridge into the effect chain.
    ///
    /// Called once per audio buffer. Reads atomic values and pushes them
    /// into each effect via `set_param()`. Also syncs bypass states.
    pub fn sync_to_chain(&self, chain: &mut crate::chain_manager::ChainManager) {
        let slots = self.slots.read();
        for (slot_idx, slot_state) in slots.iter().enumerate() {
            // Sync bypass
            chain.set_bypassed(slot_idx, slot_state.bypassed.load(Ordering::Relaxed));

            // Sync params
            if let Some(chain_slot) = chain.slot_mut(slot_idx) {
                for (param_idx, atomic_val) in slot_state.values.iter().enumerate() {
                    let val = f32::from_bits(atomic_val.load(Ordering::Relaxed));
                    chain_slot.effect.effect_set_param(param_idx, val);
                }
            }
        }
    }

    /// Appends a new slot, returning its index.
    ///
    /// Called on the audio thread when processing a `ChainCommand::Add`.
    /// Acquires an exclusive write lock. Parameter values are initialized
    /// to each descriptor's default.
    pub fn add_slot(&self, id: &'static str, descriptors: Vec<ParamDescriptor>) -> usize {
        let mut slots = self.slots.write();
        let values = descriptors
            .iter()
            .map(|d| AtomicU32::new(d.default.to_bits()))
            .collect();
        slots.push(SlotState {
            effect_id: id,
            values,
            descriptors,
            bypassed: AtomicBool::new(false),
        });
        slots.len() - 1
    }

    /// Removes a slot via swap-remove.
    ///
    /// Called on the audio thread when processing a `ChainCommand::Remove`.
    /// Acquires an exclusive write lock.
    pub fn remove_slot(&self, slot: usize) {
        let mut slots = self.slots.write();
        if slot < slots.len() {
            slots.swap_remove(slot);
        }
    }
}

impl ParamBridge for AtomicParamBridge {
    fn slot_count(&self) -> usize {
        self.slots.read().len()
    }

    fn effect_id(&self, slot: usize) -> &str {
        // We can't return a reference into the RwLock guard, but effect_id
        // is &'static str so copying is fine.
        self.slots.read().get(slot).map_or("", |s| s.effect_id)
    }

    fn param_count(&self, slot: usize) -> usize {
        self.slots
            .read()
            .get(slot)
            .map_or(0, |s| s.descriptors.len())
    }

    fn param_descriptor(&self, slot: usize, param: usize) -> Option<ParamDescriptor> {
        self.slots
            .read()
            .get(slot)
            .and_then(|s| s.descriptors.get(param))
            .cloned()
    }

    fn get(&self, slot: usize, param: usize) -> f32 {
        self.slots
            .read()
            .get(slot)
            .and_then(|s| s.values.get(param))
            .map(|v| f32::from_bits(v.load(Ordering::Acquire)))
            .unwrap_or(0.0)
    }

    fn set(&self, slot: usize, param: usize, value: f32) {
        let slots = self.slots.read();
        if let Some(s) = slots.get(slot)
            && let Some((atomic, desc)) = s.values.get(param).zip(s.descriptors.get(param))
        {
            let clamped = value.clamp(desc.min, desc.max);
            atomic.store(clamped.to_bits(), Ordering::Release);
        }
    }

    fn is_bypassed(&self, slot: usize) -> bool {
        self.slots
            .read()
            .get(slot)
            .is_some_and(|s| s.bypassed.load(Ordering::Acquire))
    }

    fn set_bypassed(&self, slot: usize, bypassed: bool) {
        let slots = self.slots.read();
        if let Some(s) = slots.get(slot) {
            s.bypassed.store(bypassed, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_from_registry() {
        let registry = EffectRegistry::new();
        let bridge = AtomicParamBridge::new(&registry, &["distortion", "reverb"], 48000.0);

        assert_eq!(bridge.slot_count(), 2);
        assert_eq!(bridge.effect_id(0), "distortion");
        assert_eq!(bridge.effect_id(1), "reverb");
        assert!(bridge.param_count(0) > 0);
        assert!(bridge.param_count(1) > 0);
    }

    #[test]
    fn get_set_clamps() {
        let registry = EffectRegistry::new();
        let bridge = AtomicParamBridge::new(&registry, &["distortion"], 48000.0);

        // Read default
        let desc = bridge.param_descriptor(0, 0).unwrap();
        assert_eq!(bridge.get(0, 0), desc.default);

        // Set within range
        bridge.set(0, 0, 10.0);
        assert_eq!(bridge.get(0, 0), 10.0);

        // Clamp above max
        bridge.set(0, 0, 999.0);
        assert_eq!(bridge.get(0, 0), desc.max);
    }

    #[test]
    fn bypass_states() {
        let registry = EffectRegistry::new();
        let bridge = AtomicParamBridge::new(&registry, &["chorus", "gate"], 48000.0);

        assert!(!bridge.is_bypassed(0));
        assert!(!bridge.is_bypassed(1));

        bridge.set_bypassed(1, true);
        assert!(!bridge.is_bypassed(0));
        assert!(bridge.is_bypassed(1));
    }

    #[test]
    fn out_of_range_safe() {
        let registry = EffectRegistry::new();
        let bridge = AtomicParamBridge::new(&registry, &["distortion"], 48000.0);

        assert_eq!(bridge.get(99, 0), 0.0);
        assert_eq!(bridge.param_count(99), 0);
        assert_eq!(bridge.effect_id(99), "");
        assert!(!bridge.is_bypassed(99));

        // These should not panic
        bridge.set(99, 0, 1.0);
        bridge.set_bypassed(99, true);
    }

    #[test]
    fn add_slot_appends() {
        let registry = EffectRegistry::new();
        let bridge = AtomicParamBridge::new(&registry, &["distortion"], 48000.0);
        assert_eq!(bridge.slot_count(), 1);

        // Create an effect to extract its descriptors
        let effect = registry.create("reverb", 48000.0).unwrap();
        let count = effect.effect_param_count();
        let descs: Vec<_> = (0..count)
            .filter_map(|i| effect.effect_param_info(i))
            .collect();

        let idx = bridge.add_slot("reverb", descs);
        assert_eq!(idx, 1);
        assert_eq!(bridge.slot_count(), 2);
        assert_eq!(bridge.effect_id(1), "reverb");
        assert!(bridge.param_count(1) > 0);
    }

    #[test]
    fn remove_slot_swap_removes() {
        let registry = EffectRegistry::new();
        let bridge =
            AtomicParamBridge::new(&registry, &["distortion", "compressor", "reverb"], 48000.0);
        assert_eq!(bridge.slot_count(), 3);

        // Remove slot 0 → "reverb" (last) swaps into position 0
        bridge.remove_slot(0);
        assert_eq!(bridge.slot_count(), 2);
        assert_eq!(bridge.effect_id(0), "reverb");
        assert_eq!(bridge.effect_id(1), "compressor");
    }
}
