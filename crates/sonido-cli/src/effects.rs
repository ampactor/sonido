//! Effect factory and parameter parsing.

use sonido_core::Effect;
use sonido_effects::{
    Chorus, CleanPreamp, Compressor, Delay, Distortion, LowPassFilter, MultiVibrato,
    Reverb, ReverbType, TapeSaturation, WaveShape,
};
use std::collections::HashMap;

/// Error type for effect creation.
#[derive(Debug, thiserror::Error)]
pub enum EffectError {
    #[error("Unknown effect: {0}")]
    UnknownEffect(String),

    #[error("Unknown parameter '{param}' for effect '{effect}'")]
    UnknownParameter { effect: String, param: String },

    #[error("Invalid parameter value for '{param}': {message}")]
    InvalidValue { param: String, message: String },

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Information about an available effect.
#[derive(Debug, Clone)]
pub struct EffectInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: &'static [ParameterInfo],
}

/// Information about an effect parameter.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub default: &'static str,
    pub range: &'static str,
}

/// Get information about all available effects.
pub fn available_effects() -> Vec<EffectInfo> {
    vec![
        EffectInfo {
            name: "distortion",
            description: "Waveshaping distortion with multiple modes",
            parameters: &[
                ParameterInfo {
                    name: "drive",
                    description: "Drive amount in dB",
                    default: "12.0",
                    range: "0-40",
                },
                ParameterInfo {
                    name: "tone",
                    description: "Tone frequency in Hz",
                    default: "4000.0",
                    range: "500-10000",
                },
                ParameterInfo {
                    name: "level",
                    description: "Output level in dB",
                    default: "-6.0",
                    range: "-20-0",
                },
                ParameterInfo {
                    name: "waveshape",
                    description: "Waveshape type",
                    default: "softclip",
                    range: "softclip|hardclip|foldback|asymmetric",
                },
            ],
        },
        EffectInfo {
            name: "compressor",
            description: "Dynamics compressor with soft knee",
            parameters: &[
                ParameterInfo {
                    name: "threshold",
                    description: "Threshold in dB",
                    default: "-18.0",
                    range: "-40-0",
                },
                ParameterInfo {
                    name: "ratio",
                    description: "Compression ratio",
                    default: "4.0",
                    range: "1-20",
                },
                ParameterInfo {
                    name: "attack",
                    description: "Attack time in ms",
                    default: "10.0",
                    range: "0.1-100",
                },
                ParameterInfo {
                    name: "release",
                    description: "Release time in ms",
                    default: "100.0",
                    range: "10-1000",
                },
                ParameterInfo {
                    name: "makeup",
                    description: "Makeup gain in dB",
                    default: "0.0",
                    range: "0-20",
                },
            ],
        },
        EffectInfo {
            name: "chorus",
            description: "Dual-voice modulated delay chorus",
            parameters: &[
                ParameterInfo {
                    name: "rate",
                    description: "LFO rate in Hz",
                    default: "1.0",
                    range: "0.1-10",
                },
                ParameterInfo {
                    name: "depth",
                    description: "Modulation depth (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
                ParameterInfo {
                    name: "mix",
                    description: "Wet/dry mix (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
            ],
        },
        EffectInfo {
            name: "delay",
            description: "Tape-style feedback delay",
            parameters: &[
                ParameterInfo {
                    name: "time",
                    description: "Delay time in ms",
                    default: "300.0",
                    range: "1-2000",
                },
                ParameterInfo {
                    name: "feedback",
                    description: "Feedback amount (0-1)",
                    default: "0.4",
                    range: "0-0.95",
                },
                ParameterInfo {
                    name: "mix",
                    description: "Wet/dry mix (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
            ],
        },
        EffectInfo {
            name: "filter",
            description: "Resonant lowpass filter",
            parameters: &[
                ParameterInfo {
                    name: "cutoff",
                    description: "Cutoff frequency in Hz",
                    default: "1000.0",
                    range: "20-20000",
                },
                ParameterInfo {
                    name: "resonance",
                    description: "Resonance (Q)",
                    default: "0.707",
                    range: "0.1-10",
                },
            ],
        },
        EffectInfo {
            name: "multivibrato",
            description: "10-unit tape wow/flutter vibrato",
            parameters: &[
                ParameterInfo {
                    name: "depth",
                    description: "Overall depth (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
            ],
        },
        EffectInfo {
            name: "tape",
            description: "Tape saturation with HF rolloff",
            parameters: &[
                ParameterInfo {
                    name: "drive",
                    description: "Drive amount in dB",
                    default: "6.0",
                    range: "0-24",
                },
                ParameterInfo {
                    name: "saturation",
                    description: "Saturation amount (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
            ],
        },
        EffectInfo {
            name: "preamp",
            description: "Clean preamp/gain stage",
            parameters: &[
                ParameterInfo {
                    name: "gain",
                    description: "Gain in dB",
                    default: "0.0",
                    range: "-20-20",
                },
            ],
        },
        EffectInfo {
            name: "reverb",
            description: "Freeverb-style algorithmic reverb",
            parameters: &[
                ParameterInfo {
                    name: "room_size",
                    description: "Room size (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
                ParameterInfo {
                    name: "decay",
                    description: "Decay time (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
                ParameterInfo {
                    name: "damping",
                    description: "HF damping (0-1, 0=bright, 1=dark)",
                    default: "0.5",
                    range: "0-1",
                },
                ParameterInfo {
                    name: "predelay",
                    description: "Pre-delay in ms",
                    default: "10.0",
                    range: "0-100",
                },
                ParameterInfo {
                    name: "mix",
                    description: "Wet/dry mix (0-1)",
                    default: "0.5",
                    range: "0-1",
                },
                ParameterInfo {
                    name: "type",
                    description: "Reverb type preset",
                    default: "room",
                    range: "room|hall",
                },
            ],
        },
    ]
}

/// Create an effect with custom parameters.
pub fn create_effect_with_params(
    name: &str,
    sample_rate: f32,
    params: &HashMap<String, String>,
) -> Result<Box<dyn Effect + Send>, EffectError> {
    match name.to_lowercase().as_str() {
        "distortion" => {
            let mut effect = Distortion::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "drive" => effect.set_drive_db(parse_f32(key, value)?),
                    "tone" => effect.set_tone_hz(parse_f32(key, value)?),
                    "level" => effect.set_level_db(parse_f32(key, value)?),
                    "waveshape" | "shape" => effect.set_waveshape(parse_waveshape(value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "compressor" => {
            let mut effect = Compressor::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "threshold" => effect.set_threshold_db(parse_f32(key, value)?),
                    "ratio" => effect.set_ratio(parse_f32(key, value)?),
                    "attack" => effect.set_attack_ms(parse_f32(key, value)?),
                    "release" => effect.set_release_ms(parse_f32(key, value)?),
                    "makeup" => effect.set_makeup_gain_db(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "chorus" => {
            let mut effect = Chorus::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "rate" => effect.set_rate(parse_f32(key, value)?),
                    "depth" => effect.set_depth(parse_f32(key, value)?),
                    "mix" => effect.set_mix(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "delay" => {
            let mut effect = Delay::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "time" => effect.set_delay_time_ms(parse_f32(key, value)?),
                    "feedback" => effect.set_feedback(parse_f32(key, value)?),
                    "mix" => effect.set_mix(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "filter" | "lowpass" => {
            let mut effect = LowPassFilter::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "cutoff" => effect.set_cutoff_hz(parse_f32(key, value)?),
                    "resonance" | "q" => effect.set_q(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "multivibrato" | "vibrato" => {
            let mut effect = MultiVibrato::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "depth" => effect.set_depth(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "tape" | "tapesaturation" => {
            let mut effect = TapeSaturation::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "drive" => effect.set_drive(parse_f32(key, value)?),
                    "saturation" => effect.set_saturation(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "preamp" | "cleanpreamp" => {
            let mut effect = CleanPreamp::new();
            for (key, value) in params {
                match key.as_str() {
                    "gain" => effect.set_gain_db(parse_f32(key, value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        "reverb" => {
            let mut effect = Reverb::new(sample_rate);
            for (key, value) in params {
                match key.as_str() {
                    "room_size" | "room" | "size" => effect.set_room_size(parse_f32(key, value)?),
                    "decay" => effect.set_decay(parse_f32(key, value)?),
                    "damping" | "damp" => effect.set_damping(parse_f32(key, value)?),
                    "predelay" | "pre" => effect.set_predelay_ms(parse_f32(key, value)?),
                    "mix" => effect.set_mix(parse_f32(key, value)?),
                    "type" | "preset" => effect.set_reverb_type(parse_reverb_type(value)?),
                    _ => {
                        return Err(EffectError::UnknownParameter {
                            effect: name.to_string(),
                            param: key.to_string(),
                        })
                    }
                }
            }
            Ok(Box::new(effect))
        }
        _ => Err(EffectError::UnknownEffect(name.to_string())),
    }
}

/// Parse an effect chain specification.
///
/// Format: "effect1:param1=value1,param2=value2|effect2:param=value"
///
/// Examples:
/// - "distortion:drive=15"
/// - "preamp:gain=6|distortion:drive=12|delay:time=300,feedback=0.4"
pub fn parse_chain(spec: &str, sample_rate: f32) -> Result<Vec<Box<dyn Effect + Send>>, EffectError> {
    let mut effects = Vec::new();

    for effect_spec in spec.split('|') {
        let effect_spec = effect_spec.trim();
        if effect_spec.is_empty() {
            continue;
        }

        let (name, params) = parse_effect_spec(effect_spec)?;
        let effect = create_effect_with_params(&name, sample_rate, &params)?;
        effects.push(effect);
    }

    Ok(effects)
}

/// Parse a single effect specification.
///
/// Format: "effect_name:param1=value1,param2=value2"
fn parse_effect_spec(spec: &str) -> Result<(String, HashMap<String, String>), EffectError> {
    let parts: Vec<&str> = spec.splitn(2, ':').collect();
    let name = parts[0].trim().to_string();

    let params = if parts.len() > 1 {
        parse_params(parts[1])?
    } else {
        HashMap::new()
    };

    Ok((name, params))
}

/// Parse parameter string into a map.
fn parse_params(params_str: &str) -> Result<HashMap<String, String>, EffectError> {
    let mut params = HashMap::new();

    for param in params_str.split(',') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }

        let kv: Vec<&str> = param.splitn(2, '=').collect();
        if kv.len() != 2 {
            return Err(EffectError::ParseError(format!(
                "Invalid parameter format: '{}' (expected key=value)",
                param
            )));
        }

        params.insert(kv[0].trim().to_string(), kv[1].trim().to_string());
    }

    Ok(params)
}

fn parse_f32(param: &str, value: &str) -> Result<f32, EffectError> {
    value.parse().map_err(|_| EffectError::InvalidValue {
        param: param.to_string(),
        message: format!("'{}' is not a valid number", value),
    })
}

fn parse_waveshape(value: &str) -> Result<WaveShape, EffectError> {
    match value.to_lowercase().as_str() {
        "softclip" | "soft" => Ok(WaveShape::SoftClip),
        "hardclip" | "hard" => Ok(WaveShape::HardClip),
        "foldback" | "fold" => Ok(WaveShape::Foldback),
        "asymmetric" | "asym" => Ok(WaveShape::Asymmetric),
        _ => Err(EffectError::InvalidValue {
            param: "waveshape".to_string(),
            message: format!(
                "'{}' is not a valid waveshape (use: softclip, hardclip, foldback, asymmetric)",
                value
            ),
        }),
    }
}

fn parse_reverb_type(value: &str) -> Result<ReverbType, EffectError> {
    match value.to_lowercase().as_str() {
        "room" => Ok(ReverbType::Room),
        "hall" => Ok(ReverbType::Hall),
        _ => Err(EffectError::InvalidValue {
            param: "type".to_string(),
            message: format!(
                "'{}' is not a valid reverb type (use: room, hall)",
                value
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_effect_with_params() {
        let params = HashMap::new();
        let effect = create_effect_with_params("distortion", 48000.0, &params);
        assert!(effect.is_ok());

        let effect = create_effect_with_params("unknown", 48000.0, &params);
        assert!(effect.is_err());
    }

    #[test]
    fn test_parse_params() {
        let params = parse_params("drive=15,tone=4000").unwrap();
        assert_eq!(params.get("drive"), Some(&"15".to_string()));
        assert_eq!(params.get("tone"), Some(&"4000".to_string()));
    }

    #[test]
    fn test_parse_effect_spec() {
        let (name, params) = parse_effect_spec("distortion:drive=15,level=-6").unwrap();
        assert_eq!(name, "distortion");
        assert_eq!(params.get("drive"), Some(&"15".to_string()));
        assert_eq!(params.get("level"), Some(&"-6".to_string()));
    }

    #[test]
    fn test_parse_chain() {
        let chain = parse_chain("preamp:gain=6|distortion:drive=12|delay:time=300", 48000.0);
        assert!(chain.is_ok());
        assert_eq!(chain.unwrap().len(), 3);
    }

    #[test]
    fn test_parse_chain_simple() {
        let chain = parse_chain("distortion", 48000.0);
        assert!(chain.is_ok());
        assert_eq!(chain.unwrap().len(), 1);
    }

    #[test]
    fn test_create_reverb() {
        let params = HashMap::new();
        let effect = create_effect_with_params("reverb", 48000.0, &params);
        assert!(effect.is_ok());

        // Test with parameters
        let mut params = HashMap::new();
        params.insert("decay".to_string(), "0.8".to_string());
        params.insert("room_size".to_string(), "0.7".to_string());
        params.insert("damping".to_string(), "0.3".to_string());
        params.insert("mix".to_string(), "0.5".to_string());
        params.insert("type".to_string(), "hall".to_string());
        let effect = create_effect_with_params("reverb", 48000.0, &params);
        assert!(effect.is_ok());
    }

    #[test]
    fn test_parse_chain_with_reverb() {
        let chain = parse_chain("delay:time=300|reverb:decay=0.9,mix=0.6", 48000.0);
        assert!(chain.is_ok());
        assert_eq!(chain.unwrap().len(), 2);
    }
}
