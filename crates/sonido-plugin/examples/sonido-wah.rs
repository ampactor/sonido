//! Sonido Wah — CLAP audio effect plugin.
//!
//! Auto-wah and manual wah with envelope follower.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "wah",
    clap_id: "com.sonido.wah",
    name: "Sonido Wah",
    features: [AUDIO_EFFECT, FILTER, STEREO],
}
