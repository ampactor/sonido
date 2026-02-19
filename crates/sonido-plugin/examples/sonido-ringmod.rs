//! Sonido Ring Modulator — CLAP audio effect plugin.
//!
//! Ring modulation with sine, triangle, and square carriers.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "ringmod",
    clap_id: "com.sonido.ringmod",
    name: "Sonido Ring Mod",
    features: [AUDIO_EFFECT, UTILITY, STEREO],
}
