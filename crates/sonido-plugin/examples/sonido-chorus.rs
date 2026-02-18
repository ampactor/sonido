//! Sonido Chorus — CLAP audio effect plugin.
//!
//! Multi-voice modulated delay chorus with feedback.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "chorus",
    clap_id: "com.sonido.chorus",
    name: "Sonido Chorus",
    features: [AUDIO_EFFECT, CHORUS, STEREO],
}
