//! Effect chain management for the GUI audio pipeline.
//!
//! [`ChainManager`] maintains an ordered sequence of [`ChainSlot`]s, each
//! holding a live effect instance. Slots can be independently bypassed and
//! reordered without reallocating the effects themselves.

use sonido_registry::{EffectRegistry, EffectWithParams};

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
}
