//! Sonido Flanger — CLAP audio effect plugin.
//!
//! Classic flanger with through-zero mode and bipolar feedback.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "flanger",
    clap_id: "com.sonido.flanger",
    name: "Sonido Flanger",
    features: [AUDIO_EFFECT, FLANGER, STEREO],
}
