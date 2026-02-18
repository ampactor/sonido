//! Sonido Reverb — CLAP audio effect plugin.
//!
//! Freeverb-style algorithmic reverb.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "reverb",
    clap_id: "com.sonido.reverb",
    name: "Sonido Reverb",
    features: [AUDIO_EFFECT, REVERB, STEREO],
}
