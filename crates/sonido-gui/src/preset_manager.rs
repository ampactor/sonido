//! Preset management for saving and loading effect configurations.

use crate::audio_bridge::SharedParams;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// A complete preset containing all effect parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    pub category: String,

    // Global
    pub input_gain: f32,
    pub master_volume: f32,

    // Effect order (indices)
    pub effect_order: Vec<usize>,

    // Bypass states
    pub preamp_bypass: bool,
    pub distortion_bypass: bool,
    pub compressor_bypass: bool,
    pub chorus_bypass: bool,
    pub delay_bypass: bool,
    pub filter_bypass: bool,
    pub multivibrato_bypass: bool,
    pub tape_bypass: bool,
    pub reverb_bypass: bool,

    // Preamp
    pub preamp_gain: f32,

    // Distortion
    pub dist_drive: f32,
    pub dist_tone: f32,
    pub dist_level: f32,
    pub dist_waveshape: u32,

    // Compressor
    pub comp_threshold: f32,
    pub comp_ratio: f32,
    pub comp_attack: f32,
    pub comp_release: f32,
    pub comp_makeup: f32,

    // Chorus
    pub chorus_rate: f32,
    pub chorus_depth: f32,
    pub chorus_mix: f32,

    // Delay
    pub delay_time: f32,
    pub delay_feedback: f32,
    pub delay_mix: f32,

    // Filter
    pub filter_cutoff: f32,
    pub filter_resonance: f32,

    // MultiVibrato
    pub vibrato_depth: f32,

    // Tape
    pub tape_drive: f32,
    pub tape_saturation: f32,

    // Reverb
    pub reverb_room_size: f32,
    pub reverb_decay: f32,
    pub reverb_damping: f32,
    pub reverb_predelay: f32,
    pub reverb_mix: f32,
    pub reverb_type: u32,
}

impl Default for Preset {
    fn default() -> Self {
        Self {
            name: "Init".to_string(),
            category: "Default".to_string(),
            effect_order: vec![0, 1, 2, 3, 4, 5, 6, 7, 8],
            input_gain: 0.0,
            master_volume: 0.0,
            preamp_bypass: false,
            distortion_bypass: true,
            compressor_bypass: true,
            chorus_bypass: true,
            delay_bypass: true,
            filter_bypass: true,
            multivibrato_bypass: true,
            tape_bypass: true,
            reverb_bypass: true,
            preamp_gain: 0.0,
            dist_drive: 15.0,
            dist_tone: 4000.0,
            dist_level: -6.0,
            dist_waveshape: 0,
            comp_threshold: -20.0,
            comp_ratio: 4.0,
            comp_attack: 10.0,
            comp_release: 100.0,
            comp_makeup: 0.0,
            chorus_rate: 1.0,
            chorus_depth: 0.5,
            chorus_mix: 0.5,
            delay_time: 300.0,
            delay_feedback: 0.4,
            delay_mix: 0.5,
            filter_cutoff: 5000.0,
            filter_resonance: 0.7,
            vibrato_depth: 0.5,
            tape_drive: 6.0,
            tape_saturation: 0.5,
            reverb_room_size: 0.5,
            reverb_decay: 0.5,
            reverb_damping: 0.5,
            reverb_predelay: 10.0,
            reverb_mix: 0.3,
            reverb_type: 0,
        }
    }
}

impl Preset {
    /// Create preset from current parameters.
    pub fn from_params(name: &str, category: &str, params: &Arc<SharedParams>) -> Self {
        Self {
            name: name.to_string(),
            category: category.to_string(),
            effect_order: vec![0, 1, 2, 3, 4, 5, 6, 7, 8], // TODO: Get from chain view
            input_gain: params.input_gain.get(),
            master_volume: params.master_volume.get(),
            preamp_bypass: params.bypass.preamp.load(Ordering::Relaxed),
            distortion_bypass: params.bypass.distortion.load(Ordering::Relaxed),
            compressor_bypass: params.bypass.compressor.load(Ordering::Relaxed),
            chorus_bypass: params.bypass.chorus.load(Ordering::Relaxed),
            delay_bypass: params.bypass.delay.load(Ordering::Relaxed),
            filter_bypass: params.bypass.filter.load(Ordering::Relaxed),
            multivibrato_bypass: params.bypass.multivibrato.load(Ordering::Relaxed),
            tape_bypass: params.bypass.tape.load(Ordering::Relaxed),
            reverb_bypass: params.bypass.reverb.load(Ordering::Relaxed),
            preamp_gain: params.preamp_gain.get(),
            dist_drive: params.dist_drive.get(),
            dist_tone: params.dist_tone.get(),
            dist_level: params.dist_level.get(),
            dist_waveshape: params.dist_waveshape.load(Ordering::Relaxed),
            comp_threshold: params.comp_threshold.get(),
            comp_ratio: params.comp_ratio.get(),
            comp_attack: params.comp_attack.get(),
            comp_release: params.comp_release.get(),
            comp_makeup: params.comp_makeup.get(),
            chorus_rate: params.chorus_rate.get(),
            chorus_depth: params.chorus_depth.get(),
            chorus_mix: params.chorus_mix.get(),
            delay_time: params.delay_time.get(),
            delay_feedback: params.delay_feedback.get(),
            delay_mix: params.delay_mix.get(),
            filter_cutoff: params.filter_cutoff.get(),
            filter_resonance: params.filter_resonance.get(),
            vibrato_depth: params.vibrato_depth.get(),
            tape_drive: params.tape_drive.get(),
            tape_saturation: params.tape_saturation.get(),
            reverb_room_size: params.reverb_room_size.get(),
            reverb_decay: params.reverb_decay.get(),
            reverb_damping: params.reverb_damping.get(),
            reverb_predelay: params.reverb_predelay.get(),
            reverb_mix: params.reverb_mix.get(),
            reverb_type: params.reverb_type.load(Ordering::Relaxed),
        }
    }

