//! Sonido Phaser — CLAP audio effect plugin.
//!
//! Multi-stage allpass phaser with LFO.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "phaser",
    clap_id: "com.sonido.phaser",
    name: "Sonido Phaser",
    features: [AUDIO_EFFECT, PHASER, STEREO],
}
