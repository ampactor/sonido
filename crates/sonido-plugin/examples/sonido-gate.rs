//! Sonido Gate — CLAP audio effect plugin.
//!
//! Noise gate with threshold, hold, hysteresis, and sidechain HPF.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "gate",
    clap_id: "com.sonido.gate",
    name: "Sonido Gate",
    features: [AUDIO_EFFECT, COMPRESSOR, STEREO],
}
