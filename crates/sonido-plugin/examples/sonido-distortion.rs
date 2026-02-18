//! Sonido Distortion — CLAP audio effect plugin.
//!
//! Anti-aliased waveshaping distortion with multiple clipping algorithms.
//!
//! Build: `cargo build -p sonido-plugin --example sonido-distortion`
//! Output: `target/debug/examples/libsonido_distortion.so` (rename to `.clap`)

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "distortion",
    clap_id: "com.sonido.distortion",
    name: "Sonido Distortion",
    features: [AUDIO_EFFECT, DISTORTION, STEREO],
}