    /// Apply preset to parameters.
    pub fn apply_to_params(&self, params: &Arc<SharedParams>) {
        params.input_gain.set(self.input_gain);
        params.master_volume.set(self.master_volume);
        params.bypass.preamp.store(self.preamp_bypass, Ordering::Relaxed);
        params.bypass.distortion.store(self.distortion_bypass, Ordering::Relaxed);
        params.bypass.compressor.store(self.compressor_bypass, Ordering::Relaxed);
        params.bypass.chorus.store(self.chorus_bypass, Ordering::Relaxed);
        params.bypass.delay.store(self.delay_bypass, Ordering::Relaxed);
        params.bypass.filter.store(self.filter_bypass, Ordering::Relaxed);
        params.bypass.multivibrato.store(self.multivibrato_bypass, Ordering::Relaxed);
        params.bypass.tape.store(self.tape_bypass, Ordering::Relaxed);
        params.bypass.reverb.store(self.reverb_bypass, Ordering::Relaxed);
        params.preamp_gain.set(self.preamp_gain);
        params.dist_drive.set(self.dist_drive);
        params.dist_tone.set(self.dist_tone);
        params.dist_level.set(self.dist_level);
        params.dist_waveshape.store(self.dist_waveshape, Ordering::Relaxed);
        params.comp_threshold.set(self.comp_threshold);
        params.comp_ratio.set(self.comp_ratio);
        params.comp_attack.set(self.comp_attack);
        params.comp_release.set(self.comp_release);
        params.comp_makeup.set(self.comp_makeup);
        params.chorus_rate.set(self.chorus_rate);
        params.chorus_depth.set(self.chorus_depth);
        params.chorus_mix.set(self.chorus_mix);
        params.delay_time.set(self.delay_time);
        params.delay_feedback.set(self.delay_feedback);
        params.delay_mix.set(self.delay_mix);
        params.filter_cutoff.set(self.filter_cutoff);
        params.filter_resonance.set(self.filter_resonance);
        params.vibrato_depth.set(self.vibrato_depth);
        params.tape_drive.set(self.tape_drive);
        params.tape_saturation.set(self.tape_saturation);
        params.reverb_room_size.set(self.reverb_room_size);
        params.reverb_decay.set(self.reverb_decay);
        params.reverb_damping.set(self.reverb_damping);
        params.reverb_predelay.set(self.reverb_predelay);
        params.reverb_mix.set(self.reverb_mix);
        params.reverb_type.store(self.reverb_type, Ordering::Relaxed);
    }
}

/// Manager for loading and saving presets.
pub struct PresetManager {
    presets: Vec<Preset>,
    current_preset: usize,
    presets_dir: PathBuf,
    modified: bool,
}

impl PresetManager {
    /// Create a new preset manager.
    pub fn new() -> Self {
        let presets_dir = Self::get_presets_dir();
        let _ = fs::create_dir_all(&presets_dir);

        let mut manager = Self {
            presets: Vec::new(),
            current_preset: 0,
            presets_dir,
            modified: false,
        };

        manager.load_factory_presets();
        manager.load_user_presets();

        if manager.presets.is_empty() {
            manager.presets.push(Preset::default());
        }

        manager
    }

