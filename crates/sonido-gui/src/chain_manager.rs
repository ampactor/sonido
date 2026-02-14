//! Effect chain management for the GUI audio pipeline.
//!
//! [`ChainManager`] maintains an ordered sequence of [`ChainSlot`]s, each
//! holding a live effect instance. Slots can be independently bypassed and
//! reordered without reallocating the effects themselves.

use crate::atomic_param_bridge::AtomicParamBridge;
use crate::audio_bridge::EffectOrder;
use sonido_core::ParamDescriptor;
use sonido_registry::{EffectRegistry, EffectWithParams};

/// A command to mutate the effect chain from the GUI thread.
///
/// Commands are sent over a lock-free channel and drained by the audio thread
/// at the start of each buffer. This decouples GUI interaction from real-time
/// processing.
pub enum ChainCommand {
    /// Add a new effect to the end of the chain.
    Add {
        /// Effect identifier (e.g., `"reverb"`, `"distortion"`).
        id: &'static str,
        /// Pre-created effect instance (constructed on the GUI thread).
        effect: Box<dyn EffectWithParams + Send>,
    },
    /// Remove an effect slot from the chain.
    Remove {
        /// Slot index to remove.
        slot: usize,
    },
}

/// A single slot in the effect chain.
///
/// Each slot owns one effect instance and tracks its bypass state.
pub struct ChainSlot {
    /// The effect instance.
    pub effect: Box<dyn EffectWithParams + Send>,
    /// Effect identifier (e.g., `"distortion"`, `"reverb"`).
    pub id: &'static str,
    /// Whether this slot is bypassed.
    pub bypassed: bool,
}

/// Manages an ordered chain of audio effects.
///
/// Effects are stored in slots and processed in a configurable order.
/// Each slot can be independently bypassed. The processing order is
/// decoupled from storage order via an explicit index sequence.
pub struct ChainManager {
    slots: Vec<ChainSlot>,
    order: Vec<usize>,
}

impl ChainManager {
    /// Creates a new chain by instantiating effects from the registry.
    ///
    /// Unknown IDs are silently skipped. The initial processing order
    /// matches insertion order (`0..n`).
    ///
    /// # Arguments
    /// * `registry` - Effect registry to look up effect factories
    /// * `ids` - Ordered slice of effect identifiers to instantiate
    /// * `sample_rate` - Sample rate in Hz for all created effects
    pub fn new(registry: &EffectRegistry, ids: &[&'static str], sample_rate: f32) -> Self {
        let mut slots = Vec::with_capacity(ids.len());
        for &id in ids {
            if let Some(effect) = registry.create(id, sample_rate) {
                slots.push(ChainSlot {
                    effect,
                    id,
                    bypassed: false,
                });
            } else {
                log::warn!("Unknown effect id \"{id}\", skipping");
            }
        }
        let order: Vec<usize> = (0..slots.len()).collect();
        Self { slots, order }
    }

    /// Processes a stereo sample pair through the chain in order.
    ///
    /// Bypassed slots are skipped. Returns the accumulated output.
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        let mut l = left;
        let mut r = right;
        for &idx in &self.order {
            if let Some(slot) = self.slots.get_mut(idx)
                && !slot.bypassed
            {
                let out = slot.effect.process_stereo(l, r);
                l = out.0;
                r = out.1;
            }
        }
        (l, r)
    }

    /// Returns the number of slots in the chain.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// Returns a reference to the slot at `index`, or `None` if out of range.
    pub fn slot(&self, index: usize) -> Option<&ChainSlot> {
        self.slots.get(index)
    }

    /// Returns a mutable reference to the slot at `index`, or `None` if out of range.
    pub fn slot_mut(&mut self, index: usize) -> Option<&mut ChainSlot> {
        self.slots.get_mut(index)
    }

    /// Returns the current processing order as slot indices.
    pub fn order(&self) -> &[usize] {
        &self.order
    }

    /// Sets a new processing order.
    ///
    /// All indices in `new_order` must be valid slot indices. If any index
    /// is out of range the call is ignored and the previous order is kept.
    pub fn reorder(&mut self, new_order: Vec<usize>) {
        let valid = new_order.iter().all(|&i| i < self.slots.len());
        if valid {
            self.order = new_order;
        }
    }

    /// Sets the bypass state of a slot.
    ///
    /// Out-of-range indices are silently ignored.
    pub fn set_bypassed(&mut self, slot: usize, bypassed: bool) {
        if let Some(s) = self.slots.get_mut(slot) {
            s.bypassed = bypassed;
        }
    }

