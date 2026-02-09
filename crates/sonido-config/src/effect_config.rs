//! Effect configuration types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for a single effect in a preset.
///
/// Each effect has a type identifier and optional parameters. Effects can be
/// bypassed by prefixing the type with `!` (e.g., `!reverb`).
///
/// # Example
///
/// ```rust
/// use sonido_config::EffectConfig;
///
/// let config = EffectConfig::new("distortion")
///     .with_param("drive", "0.7")
///     .with_param("tone", "0.5");
///
/// assert_eq!(config.effect_type, "distortion");
/// assert!(!config.bypassed);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EffectConfig {
    /// Effect type name (e.g., "distortion", "reverb").
    /// Use `!` prefix to bypass (e.g., "!reverb").
    #[serde(rename = "type")]
    pub effect_type: String,

    /// Whether the effect is bypassed.
    #[serde(default)]
    pub bypassed: bool,

    /// Effect parameters as key-value pairs.
    /// Values are strings to support various formats (numbers, percentages, etc.)
    #[serde(default)]
    pub params: HashMap<String, String>,
}

impl EffectConfig {
    /// Create a new effect configuration.
    ///
    /// If the type starts with `!`, the effect will be marked as bypassed.
    pub fn new(effect_type: impl Into<String>) -> Self {
        let type_str = effect_type.into();
        let (effect_type, bypassed) = if let Some(stripped) = type_str.strip_prefix('!') {
            (stripped.to_string(), true)
        } else {
            (type_str, false)
        };

        Self {
            effect_type,
            bypassed,
            params: HashMap::new(),
        }
    }

    /// Create a new bypassed effect configuration.
    pub fn new_bypassed(effect_type: impl Into<String>) -> Self {
        Self {
            effect_type: effect_type.into(),
            bypassed: true,
            params: HashMap::new(),
        }
    }

    /// Add a parameter to the configuration.
    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }

    /// Set whether the effect is bypassed.
    pub fn with_bypass(mut self, bypassed: bool) -> Self {
        self.bypassed = bypassed;
        self
    }

    /// Get a parameter value.
    pub fn get_param(&self, key: &str) -> Option<&str> {
        self.params.get(key).map(|s| s.as_str())
    }

    /// Set a parameter value.
    pub fn set_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.params.insert(key.into(), value.into());
    }

    /// Parse a parameter value as f32.
    ///
    /// Supports:
    /// - Plain numbers: "0.5", "1.2"
    /// - Percentages: "50%", "120%" (converted to 0-1 range)
    /// - Decibels: "-6dB", "+3dB" (converted to linear gain)
    /// - Time: "100ms", "1.5s" (converted to seconds)
    /// - Frequency: "440Hz", "1.2kHz" (converted to Hz)
    pub fn parse_param(&self, key: &str) -> Option<f32> {
        let value = self.params.get(key)?;
        parse_param_value(value)
    }

    /// Get the canonical effect type (without bypass prefix).
    pub fn canonical_type(&self) -> &str {
        &self.effect_type
    }

    /// Get the effect type string for display (with ! prefix if bypassed).
    pub fn display_type(&self) -> String {
        if self.bypassed {
            format!("!{}", self.effect_type)
        } else {
            self.effect_type.clone()
        }
    }
}

