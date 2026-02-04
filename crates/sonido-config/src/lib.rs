//! Configuration and preset management for sonido audio effects.
//!
//! This crate provides a unified configuration system for the sonido DSP framework,
//! including preset loading/saving, effect chain configuration, and parameter validation.
//!
//! # Features
//!
//! - **Preset System**: Load and save effect presets from TOML files
//! - **Effect Chains**: Build chains of effects with parameter configuration
//! - **Validation**: Validate effect types and parameter ranges
//! - **Paths**: Platform-specific preset and config directories
//! - **Factory Presets**: Built-in presets for common use cases
//!
//! # Example
//!
//! ```rust,no_run
//! use sonido_config::{Preset, EffectConfig, user_presets_dir};
//!
//! // Load a preset from file
//! let preset = Preset::load("my_preset.toml").unwrap();
//!
//! // Create a preset programmatically
//! let preset = Preset {
//!     name: "My Preset".to_string(),
//!     description: Some("A custom effect chain".to_string()),
//!     sample_rate: 48000,
//!     effects: vec![
//!         EffectConfig::new("distortion")
//!             .with_param("drive", "0.6")
//!             .with_param("tone", "0.5"),
//!         EffectConfig::new("reverb")
//!             .with_param("room_size", "0.8")
//!             .with_param("damping", "0.3"),
//!     ],
//! };
//!
//! // Save to user presets directory
//! let path = user_presets_dir().join("my_preset.toml");
//! preset.save(&path).unwrap();
//! ```

mod preset;
mod effect_config;
mod chain;
mod error;

/// Platform-specific paths for presets and configuration.
pub mod paths;

/// Effect and preset validation.
pub mod validation;

/// Factory presets bundled with the library.
pub mod factory_presets;

pub use preset::Preset;
pub use effect_config::{EffectConfig, parse_param_value};
pub use validation::{
    validate_effect, validate_preset, validate_effect_param, validate_effect_config,
    ValidationError, ValidationResult, EffectValidator, ParamValidationInfo,
};
pub use paths::{
    user_presets_dir, user_config_dir, system_presets_dir, find_preset,
    ensure_user_presets_dir, ensure_user_config_dir,
    list_user_presets, list_system_presets, list_all_presets, preset_name_from_path,
};
pub use chain::EffectChain;
pub use error::ConfigError;
pub use factory_presets::{factory_presets, get_factory_preset, factory_preset_names, is_factory_preset, FACTORY_PRESET_NAMES};

/// Re-export commonly used types from sonido-registry
pub use sonido_registry::{EffectRegistry, EffectCategory, EffectDescriptor, EffectWithParams};
