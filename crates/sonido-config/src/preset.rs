//! Preset file format and operations.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::effect_config::EffectConfig;
use crate::error::ConfigError;

/// Preset file format for effect chains.
///
/// Presets are stored as TOML files containing a list of effects with their
/// parameters. They can be loaded from files, created programmatically, and
/// saved to disk.
///
/// # TOML Format
///
/// ```toml
/// name = "My Preset"
/// description = "A warm, vintage tone"
/// sample_rate = 48000
///
/// [[effects]]
/// type = "distortion"
/// bypassed = false
/// [effects.params]
/// drive = "0.6"
/// tone = "0.5"
///
/// [[effects]]
/// type = "reverb"
/// [effects.params]
/// room_size = "0.8"
/// damping = "0.3"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    /// Name of the preset.
    pub name: String,

    /// Optional description of the preset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Sample rate hint (defaults to 48000).
    /// This is used when creating effects but may be overridden at runtime.
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,

    /// List of effects in the chain.
    #[serde(default)]
    pub effects: Vec<EffectConfig>,
}

fn default_sample_rate() -> u32 {
    48000
}

impl Preset {
    /// Create a new empty preset.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            sample_rate: 48000,
            effects: Vec::new(),
        }
    }

    /// Create a preset with a description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the sample rate hint.
    pub fn with_sample_rate(mut self, sample_rate: u32) -> Self {
        self.sample_rate = sample_rate;
        self
    }

    /// Add an effect to the preset.
    pub fn with_effect(mut self, effect: EffectConfig) -> Self {
        self.effects.push(effect);
        self
    }

    /// Add multiple effects to the preset.
    pub fn with_effects(mut self, effects: impl IntoIterator<Item = EffectConfig>) -> Self {
        self.effects.extend(effects);
        self
    }

    /// Load a preset from a TOML file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)
            .map_err(|e| ConfigError::read_file(path, e))?;
        let preset: Preset = toml::from_str(&content)?;
        Ok(preset)
    }

    /// Load a preset from a TOML string.
    pub fn from_toml(toml_str: &str) -> Result<Self, ConfigError> {
        Ok(toml::from_str(toml_str)?)
    }

    /// Save the preset to a TOML file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), ConfigError> {
        let path = path.as_ref();

        // Ensure parent directory exists
        if let Some(parent) = path.parent()
            && !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ConfigError::create_dir(parent, e))?;
            }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)
            .map_err(|e| ConfigError::write_file(path, e))?;
        Ok(())
    }

    /// Convert the preset to a TOML string.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        Ok(toml::to_string_pretty(self)?)
    }

    /// Get the number of effects in the preset.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Check if the preset is empty.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Get an effect by index.
    pub fn get(&self, index: usize) -> Option<&EffectConfig> {
        self.effects.get(index)
    }

    /// Get a mutable effect by index.
    pub fn get_mut(&mut self, index: usize) -> Option<&mut EffectConfig> {
        self.effects.get_mut(index)
    }

    /// Iterate over effects.
    pub fn iter(&self) -> impl Iterator<Item = &EffectConfig> {
        self.effects.iter()
    }

    /// Iterate over effects mutably.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut EffectConfig> {
        self.effects.iter_mut()
    }

    /// Get a list of effect types (with ! prefix for bypassed effects).
    pub fn effect_types(&self) -> Vec<String> {
        self.effects.iter().map(|e| e.display_type()).collect()
    }

    /// Get a list of canonical effect types (without bypass prefix).
    pub fn canonical_types(&self) -> Vec<&str> {
        self.effects.iter().map(|e| e.canonical_type()).collect()
    }
}

impl Default for Preset {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_new() {
        let preset = Preset::new("Test Preset");
        assert_eq!(preset.name, "Test Preset");
        assert!(preset.description.is_none());
        assert_eq!(preset.sample_rate, 48000);
        assert!(preset.effects.is_empty());
    }