/// Parse a parameter value string into an f32.
///
/// Supports various formats:
/// - Plain numbers: "0.5", "1.2", "-0.3"
/// - Percentages: "50%", "120%" (divided by 100)
/// - Decibels: "-6dB", "+3dB" (converted to linear gain)
/// - Time in ms: "100ms" (converted to seconds)
/// - Time in s: "1.5s" (kept as seconds)
/// - Frequency in Hz: "440Hz"
/// - Frequency in kHz: "1.2kHz" (converted to Hz)
pub fn parse_param_value(value: &str) -> Option<f32> {
    let value = value.trim();

    // Percentages
    if let Some(pct) = value.strip_suffix('%') {
        return pct.trim().parse::<f32>().ok().map(|v| v / 100.0);
    }

    // Decibels
    if let Some(db) = value
        .strip_suffix("dB")
        .or_else(|| value.strip_suffix("db"))
    {
        return db.trim().parse::<f32>().ok().map(|v| {
            // Convert dB to linear gain: 10^(dB/20)
            libm::powf(10.0, v / 20.0)
        });
    }

    // Milliseconds
    if let Some(ms) = value.strip_suffix("ms") {
        return ms.trim().parse::<f32>().ok().map(|v| v / 1000.0);
    }

    // Seconds
    if let Some(s) = value.strip_suffix('s') {
        // Make sure it's not "ms"
        if !value.ends_with("ms") {
            return s.trim().parse::<f32>().ok();
        }
    }

    // Kilohertz
    if let Some(khz) = value
        .strip_suffix("kHz")
        .or_else(|| value.strip_suffix("khz"))
    {
        return khz.trim().parse::<f32>().ok().map(|v| v * 1000.0);
    }

    // Hertz
    if let Some(hz) = value
        .strip_suffix("Hz")
        .or_else(|| value.strip_suffix("hz"))
    {
        return hz.trim().parse::<f32>().ok();
    }

    // Plain number
    value.parse::<f32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_effect_config_new() {
        let config = EffectConfig::new("distortion");
        assert_eq!(config.effect_type, "distortion");
        assert!(!config.bypassed);
        assert!(config.params.is_empty());
    }

    #[test]
    fn test_effect_config_bypassed_prefix() {
        let config = EffectConfig::new("!reverb");
        assert_eq!(config.effect_type, "reverb");
        assert!(config.bypassed);
    }

    #[test]
    fn test_effect_config_with_params() {
        let config = EffectConfig::new("distortion")
            .with_param("drive", "0.7")
            .with_param("tone", "0.5");

        assert_eq!(config.get_param("drive"), Some("0.7"));
        assert_eq!(config.get_param("tone"), Some("0.5"));
        assert_eq!(config.get_param("missing"), None);
    }

    #[test]
    fn test_effect_config_display_type() {
        let normal = EffectConfig::new("distortion");
        assert_eq!(normal.display_type(), "distortion");

        let bypassed = EffectConfig::new("!reverb");
        assert_eq!(bypassed.display_type(), "!reverb");
    }

    #[test]
    fn test_parse_plain_numbers() {
        assert_eq!(parse_param_value("0.5"), Some(0.5));
        assert_eq!(parse_param_value("1.2"), Some(1.2));
        assert_eq!(parse_param_value("-0.3"), Some(-0.3));
        assert_eq!(parse_param_value("  0.5  "), Some(0.5));
    }

    #[test]
    fn test_parse_percentages() {
        assert_eq!(parse_param_value("50%"), Some(0.5));
        assert_eq!(parse_param_value("100%"), Some(1.0));
        assert_eq!(parse_param_value("120%"), Some(1.2));
        assert_eq!(parse_param_value("0%"), Some(0.0));
    }

    #[test]
    fn test_parse_decibels() {
        // 0 dB = 1.0 linear
        let val = parse_param_value("0dB").unwrap();
        assert!((val - 1.0).abs() < 0.001);

        // -6 dB ≈ 0.5 linear
        let val = parse_param_value("-6dB").unwrap();
        assert!((val - 0.5).abs() < 0.05);

        // +6 dB ≈ 2.0 linear
        let val = parse_param_value("+6dB").unwrap();
        assert!((val - 2.0).abs() < 0.1);

        // Case insensitive
        let val = parse_param_value("-6db").unwrap();
        assert!((val - 0.5).abs() < 0.05);
    }

    #[test]
    fn test_parse_time() {
        assert_eq!(parse_param_value("100ms"), Some(0.1));
        assert_eq!(parse_param_value("1000ms"), Some(1.0));
        assert_eq!(parse_param_value("1.5s"), Some(1.5));
        assert_eq!(parse_param_value("0.5s"), Some(0.5));
    }

    #[test]
    fn test_parse_frequency() {
        assert_eq!(parse_param_value("440Hz"), Some(440.0));
        assert_eq!(parse_param_value("440hz"), Some(440.0));
        assert_eq!(parse_param_value("1kHz"), Some(1000.0));
        assert_eq!(parse_param_value("1.5kHz"), Some(1500.0));
        assert_eq!(parse_param_value("2.2khz"), Some(2200.0));
    }

    #[test]
    fn test_parse_invalid() {
        assert_eq!(parse_param_value("invalid"), None);
        assert_eq!(parse_param_value("abc%"), None);
    }

    #[test]
    fn test_config_parse_param() {
        let config = EffectConfig::new("test")
            .with_param("gain", "-6dB")
            .with_param("time", "100ms");

        let gain = config.parse_param("gain").unwrap();
        assert!((gain - 0.5).abs() < 0.05);

        let time = config.parse_param("time").unwrap();
        assert!((time - 0.1).abs() < 0.001);

        assert!(config.parse_param("missing").is_none());
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = EffectConfig::new("distortion")
            .with_param("drive", "0.7")
            .with_bypass(true);

        let toml_str = toml::to_string(&config).unwrap();
        let parsed: EffectConfig = toml::from_str(&toml_str).unwrap();

        assert_eq!(parsed.effect_type, "distortion");
        assert!(parsed.bypassed);
        assert_eq!(parsed.get_param("drive"), Some("0.7"));
    }
}
