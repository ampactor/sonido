//! Shared CLI helpers used across multiple commands.

use sonido_config::{Preset, find_preset as config_find_preset, get_factory_preset};
use std::path::PathBuf;

/// Parse a `key=value` string for clap's `value_parser`.
pub fn parse_key_val(s: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = s.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid parameter format: '{}' (expected key=value)",
            s
        ));
    }
    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Load a preset by name or path.
///
/// Searches in this order:
/// 1. Factory presets (by name)
/// 2. User presets (by name)
/// 3. System presets (by name)
/// 4. File path (if it's a path to a .toml file)
pub fn load_preset(name: &str) -> anyhow::Result<Preset> {
    // Try factory preset first
    if let Some(preset) = get_factory_preset(name) {
        return Ok(preset);
    }

    // Try to find in user/system directories
    if let Some(path) = config_find_preset(name) {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    // Try as a direct file path
    let path = PathBuf::from(name);
    if path.exists() {
        return Preset::load(&path).map_err(|e| anyhow::anyhow!("{}", e));
    }

    anyhow::bail!(
        "Preset '{}' not found. Use 'sonido presets list' to see available presets.",
        name
    )
}
