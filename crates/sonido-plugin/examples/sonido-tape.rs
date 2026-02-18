//! Sonido Tape Saturation — CLAP audio effect plugin.
//!
//! Full tape-machine model with wow/flutter, hysteresis, head bump, and self-erasure.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "tape",
    clap_id: "com.sonido.tape",
    name: "Sonido Tape Saturation",
    features: [AUDIO_EFFECT, DISTORTION, STEREO],
}
