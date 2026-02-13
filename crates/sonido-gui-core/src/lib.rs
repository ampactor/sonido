//! Shared GUI components for the Sonido DSP framework.
//!
//! This crate provides reusable widgets, theme definitions, and the
//! [`ParamBridge`] trait that decouples GUI parameter access from the
//! underlying storage mechanism. Used by both the standalone dashboard
//! (`sonido-gui`) and VST/CLAP plugins (`sonido-plugin`).
//!
//! # Modules
//!
//! - [`param_bridge`] — Parameter bridge trait for GUI↔audio communication
//! - [`theme`] — Visual styling constants and egui theme application
//! - [`widgets`] — Audio-specific widgets (knobs, meters, toggles)

pub mod param_bridge;
pub mod theme;
pub mod widgets;

pub use param_bridge::ParamBridge;
pub use theme::Theme;
pub use widgets::{BypassToggle, FootswitchToggle, GainReductionMeter, Knob, LevelMeter};
