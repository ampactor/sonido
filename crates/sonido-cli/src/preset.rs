//! Preset file format for effect chains.
//!
//! Presets are stored as TOML files containing effect configurations
//! that can be loaded by both process and realtime commands.

use serde::Deserialize;
use std::collections::HashMap;

/// Preset file format.
#[derive(Debug, Deserialize)]
pub struct Preset {
    /// Name of the preset
    pub name: String,
    /// Optional description
    #[serde(default)]
    #[allow(dead_code)]
    pub description: Option<String>,
    /// Sample rate hint (not currently used, for future compatibility)
    #[serde(default = "default_sample_rate")]
    #[allow(dead_code)]
    pub sample_rate: u32,
    /// List of effects in the chain
    pub effects: Vec<EffectConfig>,
}

fn default_sample_rate() -> u32 {
    48000
}

/// Configuration for a single effect in a preset.
#[derive(Debug, Deserialize)]
pub struct EffectConfig {
    /// Effect type name (e.g., "distortion", "reverb")
    #[serde(rename = "type")]
    pub effect_type: String,
    /// Effect parameters
    #[serde(default)]
    pub params: HashMap<String, String>,
}
