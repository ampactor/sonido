//! Sonido Stage — CLAP audio effect plugin.
//!
//! Signal conditioning: gain, phase, width, balance, bass mono, Haas delay.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "stage",
    clap_id: "com.sonido.stage",
    name: "Sonido Stage",
    features: [AUDIO_EFFECT, UTILITY, STEREO],
}
