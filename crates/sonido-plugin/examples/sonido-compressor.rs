//! Sonido Compressor — CLAP audio effect plugin.
//!
//! Dynamics compressor with program-dependent release.

use sonido_plugin::sonido_effect_entry;

sonido_effect_entry! {
    effect_id: "compressor",
    clap_id: "com.sonido.compressor",
    name: "Sonido Compressor",
    features: [AUDIO_EFFECT, COMPRESSOR, STEREO],
}
