//! Re-export of [`GraphEngine`] from `sonido-core`.
//!
//! `GraphEngine` was moved to `sonido_core::graph::engine` to enable `no_std`
//! and wasm targets. This module re-exports it for backwards compatibility.

pub use sonido_core::graph::{GraphEngine, GraphSnapshot, SnapshotEntry};
