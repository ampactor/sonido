//! Parameter bridge trait for decoupled GUI-to-audio parameter communication.
//!
//! [`ParamBridge`] abstracts over the parameter storage mechanism, enabling
//! the same GUI widgets to work in both the standalone dashboard (backed by
//! atomic floats) and VST/CLAP plugins (backed by nih-plug `FloatParam`).
//!
//! # Architecture
//!
//! The bridge models a fixed number of **slots**, each representing one effect
//! in the processing chain. Each slot exposes parameters via index, matching
//! the [`ParameterInfo`](sonido_core::ParameterInfo) convention from sonido-core.
//!
//! ```text
//! GUI widgets ──► ParamBridge::set(slot, param, value)
//!                         │
//!                    ┌────┴────┐
//!                    │ Atomic  │  (standalone)
//!                    │ NihPlug │  (plugin)
//!                    └────┬────┘
//!                         │
//! Audio thread ◄── ParamBridge::get(slot, param)
//! ```

use core::fmt;
use sonido_core::ParamDescriptor;

/// Type-safe index into the effect chain slot array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotIndex(pub usize);

impl fmt::Display for SlotIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for SlotIndex {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

/// Type-safe index into a slot's parameter array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamIndex(pub usize);

impl fmt::Display for ParamIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for ParamIndex {
    fn from(v: usize) -> Self {
        Self(v)
    }
}

/// Trait for bridging parameter values between GUI and audio threads.
///
/// Implementations must be thread-safe — `get` and `set` may be called
/// from different threads simultaneously. Index-based access mirrors
/// [`ParameterInfo`](sonido_core::ParameterInfo) for zero-cost mapping.
pub trait ParamBridge: Send + Sync {
    /// Number of effect slots in the chain.
    fn slot_count(&self) -> usize;

    /// Effect identifier for the given slot (e.g., `"distortion"`, `"reverb"`).
    ///
    /// Returns `""` if the slot index is out of range.
    fn effect_id(&self, slot: SlotIndex) -> &str;

    /// Number of parameters for the effect in the given slot.
    ///
    /// Returns `0` if the slot index is out of range.
    fn param_count(&self, slot: SlotIndex) -> usize;

    /// Parameter descriptor for display and validation.
    ///
    /// Returns `None` if slot or param index is out of range.
    fn param_descriptor(&self, slot: SlotIndex, param: ParamIndex) -> Option<ParamDescriptor>;

    /// Read the current value of a parameter.
    ///
    /// Returns the parameter's default value (or `0.0`) if indices are out of range.
    fn get(&self, slot: SlotIndex, param: ParamIndex) -> f32;

    /// Write a new value for a parameter.
    ///
    /// Out-of-range indices are silently ignored. Values are clamped to the
    /// parameter's valid range by the implementation.
    fn set(&self, slot: SlotIndex, param: ParamIndex, value: f32);

    /// Whether the effect in the given slot is bypassed.
    ///
    /// Returns `false` if the slot index is out of range.
    fn is_bypassed(&self, slot: SlotIndex) -> bool;

    /// Set the bypass state for the effect in the given slot.
    ///
    /// Out-of-range indices are silently ignored.
    fn set_bypassed(&self, slot: SlotIndex, bypassed: bool);
}
