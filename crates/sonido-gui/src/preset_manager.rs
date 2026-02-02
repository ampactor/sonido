//! Preset management for saving and loading effect configurations.
//!
//! This module uses sonido_config::Preset for storage (TOML format) while
//! keeping SharedParams for real-time atomic parameter access in the audio thread.

use crate::audio_bridge::SharedParams;
use sonido_config::{
    EffectConfig, Preset,
    paths::{user_presets_dir, ensure_user_presets_dir, list_user_presets},
    factory_presets,
};
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// Convert SharedParams to a sonido_config::Preset.
///
/// Creates a preset that captures the current state of all parameters.
pub fn params_to_preset(name: &str, description: Option<&str>, params: &Arc<SharedParams>) -> Preset {
    let mut preset = Preset::new(name);

    if let Some(desc) = description {
        preset = preset.with_description(desc);
    }

    // Build effect chain from current parameters
    // The GUI has a fixed set of effects, so we capture them all

    // Preamp
    preset = preset.with_effect(
        EffectConfig::new("preamp")
            .with_bypass(params.bypass.preamp.load(Ordering::Relaxed))
            .with_param("gain", format!("{}", params.preamp_gain.get()))
    );

    // Distortion
    preset = preset.with_effect(
        EffectConfig::new("distortion")
            .with_bypass(params.bypass.distortion.load(Ordering::Relaxed))
            .with_param("drive", format!("{}", params.dist_drive.get()))
            .with_param("tone", format!("{}", params.dist_tone.get()))
            .with_param("level", format!("{}", params.dist_level.get()))
            .with_param("waveshape", format!("{}", params.dist_waveshape.load(Ordering::Relaxed)))
    );

    // Compressor
    preset = preset.with_effect(
        EffectConfig::new("compressor")
            .with_bypass(params.bypass.compressor.load(Ordering::Relaxed))
            .with_param("threshold", format!("{}", params.comp_threshold.get()))
            .with_param("ratio", format!("{}", params.comp_ratio.get()))
            .with_param("attack", format!("{}", params.comp_attack.get()))
            .with_param("release", format!("{}", params.comp_release.get()))
            .with_param("makeup", format!("{}", params.comp_makeup.get()))
    );

    // Chorus
    preset = preset.with_effect(
        EffectConfig::new("chorus")
            .with_bypass(params.bypass.chorus.load(Ordering::Relaxed))
            .with_param("rate", format!("{}", params.chorus_rate.get()))
            .with_param("depth", format!("{}", params.chorus_depth.get()))
            .with_param("mix", format!("{}", params.chorus_mix.get()))
    );

    // Delay
    preset = preset.with_effect(
        EffectConfig::new("delay")
            .with_bypass(params.bypass.delay.load(Ordering::Relaxed))
            .with_param("time", format!("{}", params.delay_time.get()))
            .with_param("feedback", format!("{}", params.delay_feedback.get()))
            .with_param("mix", format!("{}", params.delay_mix.get()))
    );

    // Filter
    preset = preset.with_effect(
        EffectConfig::new("filter")
            .with_bypass(params.bypass.filter.load(Ordering::Relaxed))
            .with_param("cutoff", format!("{}", params.filter_cutoff.get()))
            .with_param("resonance", format!("{}", params.filter_resonance.get()))
    );

    // MultiVibrato
    preset = preset.with_effect(
        EffectConfig::new("multivibrato")
            .with_bypass(params.bypass.multivibrato.load(Ordering::Relaxed))
            .with_param("intensity", format!("{}", params.vibrato_depth.get()))
    );

    // Tape Saturation
    preset = preset.with_effect(
        EffectConfig::new("tape")
            .with_bypass(params.bypass.tape.load(Ordering::Relaxed))
            .with_param("drive", format!("{}", params.tape_drive.get()))
            .with_param("warmth", format!("{}", params.tape_saturation.get()))
    );

    // Reverb
    preset = preset.with_effect(
        EffectConfig::new("reverb")
            .with_bypass(params.bypass.reverb.load(Ordering::Relaxed))
            .with_param("room_size", format!("{}", params.reverb_room_size.get()))
            .with_param("decay", format!("{}", params.reverb_decay.get()))
            .with_param("damping", format!("{}", params.reverb_damping.get()))
            .with_param("predelay", format!("{}", params.reverb_predelay.get()))
            .with_param("mix", format!("{}", params.reverb_mix.get()))
    );

    preset
}

