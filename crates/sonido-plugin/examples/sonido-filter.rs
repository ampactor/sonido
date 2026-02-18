//! Sonido Filter — CLAP audio effect plugin.
//!
//! Resonant lowpass filter with drive.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "filter",
    clap_id: "com.sonido.filter",
    name: "Sonido Filter",
    features: [AUDIO_EFFECT, FILTER, STEREO],
}
