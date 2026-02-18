//! Sonido Parametric EQ — CLAP audio effect plugin.
//!
//! 3-band parametric equalizer with frequency, gain, and Q.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "eq",
    clap_id: "com.sonido.eq",
    name: "Sonido Parametric EQ",
    features: [AUDIO_EFFECT, EQUALIZER, STEREO],
}