/// Apply a sonido_config::Preset to SharedParams.
///
/// Updates all atomic parameters from the preset configuration.
pub fn preset_to_params(preset: &Preset, params: &Arc<SharedParams>) {
    for effect in &preset.effects {
        match effect.effect_type.as_str() {
            "preamp" => {
                params.bypass.preamp.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("gain") {
                    params.preamp_gain.set(v);
                }
            }
            "distortion" => {
                params.bypass.distortion.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("drive") {
                    params.dist_drive.set(v);
                }
                if let Some(v) = effect.parse_param("tone") {
                    params.dist_tone.set(v);
                }
                if let Some(v) = effect.parse_param("level") {
                    params.dist_level.set(v);
                }
                if let Some(v) = effect.parse_param("waveshape") {
                    params.dist_waveshape.store(v as u32, Ordering::Relaxed);
                }
            }
            "compressor" => {
                params.bypass.compressor.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("threshold") {
                    params.comp_threshold.set(v);
                }
                if let Some(v) = effect.parse_param("ratio") {
                    params.comp_ratio.set(v);
                }
                if let Some(v) = effect.parse_param("attack") {
                    params.comp_attack.set(v);
                }
                if let Some(v) = effect.parse_param("release") {
                    params.comp_release.set(v);
                }
                if let Some(v) = effect.parse_param("makeup") {
                    params.comp_makeup.set(v);
                }
            }
            "chorus" => {
                params.bypass.chorus.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("rate") {
                    params.chorus_rate.set(v);
                }
                if let Some(v) = effect.parse_param("depth") {
                    params.chorus_depth.set(v);
                }
                if let Some(v) = effect.parse_param("mix") {
                    params.chorus_mix.set(v);
                }
            }
            "delay" => {
                params.bypass.delay.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("time") {
                    params.delay_time.set(v);
                }
                if let Some(v) = effect.parse_param("feedback") {
                    params.delay_feedback.set(v);
                }
                if let Some(v) = effect.parse_param("mix") {
                    params.delay_mix.set(v);
                }
            }
            "filter" => {
                params.bypass.filter.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("cutoff") {
                    params.filter_cutoff.set(v);
                }
                if let Some(v) = effect.parse_param("resonance") {
                    params.filter_resonance.set(v);
                }
            }
            "multivibrato" => {
                params.bypass.multivibrato.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("intensity") {
                    params.vibrato_depth.set(v);
                }
            }
            "tape" => {
                params.bypass.tape.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("drive") {
                    params.tape_drive.set(v);
                }
                if let Some(v) = effect.parse_param("warmth") {
                    params.tape_saturation.set(v);
                }
            }
            "reverb" => {
                params.bypass.reverb.store(effect.bypassed, Ordering::Relaxed);
                if let Some(v) = effect.parse_param("room_size") {
                    params.reverb_room_size.set(v);
                }
                if let Some(v) = effect.parse_param("decay") {
                    params.reverb_decay.set(v);
                }
                if let Some(v) = effect.parse_param("damping") {
                    params.reverb_damping.set(v);
                }
                if let Some(v) = effect.parse_param("predelay") {
                    params.reverb_predelay.set(v);
                }
                if let Some(v) = effect.parse_param("mix") {
                    params.reverb_mix.set(v);
                }
            }
            _ => {
                // Unknown effect type, skip
                log::warn!("Unknown effect type in preset: {}", effect.effect_type);
            }
        }
    }
}

/// Preset entry for the manager.
#[derive(Debug, Clone)]
pub struct PresetEntry {
    /// The preset data.
    pub preset: Preset,
    /// Source: "factory", "user", or file path.
    pub source: PresetSource,
}

/// Where a preset came from.
#[derive(Debug, Clone, PartialEq)]
pub enum PresetSource {
    /// Built-in factory preset.
    Factory,
    /// User preset loaded from disk.
    User(PathBuf),
    /// Unsaved preset (created but not yet saved).
    Unsaved,
}

