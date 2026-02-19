//! Sonido Bitcrusher — CLAP audio effect plugin.
//!
//! Lo-fi bit depth and sample rate reduction with jitter.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "bitcrusher",
    clap_id: "com.sonido.bitcrusher",
    name: "Sonido Bitcrusher",
    features: [AUDIO_EFFECT, DISTORTION, STEREO],
}