    #[test]
    fn test_preset_builder() {
        let preset = Preset::new("My Preset")
            .with_description("A test preset")
            .with_sample_rate(44100)
            .with_effect(EffectConfig::new("distortion").with_param("drive", "0.6"))
            .with_effect(EffectConfig::new("reverb"));

        assert_eq!(preset.name, "My Preset");
        assert_eq!(preset.description, Some("A test preset".to_string()));
        assert_eq!(preset.sample_rate, 44100);
        assert_eq!(preset.len(), 2);
    }

    #[test]
    fn test_preset_from_toml() {
        let toml = r#"
name = "Test"
description = "A test preset"
sample_rate = 44100

[[effects]]
type = "distortion"
[effects.params]
drive = "0.7"

[[effects]]
type = "reverb"
bypassed = true
[effects.params]
room_size = "0.8"
"#;

        let preset = Preset::from_toml(toml).unwrap();
        assert_eq!(preset.name, "Test");
        assert_eq!(preset.description, Some("A test preset".to_string()));
        assert_eq!(preset.sample_rate, 44100);
        assert_eq!(preset.len(), 2);

        let dist = &preset.effects[0];
        assert_eq!(dist.effect_type, "distortion");
        assert!(!dist.bypassed);
        assert_eq!(dist.get_param("drive"), Some("0.7"));

        let reverb = &preset.effects[1];
        assert_eq!(reverb.effect_type, "reverb");
        assert!(reverb.bypassed);
        assert_eq!(reverb.get_param("room_size"), Some("0.8"));
    }

    #[test]
    fn test_preset_to_toml() {
        let preset = Preset::new("Test")
            .with_description("Test description")
            .with_effect(EffectConfig::new("distortion").with_param("drive", "0.5"));

        let toml = preset.to_toml().unwrap();

        assert!(toml.contains("name = \"Test\""));
        assert!(toml.contains("description = \"Test description\""));
        assert!(toml.contains("type = \"distortion\""));
        assert!(toml.contains("drive = \"0.5\""));
    }

    #[test]
    fn test_preset_roundtrip() {
        let original = Preset::new("Roundtrip Test")
            .with_description("Testing serialization")
            .with_sample_rate(96000)
            .with_effect(
                EffectConfig::new("distortion")
                    .with_param("drive", "0.7")
                    .with_param("tone", "0.5"),
            )
            .with_effect(
                EffectConfig::new("reverb")
                    .with_bypass(true)
                    .with_param("room_size", "0.8"),
            );

        let toml = original.to_toml().unwrap();
        let parsed = Preset::from_toml(&toml).unwrap();

        assert_eq!(original.name, parsed.name);
        assert_eq!(original.description, parsed.description);
        assert_eq!(original.sample_rate, parsed.sample_rate);
        assert_eq!(original.effects.len(), parsed.effects.len());
    }

    #[test]
    fn test_preset_effect_types() {
        let preset = Preset::new("Test")
            .with_effect(EffectConfig::new("distortion"))
            .with_effect(EffectConfig::new("!reverb"));

        let types = preset.effect_types();
        assert_eq!(types, vec!["distortion", "!reverb"]);

        let canonical = preset.canonical_types();
        assert_eq!(canonical, vec!["distortion", "reverb"]);
    }

    #[test]
    fn test_preset_default() {
        let preset = Preset::default();
        assert_eq!(preset.name, "Untitled");
        assert!(preset.is_empty());
    }

    #[test]
    fn test_preset_iteration() {
        let preset = Preset::new("Test")
            .with_effect(EffectConfig::new("a"))
            .with_effect(EffectConfig::new("b"));

        let types: Vec<_> = preset.iter().map(|e| e.effect_type.as_str()).collect();
        assert_eq!(types, vec!["a", "b"]);
    }

    #[test]
    fn test_minimal_toml() {
        let toml = r#"
name = "Minimal"

[[effects]]
type = "distortion"
"#;

        let preset = Preset::from_toml(toml).unwrap();
        assert_eq!(preset.name, "Minimal");
        assert!(preset.description.is_none());
        assert_eq!(preset.sample_rate, 48000); // default
        assert_eq!(preset.len(), 1);
    }
}