impl PresetEntry {
    /// Create a factory preset entry.
    pub fn factory(preset: Preset) -> Self {
        Self {
            preset,
            source: PresetSource::Factory,
        }
    }

    /// Create a user preset entry.
    pub fn user(preset: Preset, path: PathBuf) -> Self {
        Self {
            preset,
            source: PresetSource::User(path),
        }
    }

    /// Create an unsaved preset entry.
    pub fn unsaved(preset: Preset) -> Self {
        Self {
            preset,
            source: PresetSource::Unsaved,
        }
    }

    /// Check if this is a factory preset.
    pub fn is_factory(&self) -> bool {
        matches!(self.source, PresetSource::Factory)
    }

    /// Check if this is a user preset.
    pub fn is_user(&self) -> bool {
        matches!(self.source, PresetSource::User(_))
    }

    /// Get the file path if this is a user preset.
    pub fn path(&self) -> Option<&PathBuf> {
        match &self.source {
            PresetSource::User(p) => Some(p),
            _ => None,
        }
    }
}

/// Manager for loading and saving presets.
///
/// Uses sonido_config::Preset for storage (TOML format) while maintaining
/// compatibility with the GUI's SharedParams for real-time parameter access.
pub struct PresetManager {
    /// All available presets (factory + user).
    presets: Vec<PresetEntry>,
    /// Index of the currently selected preset.
    current_preset: usize,
    /// Whether the current preset has been modified.
    modified: bool,
}

impl PresetManager {
    /// Create a new preset manager.
    ///
    /// Loads factory presets and any user presets from the user presets directory.
    pub fn new() -> Self {
        let mut manager = Self {
            presets: Vec::new(),
            current_preset: 0,
            modified: false,
        };

        manager.load_factory_presets();
        manager.load_user_presets();

        // Ensure we have at least one preset
        if manager.presets.is_empty() {
            let init = Preset::new("Init")
                .with_description("Clean signal path");
            manager.presets.push(PresetEntry::unsaved(init));
        }

        manager
    }

    /// Load factory presets from sonido_config.
    fn load_factory_presets(&mut self) {
        for preset in factory_presets() {
            self.presets.push(PresetEntry::factory(preset));
        }
    }

    /// Load user presets from the user presets directory.
    fn load_user_presets(&mut self) {
        for path in list_user_presets() {
            match Preset::load(&path) {
                Ok(preset) => {
                    self.presets.push(PresetEntry::user(preset, path));
                }
                Err(e) => {
                    log::warn!("Failed to load preset {:?}: {}", path, e);
                }
            }
        }
    }

    /// Get all presets.
    pub fn presets(&self) -> &[PresetEntry] {
        &self.presets
    }

    /// Get the current preset index.
    pub fn current_preset(&self) -> usize {
        self.current_preset
    }

    /// Get the current preset.
    pub fn current(&self) -> Option<&PresetEntry> {
        self.presets.get(self.current_preset)
    }

    /// Select a preset by index and apply it to the parameters.
    pub fn select(&mut self, index: usize, params: &Arc<SharedParams>) {
        if index < self.presets.len() {
            self.current_preset = index;
            preset_to_params(&self.presets[index].preset, params);
            self.modified = false;
        }
    }

    /// Mark the current preset as modified.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Check if the current preset has been modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Save the current parameters as a new preset.
    ///
    /// The preset is saved to the user presets directory as a TOML file.
    pub fn save_as(
        &mut self,
        name: &str,
        description: Option<&str>,
        params: &Arc<SharedParams>,
    ) -> Result<(), String> {
        // Create the preset from current parameters
        let preset = params_to_preset(name, description, params);

        // Ensure the presets directory exists
        ensure_user_presets_dir().map_err(|e| format!("Failed to create presets directory: {}", e))?;

        // Generate filename from name
        let filename = format!("{}.toml", name.to_lowercase().replace(' ', "_"));
        let path = user_presets_dir().join(&filename);

        // Save to file
        preset.save(&path).map_err(|e| format!("Failed to save preset: {}", e))?;

        // Add to our list
        self.presets.push(PresetEntry::user(preset, path));
        self.current_preset = self.presets.len() - 1;
        self.modified = false;

        Ok(())
    }

