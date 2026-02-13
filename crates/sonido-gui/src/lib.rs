//! Sonido GUI - Professional DSP effect processor interface
//!
//! This crate provides a real-time audio effects GUI built on egui,
//! designed for musicians and audio engineers.

pub mod app;
pub mod atomic_param_bridge;
pub mod audio_bridge;
pub mod chain_manager;
pub mod chain_view;
pub mod effects_ui;
pub mod file_player;
pub mod preset_manager;
pub mod theme;
pub mod widgets;

pub use app::SonidoApp;
pub use audio_bridge::AtomicParam;
pub use preset_manager::{PresetEntry, PresetManager, PresetSource};
pub use sonido_config::Preset;
pub use theme::Theme;