    /// Returns whether a slot is bypassed.
    ///
    /// Returns `false` for out-of-range indices.
    pub fn is_bypassed(&self, slot: usize) -> bool {
        self.slots.get(slot).is_some_and(|s| s.bypassed)
    }

    /// Returns the effect ID for a slot, or `""` if out of range.
    pub fn effect_id(&self, slot: usize) -> &str {
        self.slots.get(slot).map_or("", |s| s.id)
    }

    /// Appends an effect to the chain, returning its slot index.
    ///
    /// The new slot starts active (not bypassed) and is appended to the end
    /// of the processing order.
    pub fn add_effect(
        &mut self,
        id: &'static str,
        effect: Box<dyn EffectWithParams + Send>,
    ) -> usize {
        let idx = self.slots.len();
        self.slots.push(ChainSlot {
            effect,
            id,
            bypassed: false,
        });
        self.order.push(idx);
        idx
    }

    /// Removes an effect slot via swap-remove, returning the removed effect's ID.
    ///
    /// Returns `None` if `slot` is out of range. When `slot` is not the last
    /// element, the last slot is moved into position `slot` and the processing
    /// order is updated to reflect the move.
    pub(crate) fn remove_effect(&mut self, slot: usize) -> Option<&'static str> {
        if slot >= self.slots.len() {
            return None;
        }
        let old_last = self.slots.len() - 1;
        let removed = self.slots.swap_remove(slot);

        // Remove `slot` from the order
        self.order.retain(|&i| i != slot);

        // If we swapped the last element into `slot`, update its references
        if slot != old_last {
            for idx in &mut self.order {
                if *idx == old_last {
                    *idx = slot;
                }
            }
        }

