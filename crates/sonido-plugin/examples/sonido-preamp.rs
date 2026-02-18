//! Sonido Preamp — CLAP audio effect plugin.
//!
//! High-headroom clean gain stage.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "preamp",
    clap_id: "com.sonido.preamp",
    name: "Sonido Preamp",
    features: [AUDIO_EFFECT, UTILITY, STEREO],
}
