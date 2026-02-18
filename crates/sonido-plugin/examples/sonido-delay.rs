//! Sonido Delay — CLAP audio effect plugin.
//!
//! Feedback delay with filtering, diffusion, and tempo sync.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "delay",
    clap_id: "com.sonido.delay",
    name: "Sonido Delay",
    features: [AUDIO_EFFECT, DELAY, STEREO],
}