    /// Get presets directory path.
    fn get_presets_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sonido")
            .join("presets")
    }

    /// Load factory presets.
    fn load_factory_presets(&mut self) {
        // Clean preset
        self.presets.push(Preset::default());

        // Crunch preset
        let mut crunch = Preset::default();
        crunch.name = "Crunch".to_string();
        crunch.category = "Factory".to_string();
        crunch.distortion_bypass = false;
        crunch.dist_drive = 12.0;
        crunch.dist_tone = 5000.0;
        crunch.dist_level = -3.0;
        self.presets.push(crunch);

        // High Gain preset
        let mut high_gain = Preset::default();
        high_gain.name = "High Gain".to_string();
        high_gain.category = "Factory".to_string();
        high_gain.distortion_bypass = false;
        high_gain.compressor_bypass = false;
        high_gain.dist_drive = 30.0;
        high_gain.dist_tone = 4500.0;
        high_gain.dist_level = -6.0;
        high_gain.comp_threshold = -15.0;
        high_gain.comp_ratio = 6.0;
        self.presets.push(high_gain);

        // Ambient preset
        let mut ambient = Preset::default();
        ambient.name = "Ambient".to_string();
        ambient.category = "Factory".to_string();
        ambient.delay_bypass = false;
        ambient.reverb_bypass = false;
        ambient.chorus_bypass = false;
        ambient.delay_time = 500.0;
        ambient.delay_feedback = 0.5;
        ambient.delay_mix = 0.4;
        ambient.reverb_room_size = 0.8;
        ambient.reverb_decay = 0.7;
        ambient.reverb_mix = 0.4;
        ambient.chorus_depth = 0.3;
        ambient.chorus_rate = 0.5;
        ambient.chorus_mix = 0.3;
        self.presets.push(ambient);

        // Tape Warmth preset
        let mut tape_warmth = Preset::default();
        tape_warmth.name = "Tape Warmth".to_string();
        tape_warmth.category = "Factory".to_string();
        tape_warmth.tape_bypass = false;
        tape_warmth.tape_drive = 12.0;
        tape_warmth.tape_saturation = 0.6;
        self.presets.push(tape_warmth);
    }

    /// Load user presets from disk.
    fn load_user_presets(&mut self) {
        if let Ok(entries) = fs::read_dir(&self.presets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "json") {
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(preset) = serde_json::from_str::<Preset>(&content) {
                            self.presets.push(preset);
                        }
                    }
                }
            }
        }
    }

    /// Get all presets.
    pub fn presets(&self) -> &[Preset] {
        &self.presets
    }

    /// Get current preset index.
    pub fn current_preset(&self) -> usize {
        self.current_preset
    }

    /// Get current preset.
    pub fn current(&self) -> Option<&Preset> {
        self.presets.get(self.current_preset)
    }

    /// Select a preset by index.
    pub fn select(&mut self, index: usize, params: &Arc<SharedParams>) {
        if index < self.presets.len() {
            self.current_preset = index;
            self.presets[index].apply_to_params(params);
            self.modified = false;
        }
    }

    /// Mark current preset as modified.
    pub fn mark_modified(&mut self) {
        self.modified = true;
    }

    /// Check if current preset is modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Save current settings as a new preset.
    pub fn save_as(&mut self, name: &str, category: &str, params: &Arc<SharedParams>) -> Result<(), String> {
        let preset = Preset::from_params(name, category, params);

        // Save to file
        let filename = format!("{}.json", name.replace(' ', "_").to_lowercase());
        let path = self.presets_dir.join(&filename);

        let json = serde_json::to_string_pretty(&preset)
            .map_err(|e| format!("Failed to serialize preset: {}", e))?;

        fs::write(&path, json).map_err(|e| format!("Failed to write preset file: {}", e))?;

        // Add to list
        self.presets.push(preset);
        self.current_preset = self.presets.len() - 1;
        self.modified = false;

        Ok(())
    }

    /// Save current preset (overwrite).
    pub fn save_current(&mut self, params: &Arc<SharedParams>) -> Result<(), String> {
        let preset = self.presets.get(self.current_preset).ok_or("No preset selected")?;
        let name = preset.name.clone();
        let category = preset.category.clone();

        let updated = Preset::from_params(&name, &category, params);

        // Save to file
        let filename = format!("{}.json", name.replace(' ', "_").to_lowercase());
        let path = self.presets_dir.join(&filename);

        let json = serde_json::to_string_pretty(&updated)
            .map_err(|e| format!("Failed to serialize preset: {}", e))?;

        fs::write(&path, json).map_err(|e| format!("Failed to write preset file: {}", e))?;

        self.presets[self.current_preset] = updated;
        self.modified = false;

        Ok(())
    }
}

impl Default for PresetManager {
    fn default() -> Self {
        Self::new()
    }
}
