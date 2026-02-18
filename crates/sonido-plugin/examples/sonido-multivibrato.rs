//! Sonido Multi Vibrato — CLAP audio effect plugin.
//!
//! 10-unit tape wow/flutter simulation.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "multivibrato",
    clap_id: "com.sonido.multivibrato",
    name: "Sonido Multi Vibrato",
    features: [AUDIO_EFFECT, CHORUS, STEREO],
}