    /// Overwrite the current user preset with updated parameters.
    ///
    /// Only works for user presets, not factory presets.
    pub fn save_current(&mut self, params: &Arc<SharedParams>) -> Result<(), String> {
        let entry = self.presets.get(self.current_preset)
            .ok_or_else(|| "No preset selected".to_string())?;

        let path = match &entry.source {
            PresetSource::User(p) => p.clone(),
            PresetSource::Factory => {
                return Err("Cannot overwrite factory preset. Use 'Save As' instead.".to_string());
            }
            PresetSource::Unsaved => {
                return Err("Preset has not been saved yet. Use 'Save As'.".to_string());
            }
        };

        // Create updated preset
        let preset = params_to_preset(
            &entry.preset.name,
            entry.preset.description.as_deref(),
            params,
        );

        // Save to file
        preset.save(&path).map_err(|e| format!("Failed to save preset: {}", e))?;

        // Update our entry
        self.presets[self.current_preset] = PresetEntry::user(preset, path);
        self.modified = false;

        Ok(())
    }

    /// Delete a user preset.
    ///
    /// Only works for user presets, not factory presets.
    pub fn delete(&mut self, index: usize) -> Result<(), String> {
        if index >= self.presets.len() {
            return Err("Invalid preset index".to_string());
        }

        let entry = &self.presets[index];
        let path = match &entry.source {
            PresetSource::User(p) => p.clone(),
            PresetSource::Factory => {
                return Err("Cannot delete factory preset".to_string());
            }
            PresetSource::Unsaved => {
                // Just remove from list
                self.presets.remove(index);
                if self.current_preset >= self.presets.len() && self.current_preset > 0 {
                    self.current_preset -= 1;
                }
                return Ok(());
            }
        };

        // Delete the file
        std::fs::remove_file(&path).map_err(|e| format!("Failed to delete preset file: {}", e))?;

        // Remove from list
        self.presets.remove(index);
        if self.current_preset >= self.presets.len() && self.current_preset > 0 {
            self.current_preset -= 1;
        }

        Ok(())
    }

    /// Reload all presets from disk.
    pub fn reload(&mut self) {
        let current_name = self.current()
            .map(|e| e.preset.name.clone());

        self.presets.clear();
        self.load_factory_presets();
        self.load_user_presets();

        // Try to restore selection by name
        if let Some(name) = current_name {
            if let Some(idx) = self.presets.iter().position(|e| e.preset.name == name) {
                self.current_preset = idx;
            } else {
                self.current_preset = 0;
            }
        } else {
            self.current_preset = 0;
        }

        self.modified = false;
    }

    /// Get the user presets directory path.
    pub fn presets_dir() -> PathBuf {
        user_presets_dir()
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preset_manager_new() {
        let manager = PresetManager::new();
        // Should have factory presets loaded
        assert!(!manager.presets.is_empty());
        // First preset should be factory
        assert!(manager.presets[0].is_factory());
    }

    #[test]
    fn test_params_to_preset_roundtrip() {
        let params = Arc::new(SharedParams::default());

        // Set some test values
        params.dist_drive.set(20.0);
        params.reverb_mix.set(0.7);
        params.bypass.chorus.store(true, Ordering::Relaxed);

        // Convert to preset
        let preset = params_to_preset("Test", Some("Test preset"), &params);

        // Create fresh params and apply preset
        let params2 = Arc::new(SharedParams::default());
        preset_to_params(&preset, &params2);

        // Verify values match
        assert!((params2.dist_drive.get() - 20.0).abs() < 0.01);
        assert!((params2.reverb_mix.get() - 0.7).abs() < 0.01);
        assert!(params2.bypass.chorus.load(Ordering::Relaxed));
    }

    #[test]
    fn test_preset_entry_sources() {
        let preset = Preset::new("Test");

        let factory = PresetEntry::factory(preset.clone());
        assert!(factory.is_factory());
        assert!(!factory.is_user());
        assert!(factory.path().is_none());

        let user = PresetEntry::user(preset.clone(), PathBuf::from("/test.toml"));
        assert!(!user.is_factory());
        assert!(user.is_user());
        assert!(user.path().is_some());

        let unsaved = PresetEntry::unsaved(preset);
        assert!(!unsaved.is_factory());
        assert!(!unsaved.is_user());
    }
}
