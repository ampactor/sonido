//! Chain/graph command types for GUI→audio thread communication.
//!
//! [`GraphCommand`] is sent over a lock-free channel to mutate the effect
//! topology from the GUI thread. The audio thread drains commands at the start
//! of each buffer.

use sonido_core::ParamDescriptor;
use sonido_core::graph::GraphEngine;
use sonido_gui_core::SlotIndex;
use sonido_registry::EffectWithParams;

/// A command to mutate the effect topology from the GUI thread.
///
/// Commands are sent over a lock-free channel and drained by the audio thread
/// at the start of each buffer. This decouples GUI interaction from real-time
/// processing.
pub enum GraphCommand {
    /// Add a new effect to the end of the linear chain.
    Add {
        /// Effect identifier (e.g., `"reverb"`, `"distortion"`).
        id: &'static str,
        /// Pre-created effect instance (constructed on the GUI thread).
        effect: Box<dyn EffectWithParams + Send>,
        /// Parameter descriptors for bridge registration (applied atomically on audio thread).
        descriptors: Vec<ParamDescriptor>,
    },
    /// Remove an effect slot from the chain.
    Remove {
        /// Slot index to remove.
        slot: SlotIndex,
    },
    /// Replace the entire topology with a pre-compiled DAG.
    ///
    /// The GUI thread builds the graph, compiles it, and creates a
    /// [`GraphEngine`] via [`new_dag()`](GraphEngine::new_dag). The audio
    /// thread swaps the entire engine atomically. The old engine drops on
    /// the audio thread (Vec drops only, no syscalls).
    ReplaceTopology {
        /// Pre-compiled graph engine, ready to process audio.
        engine: Box<GraphEngine>,
        /// Effect IDs in slot order (parallel to the manifest used to build the engine).
        effect_ids: Vec<&'static str>,
        /// Parameter descriptors per slot (for bridge rebuild).
        slot_descriptors: Vec<Vec<ParamDescriptor>>,
    },
}
