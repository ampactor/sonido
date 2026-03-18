//! Noon preset values — guitarist sweet spots for hardware knob mapping.
//!
//! "Noon" = 12 o'clock knob position on the Hothouse 6-knob pedal.
//! Each hardware-mapped effect defines parameter values that constitute a
//! "sweet spot" — the tone you get when all knobs are at center. These values
//! feed into the biased ADC mapping so that knob center maps to the sweet spot
//! rather than the raw midpoint of the parameter range.
//!
//! Noon values are **guitarist sweet spots**, not always descriptor defaults.
//! Mix params use 50% (pedal blend convention) even though the plugin default
//! is 100% (insert chain convention).
//!
//! Arrays include ALL parameters (writable + READ_ONLY) to match
//! `KernelParams::COUNT` for index-based embedded access. The biased mapping
//! automatically falls back to linear for STEPPED params and noon values at
//! range extremes.

/// Effects with hardware knob mappings (Hothouse 6-knob pedal).
///
/// Only these effects require noon presets. Effects not on this list
/// (amp, cabinet, pitch_shift, etc.) use descriptor defaults on hardware
/// or are plugin/GUI-only.
pub const HARDWARE_MAPPED: &[&str] = &[
    "bitcrusher",
    "chorus",
    "compressor",
    "delay",
    "distortion",
    "eq",
    "filter",
    "flanger",
    "gate",
    "limiter",
    "looper",
    "phaser",
    "preamp",
    "reverb",
    "ringmod",
    "stage",
    "tape",
    "tremolo",
    "vibrato",
    "wah",
];

/// Returns the "sweet spot" value for a parameter of a named effect.
///
/// `effect_id` is the registry ID (e.g., `"distortion"`, `"delay"`).
/// `param_idx` is the parameter index within the effect.
///
/// Returns `None` if the effect has no noon preset or the index is out of
/// range, in which case callers should fall back to the descriptor's `default`.
pub fn noon_value(effect_id: &str, param_idx: usize) -> Option<f32> {
    let values: &[f32] = match effect_id {
        "bitcrusher" => &[8.0, 1.0, 0.0, 50.0, 0.0],
        "chorus" => &[1.0, 50.0, 50.0, 2.0, 0.0, 15.0, 0.0, 3.0, 0.0, 0.0],
        "compressor" => &[
            -18.0, 4.0, 10.0, 100.0, 0.0, 6.0, 0.0, 80.0, 0.0, 0.0, 50.0, 0.0,
        ],
        "delay" => &[300.0, 40.0, 50.0, 0.0, 20000.0, 20.0, 0.0, 0.0, 2.0, 0.0],
        "distortion" => &[8.0, 0.0, 0.0, 0.0, 50.0, 0.0],
        "eq" => &[100.0, 0.0, 1.0, 500.0, 0.0, 1.0, 5000.0, 0.0, 1.0, 0.0],
        "filter" => &[1000.0, 0.707, 0.0, 0.0],
        "flanger" => &[0.5, 35.0, 50.0, 50.0, 0.0, 0.0, 3.0, 0.0, 0.0],
        "gate" => &[-40.0, 1.0, 100.0, 50.0, -80.0, 3.0, 80.0, 0.0, 0.0],
        "limiter" => &[-6.0, -0.3, 100.0, 5.0, 0.0],
        "looper" => &[0.0, 80.0, 0.0, 0.0, 50.0, 0.0],
        "phaser" => &[0.3, 50.0, 6.0, 50.0, 50.0, 20.0, 4000.0, 0.0, 3.0, 0.0, 0.0],
        "preamp" => &[0.0, 0.0, 0.0],
        "reverb" => &[50.0, 50.0, 30.0, 10.0, 50.0, 100.0, 50.0, 0.0],
        "ringmod" => &[440.0, 50.0, 0.0, 50.0, 0.0],
        "stage" => &[
            0.0, 100.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 120.0, 0.0, 0.0, 0.0,
        ],
        "tape" => &[6.0, 30.0, 12000.0, 0.0, 0.3, 0.2, 0.15, 0.3, 80.0, -6.0],
        "tremolo" => &[5.0, 50.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0],
        "vibrato" => &[100.0, 50.0, 0.0],
        "wah" => &[800.0, 5.0, 50.0, 0.0, 0.0],
        _ => return None,
    };
    values.get(param_idx).copied()
}