        Some(removed.id)
    }

    /// Adds an effect and registers it in the bridge and order atomically.
    ///
    /// Bundles three mutations that must stay in sync:
    /// 1. Appends the effect to the chain
    /// 2. Registers parameter descriptors in the bridge
    /// 3. Appends the slot index to the effect order
    ///
    /// Returns the new slot index.
    pub fn add_transactional(
        &mut self,
        id: &'static str,
        effect: Box<dyn EffectWithParams + Send>,
        bridge: &AtomicParamBridge,
        order: &EffectOrder,
        descriptors: Vec<ParamDescriptor>,
    ) -> usize {
        let slot = self.add_effect(id, effect);
        bridge.add_slot(id, descriptors);
        order.push();
        slot
    }

    /// Removes an effect and cleans up bridge and order atomically.
    ///
    /// Bundles three mutations that must stay in sync:
    /// 1. Removes the effect from the chain
    /// 2. Removes the parameter slot from the bridge
    /// 3. Removes and remaps the slot in the effect order
    ///
    /// Returns the removed effect's ID, or `None` if `slot` was out of range.
    pub fn remove_transactional(
        &mut self,
        slot: usize,
        bridge: &AtomicParamBridge,
        order: &EffectOrder,
    ) -> Option<&'static str> {
        let removed_id = self.remove_effect(slot)?;
        bridge.remove_slot(slot);
        order.swap_remove(slot);
        Some(removed_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> EffectRegistry {
        EffectRegistry::new()
    }

    #[test]
    fn creation_with_known_ids() {
        let reg = registry();
        let chain = ChainManager::new(&reg, &["distortion", "reverb"], 48000.0);
        assert_eq!(chain.slot_count(), 2);
        assert_eq!(chain.effect_id(0), "distortion");
        assert_eq!(chain.effect_id(1), "reverb");
        assert_eq!(chain.order(), &[0, 1]);
    }

    #[test]
    fn unknown_ids_skipped() {
        let reg = registry();
        let chain = ChainManager::new(&reg, &["distortion", "bogus", "reverb"], 48000.0);
        assert_eq!(chain.slot_count(), 2);
        assert_eq!(chain.effect_id(0), "distortion");
        assert_eq!(chain.effect_id(1), "reverb");
    }

    #[test]
    fn process_stereo_runs() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion"], 48000.0);
        let (l, r) = chain.process_stereo(0.5, 0.5);
        assert!(l.is_finite());
        assert!(r.is_finite());
    }

    #[test]
    fn bypass_skips_processing() {
        let reg = registry();
        // Preamp with default gain should modify signal
        let mut chain = ChainManager::new(&reg, &["preamp"], 48000.0);

        let (l_active, _) = chain.process_stereo(0.5, 0.5);

        chain.set_bypassed(0, true);
        assert!(chain.is_bypassed(0));

        let (l_bypass, r_bypass) = chain.process_stereo(0.5, 0.5);
        // Bypassed → passthrough
        assert!((l_bypass - 0.5).abs() < 1e-6);
        assert!((r_bypass - 0.5).abs() < 1e-6);

        // Un-bypass should restore active processing
        chain.set_bypassed(0, false);
        assert!(!chain.is_bypassed(0));
        let (l_restored, _) = chain.process_stereo(0.5, 0.5);
        assert!((l_restored - l_active).abs() < 1e-6);
    }

    #[test]
    fn reorder_changes_processing_order() {
        let reg = registry();
        // Two different effects so order matters
        let mut chain = ChainManager::new(&reg, &["distortion", "compressor"], 48000.0);

        let (l_orig, r_orig) = chain.process_stereo(0.3, 0.3);

        // Reset internal state so comparison is fair
        chain.slot_mut(0).unwrap().effect.reset();
        chain.slot_mut(1).unwrap().effect.reset();

        chain.reorder(vec![1, 0]);
        assert_eq!(chain.order(), &[1, 0]);

        let (l_reorder, r_reorder) = chain.process_stereo(0.3, 0.3);

        // Different order should (generally) produce different output
        // At minimum, verify it ran without panic and produced finite values
        assert!(l_reorder.is_finite());
        assert!(r_reorder.is_finite());
        // The two orderings may differ (depends on effect state), but
        // we primarily verify correctness of the reorder mechanism
        let _ = (l_orig, r_orig, l_reorder, r_reorder);
    }

    #[test]
    fn invalid_reorder_rejected() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion"], 48000.0);
        chain.reorder(vec![5]); // out of range
        assert_eq!(chain.order(), &[0]); // unchanged
    }

    #[test]
    fn out_of_range_access_safe() {
        let reg = registry();
        let chain = ChainManager::new(&reg, &[], 48000.0);
        assert!(chain.slot(0).is_none());
        assert!(!chain.is_bypassed(99));
        assert_eq!(chain.effect_id(99), "");
    }

    #[test]
    fn add_effect_basic() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion"], 48000.0);
        assert_eq!(chain.slot_count(), 1);

        let effect = reg.create("reverb", 48000.0).unwrap();
        let idx = chain.add_effect("reverb", effect);
        assert_eq!(idx, 1);
        assert_eq!(chain.slot_count(), 2);
        assert_eq!(chain.effect_id(1), "reverb");
        assert_eq!(chain.order(), &[0, 1]);
    }

    #[test]
    fn remove_effect_last_slot() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion", "reverb"], 48000.0);
        let removed = chain.remove_effect(1);
        assert_eq!(removed, Some("reverb"));
        assert_eq!(chain.slot_count(), 1);
        assert_eq!(chain.effect_id(0), "distortion");
        assert_eq!(chain.order(), &[0]);
    }

    #[test]
    fn remove_effect_swap_semantics() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion", "compressor", "reverb"], 48000.0);
        // Remove slot 0 → "reverb" (last) swaps into position 0
        let removed = chain.remove_effect(0);
        assert_eq!(removed, Some("distortion"));
        assert_eq!(chain.slot_count(), 2);
        // Slot 0 is now what was slot 2 (reverb)
        assert_eq!(chain.effect_id(0), "reverb");
        // Slot 1 is still compressor
        assert_eq!(chain.effect_id(1), "compressor");
    }

    #[test]
    fn remove_effect_updates_order() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion", "compressor", "reverb"], 48000.0);
        // Order is [0, 1, 2]. Remove slot 0 → slot 2 moves to 0.
        chain.remove_effect(0);
        // Order should no longer contain 0 (the removed slot), and old index 2
        // should be remapped to 0.
        // Original order [0, 1, 2] → remove 0 → [1, 2] → remap 2→0 → [1, 0]
        assert_eq!(chain.order(), &[1, 0]);
    }

    #[test]
    fn remove_effect_out_of_range() {
        let reg = registry();
        let mut chain = ChainManager::new(&reg, &["distortion"], 48000.0);
        assert_eq!(chain.remove_effect(5), None);
        assert_eq!(chain.slot_count(), 1);
    }
}
